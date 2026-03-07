use dreamusd_core::NativeTextureInfo;
use eframe::egui;
use std::time::Instant;

use super::{DreamUsdApp, ViewportTexture, DISPLAY_MODES};

impl DreamUsdApp {
    fn render_hydra_frame(
        &mut self,
        width: u32,
        height: u32,
        interactive: bool,
        fetch_native_texture: bool,
    ) -> Result<Option<NativeTextureInfo>, ()> {
        let Some(hydra) = self.hydra.as_ref() else {
            return Err(());
        };

        let _ = hydra.set_camera(
            self.camera.eye_as_f64(),
            self.camera.target_as_f64(),
            self.camera.up_as_f64(),
        );
        let _ = hydra.set_camera_lens(
            self.camera.fov as f64,
            self.camera.near as f64,
            self.camera.far as f64,
        );
        let selection_target = self
            .hierarchy
            .selected_path
            .as_deref()
            .and_then(|path| self.resolve_transform_target_path(path))
            .or_else(|| self.hierarchy.selected_path.clone());
        let _ = hydra.set_selection(selection_target.as_deref());

        let (_, mode) = DISPLAY_MODES[self.current_display_mode];
        let (_, complexity) = super::VIEWPORT_COMPLEXITIES[self.current_complexity];
        let _ = hydra.set_display_mode(mode);
        let _ = hydra.set_complexity(complexity);
        let _ = hydra.set_enable_lighting(self.show_lights);
        let _ = hydra.set_enable_shadows(self.show_shadows);
        let _ = hydra.set_show_guides(self.show_guides);
        let _ = hydra.set_show_proxy(self.show_proxy);
        let _ = hydra.set_show_render(self.show_render);
        let _ = hydra.set_cull_backfaces(self.cull_backfaces);
        let _ = hydra.set_enable_scene_materials(self.enable_scene_materials);
        let _ = hydra.set_dome_light_camera_visibility(self.dome_light_textures_visible);
        let _ = hydra.set_msaa(self.aa_mode.uses_msaa() && !interactive);

        if hydra.render(width, height).is_err() {
            return Err(());
        }

        if fetch_native_texture {
            Ok(hydra.get_native_texture().ok())
        } else {
            Ok(None)
        }
    }

    pub(super) fn render_viewport(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        #[cfg(target_os = "macos")]
        self.drain_retired_native_textures();

        let interactive = self.viewport_interaction_frames > 0;
        let (width, height) = self.viewport_render_size(rect);
        let render_size = (width, height);
        let size_changed = self.last_viewport_render_size != Some(render_size);
        self.last_viewport_render_size = Some(render_size);
        self.update_auto_clip_target(interactive);
        let render_start = Instant::now();
        let mut render_count = 0;

        if size_changed {
            if self
                .render_hydra_frame(width, height, interactive, false)
                .is_err()
            {
                return;
            }
            render_count += 1;
        }

        let mut native_texture = match self.render_hydra_frame(width, height, interactive, true) {
            Ok(native_texture) => native_texture,
            Err(()) => return,
        };
        render_count += 1;

        let present_start = Instant::now();
        let mut used_gpu = native_texture
            .as_ref()
            .is_some_and(|texture| self.try_sync_native_viewport_texture(texture));

        if !used_gpu && !size_changed {
            native_texture = match self.render_hydra_frame(width, height, interactive, true) {
                Ok(native_texture) => native_texture,
                Err(()) => return,
            };
            render_count += 1;
            used_gpu = native_texture
                .as_ref()
                .is_some_and(|texture| self.try_sync_native_viewport_texture(texture));
        }

        if !used_gpu {
            let Some((pixels, framebuffer_width, framebuffer_height)) = self
                .hydra
                .as_ref()
                .and_then(|hydra| hydra.get_framebuffer().ok())
                .map(|(pixels, width, height)| (pixels.to_vec(), width, height))
            else {
                self.viewport_present_path = "---";
                return;
            };
            self.sync_cpu_viewport_texture(ctx, &pixels, framebuffer_width, framebuffer_height);
            self.viewport_present_path = "CPU";
            Self::update_smoothed_metric(
                &mut self.smoothed_viewport_present_time,
                present_start.elapsed().as_secs_f32(),
            );
            Self::update_smoothed_metric(
                &mut self.smoothed_hydra_render_time,
                render_start.elapsed().as_secs_f32(),
            );
            return;
        }

        Self::update_smoothed_metric(
            &mut self.smoothed_hydra_render_time,
            render_start.elapsed().as_secs_f32(),
        );

        self.viewport_present_path = if render_count > 1 { "GPUx2" } else { "GPU" };
        Self::update_smoothed_metric(
            &mut self.smoothed_viewport_present_time,
            present_start.elapsed().as_secs_f32(),
        );
    }

