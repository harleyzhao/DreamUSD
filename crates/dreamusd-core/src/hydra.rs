use crate::error::{check, DuError};
use crate::stage::Stage;
use dreamusd_sys::{self, DuDisplayMode, DuRendererSettingType};
use std::ffi::{CStr, CString};
use std::os::raw::c_void;
use std::ptr;

/// Safe wrapper around the Hydra rendering engine.
pub struct HydraEngine {
    raw: *mut dreamusd_sys::DuHydraEngine,
}

unsafe impl Send for HydraEngine {}

/// Display mode for the Hydra viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    SmoothShaded,
    Wireframe,
    WireframeOnShaded,
    FlatShaded,
    Points,
    Textured,
    GeomOnly,
    GeomFlat,
    GeomSmooth,
}

impl From<DisplayMode> for DuDisplayMode {
    fn from(m: DisplayMode) -> Self {
        match m {
            DisplayMode::SmoothShaded => DuDisplayMode::SmoothShaded,
            DisplayMode::Wireframe => DuDisplayMode::Wireframe,
            DisplayMode::WireframeOnShaded => DuDisplayMode::WireframeOnShaded,
            DisplayMode::FlatShaded => DuDisplayMode::FlatShaded,
            DisplayMode::Points => DuDisplayMode::Points,
            DisplayMode::Textured => DuDisplayMode::Textured,
            DisplayMode::GeomOnly => DuDisplayMode::GeomOnly,
            DisplayMode::GeomFlat => DuDisplayMode::GeomFlat,
            DisplayMode::GeomSmooth => DuDisplayMode::GeomSmooth,
        }
    }
}

/// Information about the Vulkan image produced by a Hydra render pass.
pub struct VkImageInfo {
    pub image: u64,
    pub image_view: u64,
    pub format: u32,
    pub width: u32,
    pub height: u32,
}

