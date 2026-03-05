use crate::error::{check, DuError};
use crate::stage::Stage;
use dreamusd_sys::{self, DuDisplayMode};
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