    fn sync_cpu_viewport_texture(
        &mut self,
        ctx: &egui::Context,
        pixels: &[u8],
        framebuffer_width: u32,
        framebuffer_height: u32,
    ) {
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [framebuffer_width as usize, framebuffer_height as usize],
            pixels,
        );
        if let Some(ViewportTexture::Cpu(texture)) = self.viewport_texture.as_mut() {
            texture.set(image, egui::TextureOptions::LINEAR);
        } else {
            let texture = ctx.load_texture("viewport", image, egui::TextureOptions::LINEAR);
            self.viewport_texture = Some(ViewportTexture::Cpu(texture));
        }
        self.viewport_texture_size = Some((framebuffer_width, framebuffer_height));
    }

    #[cfg(not(target_os = "macos"))]
    fn try_sync_native_viewport_texture(&mut self, _native_texture: &NativeTextureInfo) -> bool {
        false
    }

    #[cfg(target_os = "macos")]
    fn try_sync_native_viewport_texture(&mut self, native_texture: &NativeTextureInfo) -> bool {
        let Some(render_state) = self.render_state.clone() else {
            return false;
        };
        if render_state.adapter.get_info().backend != eframe::wgpu::Backend::Metal {
            return false;
        }

        match self.sync_metal_viewport_texture(render_state, native_texture) {
            Ok(texture_id) => {
                self.viewport_texture = Some(ViewportTexture::Native(texture_id));
                true
            }
            Err(err) => {
                tracing::warn!("Failed to import Hydra viewport texture: {err}");
                false
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn sync_metal_viewport_texture(
        &mut self,
        render_state: eframe::egui_wgpu::RenderState,
        native_texture: &NativeTextureInfo,
    ) -> Result<egui::TextureId, String> {
        let imported = import_metal_texture(&render_state, native_texture)?;

        if let Some(texture) = self.viewport_native_texture.as_ref() {
            if texture.raw_handle == native_texture.texture
                && texture.width == imported.width
                && texture.height == imported.height
            {
                return Ok(texture.texture_id);
            }
        }
        let mut renderer = render_state.renderer.write();
        let mut retired = None;
        let texture_id = if let Some(existing) = self.viewport_native_texture.as_ref() {
            let size_changed =
                existing.width != imported.width || existing.height != imported.height;
            if size_changed {
                retired = self.viewport_native_texture.take();
                renderer.register_native_texture(
                    &render_state.device,
                    &imported.view,
                    eframe::wgpu::FilterMode::Linear,
                )
            } else {
                renderer.update_egui_texture_from_wgpu_texture(
                    &render_state.device,
                    &imported.view,
                    eframe::wgpu::FilterMode::Linear,
                    existing.texture_id,
                );
                existing.texture_id
            }
        } else {
            renderer.register_native_texture(
                &render_state.device,
                &imported.view,
                eframe::wgpu::FilterMode::Linear,
            )
        };
        drop(renderer);

        if let Some(texture) = retired {
            self.retired_native_textures
                .push(super::RetiredViewportNativeTexture {
                    frames_left: 2,
                    texture,
                });
        }

        self.viewport_native_texture = Some(super::ViewportNativeTexture {
            texture_id,
            raw_handle: native_texture.texture,
            width: imported.width,
            height: imported.height,
            _texture: imported.texture,
            _view: imported.view,
        });
        self.viewport_texture_size = Some((imported.width, imported.height));
        Ok(texture_id)
    }
}

#[cfg(target_os = "macos")]
struct ImportedMetalTexture {
    width: u32,
    height: u32,
    texture: eframe::wgpu::Texture,
    view: eframe::wgpu::TextureView,
}

fn import_metal_texture(
    render_state: &eframe::egui_wgpu::RenderState,
    native_texture: &NativeTextureInfo,
) -> Result<ImportedMetalTexture, String> {
    use metal::{foreign_types::ForeignType, Texture};
    use objc::runtime::{objc_retain, Object};

    if native_texture.texture == 0 {
        return Err("Hydra returned a null native texture".into());
    }

    let raw_ptr = native_texture.texture as *mut Object;
    if raw_ptr.is_null() {
        return Err("Hydra returned an invalid native texture pointer".into());
    }

    let retained: *mut Object = unsafe { objc_retain(raw_ptr) };
    let metal_texture = unsafe { Texture::from_ptr(retained.cast()) };
    let width = metal_texture.width() as u32;
    let height = metal_texture.height() as u32;
    let depth = metal_texture.depth() as u32;
    let mip_level_count = metal_texture.mipmap_level_count() as u32;
    let sample_count = metal_texture.sample_count() as u32;
    let array_layers = metal_texture.array_length() as u32;
    let raw_type = metal_texture.texture_type();
    let format = map_metal_pixel_format(metal_texture.pixel_format())?;

    // egui displays imported native textures without an HDR tonemap pass.
    // For float AOVs, the CPU framebuffer path currently gives a more
    // correct viewport image because bridge-side readback applies tonemapping.
    if matches!(
        format,
        eframe::wgpu::TextureFormat::Rgba16Float | eframe::wgpu::TextureFormat::Rgba32Float
    ) {
        return Err("HDR Hydra AOV requires CPU tonemapping fallback".into());
    }

    let view_format = srgb_view_format(format);

    let hal_texture = unsafe {
        render_state
            .device
            .as_hal::<wgpu_hal::api::Metal, _, _>(|device| {
                let Some(_device) = device else {
                    return Err("eframe is not using the Metal backend".to_string());
                };

                Ok(wgpu_hal::metal::Device::texture_from_raw(
                    metal_texture,
                    format,
                    raw_type,
                    array_layers.max(1),
                    mip_level_count.max(1),
                    wgpu_hal::CopyExtent {
                        width: width.max(1),
                        height: height.max(1),
                        depth: depth.max(1),
                    },
                ))
            })
    }?;

    let descriptor = eframe::wgpu::TextureDescriptor {
        label: Some("dreamusd_viewport_native"),
        size: eframe::wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: depth.max(1),
        },
        mip_level_count: mip_level_count.max(1),
        sample_count: sample_count.max(1),
        dimension: eframe::wgpu::TextureDimension::D2,
        format,
        usage: eframe::wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: view_format
            .as_ref()
            .map(std::slice::from_ref)
            .unwrap_or(&[]),
    };
    let texture = unsafe {
        render_state
            .device
            .create_texture_from_hal::<wgpu_hal::api::Metal>(hal_texture, &descriptor)
    };
    let view = texture.create_view(&eframe::wgpu::TextureViewDescriptor {
        format: view_format,
        ..Default::default()
    });

    Ok(ImportedMetalTexture {
        width,
        height,
        texture,
        view,
    })
}

#[cfg(target_os = "macos")]
fn srgb_view_format(
    format: eframe::wgpu::TextureFormat,
) -> Option<eframe::wgpu::TextureFormat> {
    match format {
        eframe::wgpu::TextureFormat::Rgba8Unorm => {
            Some(eframe::wgpu::TextureFormat::Rgba8UnormSrgb)
        }
        eframe::wgpu::TextureFormat::Bgra8Unorm => {
            Some(eframe::wgpu::TextureFormat::Bgra8UnormSrgb)
        }
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn map_metal_pixel_format(
    pixel_format: metal::MTLPixelFormat,
) -> Result<eframe::wgpu::TextureFormat, String> {
    let format = match pixel_format {
        metal::MTLPixelFormat::RGBA8Unorm => eframe::wgpu::TextureFormat::Rgba8Unorm,
        metal::MTLPixelFormat::RGBA8Unorm_sRGB => eframe::wgpu::TextureFormat::Rgba8UnormSrgb,
        metal::MTLPixelFormat::BGRA8Unorm => eframe::wgpu::TextureFormat::Bgra8Unorm,
        metal::MTLPixelFormat::BGRA8Unorm_sRGB => eframe::wgpu::TextureFormat::Bgra8UnormSrgb,
        metal::MTLPixelFormat::RGBA16Float => eframe::wgpu::TextureFormat::Rgba16Float,
        metal::MTLPixelFormat::RGBA32Float => eframe::wgpu::TextureFormat::Rgba32Float,
        other => {
            return Err(format!(
                "unsupported Metal color texture format for egui import: {other:?}"
            ));
        }
    };
    Ok(format)
}
