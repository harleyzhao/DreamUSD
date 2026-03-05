use crate::camera::ViewportCamera;

/// A viewport with a camera and dimensions.
pub struct Viewport {
    pub camera: ViewportCamera,
    pub width: u32,
    pub height: u32,
}

impl Viewport {
    /// Create a new viewport with the given dimensions and a default camera.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            camera: ViewportCamera::default(),
            width,
            height,
        }
    }

    /// Return the aspect ratio of the viewport.
    pub fn aspect_ratio(&self) -> f32 {
        if self.height == 0 {
            1.0
        } else {
            self.width as f32 / self.height as f32
        }
    }

    /// Resize the viewport.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}