/// Information about the platform-native texture produced by the color AOV.
pub struct NativeTextureInfo {
    pub texture: u64,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererSettingType {
    Flag,
    Int,
    Float,
    String,
}

impl From<DuRendererSettingType> for RendererSettingType {
    fn from(value: DuRendererSettingType) -> Self {
        match value {
            DuRendererSettingType::Flag => Self::Flag,
            DuRendererSettingType::Int => Self::Int,
            DuRendererSettingType::Float => Self::Float,
            DuRendererSettingType::String => Self::String,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RendererSetting {
    pub key: String,
    pub name: String,
    pub setting_type: RendererSettingType,
    pub current_value: String,
    pub default_value: String,
}

impl HydraEngine {
    /// Create a new Hydra engine using platform-default graphics backend.
    /// Uses Metal on macOS, OpenGL on Linux.
    pub fn create(stage: &Stage) -> Result<Self, DuError> {
        let mut raw: *mut dreamusd_sys::DuHydraEngine = ptr::null_mut();
        unsafe {
            check(dreamusd_sys::du_hydra_create(stage.raw, &mut raw))?;
        }
        Ok(Self { raw })
    }

    /// Create a new Hydra engine bound to the given stage and Vulkan handles.
    pub fn new(
        stage: &Stage,
        vk_instance: *mut c_void,
        vk_physical_device: *mut c_void,
        vk_device: *mut c_void,
        queue_family_index: u32,
    ) -> Result<Self, DuError> {
        let mut raw: *mut dreamusd_sys::DuHydraEngine = ptr::null_mut();
        unsafe {
            check(dreamusd_sys::du_hydra_create_with_vulkan(
                stage.raw,
                vk_instance,
                vk_physical_device,
                vk_device,
                queue_family_index,
                &mut raw,
            ))?;
        }
        Ok(Self { raw })
    }

    /// Render a frame at the given resolution.
    pub fn render(&self, width: u32, height: u32) -> Result<(), DuError> {
        unsafe { check(dreamusd_sys::du_hydra_render(self.raw, width, height)) }
    }

    /// Get the rendered framebuffer as RGBA pixels (CPU readback).
    /// Returns (rgba_data, width, height). The data is owned by the engine
    /// and valid until the next render call.
    pub fn get_framebuffer(&self) -> Result<(&[u8], u32, u32), DuError> {
        let mut rgba: *mut u8 = ptr::null_mut();
        let mut w: u32 = 0;
        let mut h: u32 = 0;
        unsafe {
            check(dreamusd_sys::du_hydra_get_framebuffer(
                self.raw, &mut rgba, &mut w, &mut h,
            ))?;
            let len = (w as usize) * (h as usize) * 4;
            Ok((std::slice::from_raw_parts(rgba, len), w, h))
        }
    }

    /// Retrieve the Vulkan image produced by the last render pass.
    pub fn get_vk_image(&self) -> Result<VkImageInfo, DuError> {
        let mut image: u64 = 0;
        let mut view: u64 = 0;
        let mut format: u32 = 0;
        let mut width: u32 = 0;
        let mut height: u32 = 0;
        unsafe {
            check(dreamusd_sys::du_hydra_get_vk_image(
                self.raw,
                &mut image as *mut u64 as *mut c_void,
                &mut view as *mut u64 as *mut c_void,
                &mut format,
                &mut width,
                &mut height,
            ))?;
        }
        Ok(VkImageInfo {
            image,
            image_view: view,
            format,
            width,
            height,
        })
    }

    /// Retrieve the platform-native texture for the color AOV.
    pub fn get_native_texture(&self) -> Result<NativeTextureInfo, DuError> {
        let mut texture: u64 = 0;
        let mut width: u32 = 0;
        let mut height: u32 = 0;
        unsafe {
            check(dreamusd_sys::du_hydra_get_native_texture(
                self.raw,
                &mut texture as *mut u64 as *mut c_void,
                &mut width,
                &mut height,
            ))?;
        }
        Ok(NativeTextureInfo {
            texture,
            width,
            height,
        })
    }

    /// Get the Vulkan semaphore that signals when rendering is complete.
    pub fn get_render_semaphore(&self) -> Result<u64, DuError> {
        let mut semaphore: u64 = 0;
        unsafe {
            check(dreamusd_sys::du_hydra_get_render_semaphore(
                self.raw,
                &mut semaphore as *mut u64 as *mut c_void,
            ))?;
        }
        Ok(semaphore)
    }

    /// Set the camera for the next render.
    pub fn set_camera(
        &self,
        eye: [f64; 3],
        target: [f64; 3],
        up: [f64; 3],
    ) -> Result<(), DuError> {
        let mut eye = eye;
        let mut target = target;
        let mut up = up;
        unsafe {
            check(dreamusd_sys::du_hydra_set_camera(
                self.raw,
                eye.as_mut_ptr(),
                target.as_mut_ptr(),
                up.as_mut_ptr(),
            ))
        }
    }

    /// Set the lens parameters for the free camera used by Hydra.
    pub fn set_camera_lens(
        &self,
        fov_y_radians: f64,
        near_plane: f64,
        far_plane: f64,
    ) -> Result<(), DuError> {
        unsafe {
            check(dreamusd_sys::du_hydra_set_camera_lens(
                self.raw,
                fov_y_radians,
                near_plane,
                far_plane,
            ))
        }
    }

    /// Compute automatic clipping planes from the current free camera and scene bounds.
    pub fn compute_auto_clip(&self) -> Result<(f64, f64), DuError> {
        let mut near_plane = 0.0f64;
        let mut far_plane = 0.0f64;
        unsafe {
            check(dreamusd_sys::du_hydra_compute_auto_clip(
                self.raw,
                &mut near_plane,
                &mut far_plane,
            ))?;
        }
        Ok((near_plane, far_plane))
    }

    /// Project a 3D world point to 2D screen coordinates using the same
    /// view/projection matrices as the Hydra render. Returns (screen_x, screen_y)
    /// in pixel coordinates within the viewport. Returns None if behind camera.
    pub fn project_point(
        &self,
        world_xyz: [f64; 3],
        viewport_w: u32,
        viewport_h: u32,
    ) -> Option<(f64, f64)> {
        let mut screen_xy = [0.0f64; 2];
        let status = unsafe {
            dreamusd_sys::du_hydra_project_point(
                self.raw,
                world_xyz.as_ptr(),
                viewport_w,
                viewport_h,
                screen_xy.as_mut_ptr(),
            )
        };
        if status == dreamusd_sys::DuStatus::Ok {
            Some((screen_xy[0], screen_xy[1]))
        } else {
            None
        }
    }

    /// Pick the prim under the given screen-space pixel coordinate.
    pub fn pick_prim(
        &self,
        screen_x: f64,
        screen_y: f64,
        viewport_w: u32,
        viewport_h: u32,
    ) -> Result<String, DuError> {
        let mut path: *const std::os::raw::c_char = ptr::null();
        unsafe {
            check(dreamusd_sys::du_hydra_pick(
                self.raw,
                screen_x,
                screen_y,
                viewport_w,
                viewport_h,
                &mut path,
            ))?;
            Ok(CStr::from_ptr(path).to_string_lossy().into_owned())
        }
    }

    /// Update the currently highlighted prim in Hydra.
    pub fn set_selection(&self, selected_path: Option<&str>) -> Result<(), DuError> {
        let selection = selected_path.into_iter().collect::<Vec<_>>();
        self.set_selection_paths(&selection)
    }

    /// Update the currently highlighted prim set in Hydra.
    pub fn set_selection_paths(&self, selected_paths: &[&str]) -> Result<(), DuError> {
        let c_paths = selected_paths
            .iter()
            .map(|path| {
                CString::new(*path).map_err(|_| {
                    DuError::Invalid("Selection path contains an interior NUL byte".into())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let path_ptrs = c_paths.iter().map(|path| path.as_ptr()).collect::<Vec<_>>();
        unsafe {
            check(dreamusd_sys::du_hydra_set_selection_paths(
                self.raw,
                path_ptrs.as_ptr(),
                path_ptrs.len() as u32,
            ))
        }
    }

    /// Poll the renderer for asynchronous updates.
    pub fn poll_async_updates(&self) -> Result<bool, DuError> {
        let mut changed = false;
        unsafe {
            check(dreamusd_sys::du_hydra_poll_async_updates(
                self.raw,
                &mut changed,
            ))?;
        }
        Ok(changed)
    }

    /// Enable or disable lighting.
    pub fn set_enable_lighting(&self, enable: bool) -> Result<(), DuError> {
        unsafe {
            check(dreamusd_sys::du_hydra_set_enable_lighting(self.raw, enable))
        }
    }

    /// Enable or disable shadow rendering.
    pub fn set_enable_shadows(&self, enable: bool) -> Result<(), DuError> {
        unsafe {
            check(dreamusd_sys::du_hydra_set_enable_shadows(self.raw, enable))
        }
    }

    /// Enable or disable MSAA anti-aliasing.
    pub fn set_msaa(&self, enable: bool) -> Result<(), DuError> {
        unsafe {
            check(dreamusd_sys::du_hydra_set_msaa(self.raw, enable))
        }
    }

    /// Set the viewport refinement complexity.
    pub fn set_complexity(&self, complexity: f32) -> Result<(), DuError> {
        unsafe { check(dreamusd_sys::du_hydra_set_complexity(self.raw, complexity)) }
    }

    /// Show or hide guide-purpose prims.
    pub fn set_show_guides(&self, enable: bool) -> Result<(), DuError> {
        unsafe { check(dreamusd_sys::du_hydra_set_show_guides(self.raw, enable)) }
    }

    /// Show or hide proxy-purpose prims.
    pub fn set_show_proxy(&self, enable: bool) -> Result<(), DuError> {
        unsafe { check(dreamusd_sys::du_hydra_set_show_proxy(self.raw, enable)) }
    }

    /// Show or hide render-purpose prims.
    pub fn set_show_render(&self, enable: bool) -> Result<(), DuError> {
        unsafe { check(dreamusd_sys::du_hydra_set_show_render(self.raw, enable)) }
    }

    /// Enable or disable backface culling.
    pub fn set_cull_backfaces(&self, enable: bool) -> Result<(), DuError> {
        unsafe { check(dreamusd_sys::du_hydra_set_cull_backfaces(self.raw, enable)) }
    }

    /// Enable or disable scene materials.
    pub fn set_enable_scene_materials(&self, enable: bool) -> Result<(), DuError> {
        unsafe {
            check(dreamusd_sys::du_hydra_set_enable_scene_materials(
                self.raw,
                enable,
            ))
        }
    }

    /// Control whether dome lights remain visible to the camera.
    pub fn set_dome_light_camera_visibility(&self, enable: bool) -> Result<(), DuError> {
        unsafe {
            check(dreamusd_sys::du_hydra_set_dome_light_camera_visibility(
                self.raw,
                enable,
            ))
        }
    }

    /// Set the display mode (shading style).
    pub fn set_display_mode(&self, mode: DisplayMode) -> Result<(), DuError> {
        unsafe {
            check(dreamusd_sys::du_hydra_set_display_mode(
                self.raw,
                mode.into(),
            ))
        }
    }

    /// List all available render delegates.
    pub fn list_render_delegates() -> Result<Vec<String>, DuError> {
        let mut names: *mut *const std::os::raw::c_char = ptr::null_mut();
        let mut count: u32 = 0;
        unsafe {
            check(dreamusd_sys::du_rd_list_available(&mut names, &mut count))?;
            let result = (0..count as usize)
                .map(|i| {
                    CStr::from_ptr(*names.add(i))
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            dreamusd_sys::du_free_string_array(names, count);
            Ok(result)
        }
    }

    /// Get the name of the currently active render delegate.
    pub fn current_render_delegate(&self) -> Result<String, DuError> {
        let mut name: *const std::os::raw::c_char = ptr::null();
        unsafe {
            check(dreamusd_sys::du_rd_get_current(self.raw, &mut name))?;
            Ok(CStr::from_ptr(name).to_string_lossy().into_owned())
        }
    }

    /// Switch to a different render delegate by name.
    pub fn set_render_delegate(&self, name: &str) -> Result<(), DuError> {
        let c_name = CString::new(name)
            .map_err(|_| DuError::Invalid("name contains null byte".into()))?;
        unsafe { check(dreamusd_sys::du_rd_set_current(self.raw, c_name.as_ptr())) }
    }

    /// List renderer AOVs available on the current delegate.
    pub fn list_renderer_aovs(&self) -> Result<Vec<String>, DuError> {
        let mut names: *mut *const std::os::raw::c_char = ptr::null_mut();
        let mut count: u32 = 0;
        unsafe {
            check(dreamusd_sys::du_rd_get_aovs(self.raw, &mut names, &mut count))?;
            let result = (0..count as usize)
                .map(|i| {
                    CStr::from_ptr(*names.add(i))
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            dreamusd_sys::du_free_string_array(names, count);
            Ok(result)
        }
    }

    /// Get the currently selected renderer AOV.
    pub fn current_renderer_aov(&self) -> Result<String, DuError> {
        let mut name: *const std::os::raw::c_char = ptr::null();
        unsafe {
            check(dreamusd_sys::du_rd_get_current_aov(self.raw, &mut name))?;
            Ok(CStr::from_ptr(name).to_string_lossy().into_owned())
        }
    }

    /// Switch the viewport to a different renderer AOV.
    pub fn set_renderer_aov(&self, name: &str) -> Result<(), DuError> {
        let c_name = CString::new(name)
            .map_err(|_| DuError::Invalid("AOV name contains null byte".into()))?;
        unsafe { check(dreamusd_sys::du_rd_set_current_aov(self.raw, c_name.as_ptr())) }
    }

    /// Query renderer-specific settings exposed by the active delegate.
    pub fn renderer_settings(&self) -> Result<Vec<RendererSetting>, DuError> {
        let mut settings: *mut dreamusd_sys::DuRendererSetting = ptr::null_mut();
        let mut count: u32 = 0;
        unsafe {
            check(dreamusd_sys::du_rd_get_settings(self.raw, &mut settings, &mut count))?;
            let result = (0..count as usize)
                .map(|i| {
                    let setting = &*settings.add(i);
                    RendererSetting {
                        key: CStr::from_ptr(setting.key).to_string_lossy().into_owned(),
                        name: CStr::from_ptr(setting.name).to_string_lossy().into_owned(),
                        setting_type: setting.r#type.into(),
                        current_value: CStr::from_ptr(setting.current_value)
                            .to_string_lossy()
                            .into_owned(),
                        default_value: CStr::from_ptr(setting.default_value)
                            .to_string_lossy()
                            .into_owned(),
                    }
                })
                .collect();
            dreamusd_sys::du_free_renderer_settings(settings, count);
            Ok(result)
        }
    }

    pub fn set_renderer_setting_bool(&self, key: &str, value: bool) -> Result<(), DuError> {
        let c_key = CString::new(key)
            .map_err(|_| DuError::Invalid("setting key contains null byte".into()))?;
        unsafe { check(dreamusd_sys::du_rd_set_setting_bool(self.raw, c_key.as_ptr(), value)) }
    }

    pub fn set_renderer_setting_int(&self, key: &str, value: i32) -> Result<(), DuError> {
        let c_key = CString::new(key)
            .map_err(|_| DuError::Invalid("setting key contains null byte".into()))?;
        unsafe { check(dreamusd_sys::du_rd_set_setting_int(self.raw, c_key.as_ptr(), value)) }
    }

    pub fn set_renderer_setting_float(&self, key: &str, value: f32) -> Result<(), DuError> {
        let c_key = CString::new(key)
            .map_err(|_| DuError::Invalid("setting key contains null byte".into()))?;
        unsafe { check(dreamusd_sys::du_rd_set_setting_float(self.raw, c_key.as_ptr(), value)) }
    }

    pub fn set_renderer_setting_string(&self, key: &str, value: &str) -> Result<(), DuError> {
        let c_key = CString::new(key)
            .map_err(|_| DuError::Invalid("setting key contains null byte".into()))?;
        let c_value = CString::new(value)
            .map_err(|_| DuError::Invalid("setting value contains null byte".into()))?;
        unsafe {
            check(dreamusd_sys::du_rd_set_setting_string(
                self.raw,
                c_key.as_ptr(),
                c_value.as_ptr(),
            ))
        }
    }
}

impl Drop for HydraEngine {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                dreamusd_sys::du_hydra_destroy(self.raw);
            }
        }
    }
}
