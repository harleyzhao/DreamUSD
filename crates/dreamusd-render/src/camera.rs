use glam::{Mat4, Vec3, Vec4};

/// Camera interaction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    Orbit,
    Pan,
    Fly,
}

/// A camera suitable for 3-D viewport navigation.
pub struct ViewportCamera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub mode: CameraMode,
    orbit_distance: f32,
    yaw: f32,
    pitch: f32,
}

impl Default for ViewportCamera {
    fn default() -> Self {
        let eye = Vec3::new(0.0, 5.0, 10.0);
        let target = Vec3::ZERO;
        let diff = eye - target;
        let orbit_distance = diff.length();
        let yaw = diff.x.atan2(diff.z);
        let pitch = (diff.y / orbit_distance).asin();
        Self {
            eye,
            target,
            up: Vec3::Y,
            fov: 60.0_f32.to_radians(),
            near: 1.0,
            far: 2_000_000.0,
            mode: CameraMode::Orbit,
            orbit_distance,
            yaw,
            pitch,
        }
    }
}

impl ViewportCamera {
    /// Configure camera for Z-up coordinate system.
    pub fn set_z_up(&mut self) {
        self.up = Vec3::Z;
        self.eye = Vec3::new(10.0, -10.0, 5.0);
        self.target = Vec3::ZERO;
        self.near = 1.0;
        self.far = 2_000_000.0;
        let diff = self.eye - self.target;
        self.orbit_distance = diff.length();
        // For Z-up: yaw is rotation around Z, pitch is elevation from XY plane
        self.yaw = diff.y.atan2(diff.x);
        self.pitch = (diff.z / self.orbit_distance).asin();
    }

    /// Configure camera for Y-up coordinate system (default).
    pub fn set_y_up(&mut self) {
        self.up = Vec3::Y;
        self.eye = Vec3::new(0.0, 5.0, 10.0);
        self.target = Vec3::ZERO;
        self.near = 1.0;
        self.far = 2_000_000.0;
        let diff = self.eye - self.target;
        self.orbit_distance = diff.length();
        self.yaw = diff.x.atan2(diff.z);
        self.pitch = (diff.y / self.orbit_distance).asin();
    }

    /// Orbit the camera around the target by the given screen-space deltas.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        let sensitivity = 0.005;
        self.yaw -= dx * sensitivity;
        self.pitch = (self.pitch + dy * sensitivity).clamp(
            -std::f32::consts::FRAC_PI_2 + 0.01,
            std::f32::consts::FRAC_PI_2 - 0.01,
        );
        self.update_eye_from_orbit();
    }

    /// Pan the camera (translate both eye and target) by the given screen-space deltas.
    pub fn pan_pixels(&mut self, dx: f32, dy: f32, viewport_height: f32) {
        let forward = (self.target - self.eye).normalize();
        let right = forward.cross(self.up).normalize();
        let cam_up = right.cross(forward).normalize();
        let pixels_to_world = (2.0 * self.orbit_distance * (self.fov * 0.5).tan())
            / viewport_height.max(1.0);
        let offset = right * (-dx * pixels_to_world) + cam_up * (dy * pixels_to_world);
        self.eye += offset;
        self.target += offset;
    }

    /// Zoom (dolly) the camera toward or away from the target using scroll-wheel delta.
    pub fn zoom_scroll(&mut self, delta: f32) {
        let factor = 1.0 - (delta / 1000.0).clamp(-0.5, 0.5);
        self.orbit_distance = (self.orbit_distance * factor).max(0.01);
        self.update_eye_from_orbit();
    }

    /// Focus the camera on a bounding sphere defined by center and radius.
    pub fn focus_on(&mut self, center: Vec3, radius: f32) {
        self.target = center;
        self.orbit_distance = radius * 2.5;
        self.update_eye_from_orbit();
    }

    /// Return eye position as `[f64; 3]` for passing to the Hydra engine.
    pub fn eye_as_f64(&self) -> [f64; 3] {
        [self.eye.x as f64, self.eye.y as f64, self.eye.z as f64]
    }

    /// Return target position as `[f64; 3]` for passing to the Hydra engine.
    pub fn target_as_f64(&self) -> [f64; 3] {
        [
            self.target.x as f64,
            self.target.y as f64,
            self.target.z as f64,
        ]
    }

    /// Return up vector as `[f64; 3]` for passing to the Hydra engine.
    pub fn up_as_f64(&self) -> [f64; 3] {
        [self.up.x as f64, self.up.y as f64, self.up.z as f64]
    }

    /// Compute the view matrix.
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye, self.target, self.up)
    }

    /// Compute the projection matrix for a given aspect ratio.
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov, aspect, self.near, self.far)
    }

    /// Project a 3D world point to 2D screen coordinates.
    /// Returns (x, y) in pixels within the viewport, and depth.
    /// Returns None if the point is behind the camera.
    pub fn project_point(
        &self,
        world_pos: Vec3,
        viewport_x: f32,
        viewport_y: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> Option<(f32, f32, f32)> {
        let view = self.view_matrix();
        let proj = self.projection_matrix(viewport_w / viewport_h);
        let vp = proj * view;
        let clip = vp * Vec4::new(world_pos.x, world_pos.y, world_pos.z, 1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = Vec3::new(clip.x / clip.w, clip.y / clip.w, clip.z / clip.w);
        let sx = viewport_x + (ndc.x * 0.5 + 0.5) * viewport_w;
        let sy = viewport_y + (1.0 - (ndc.y * 0.5 + 0.5)) * viewport_h;
        Some((sx, sy, ndc.z))
    }

    /// Compute a world-space ray direction for a given screen point.
    pub fn unproject_direction(
        &self,
        screen_x: f32,
        screen_y: f32,
        viewport_x: f32,
        viewport_y: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> Vec3 {
        let ndc_x = ((screen_x - viewport_x) / viewport_w) * 2.0 - 1.0;
        let ndc_y = 1.0 - ((screen_y - viewport_y) / viewport_h) * 2.0;
        let view = self.view_matrix();
        let proj = self.projection_matrix(viewport_w / viewport_h);
        let inv_vp = (proj * view).inverse();
        let world_near = inv_vp * Vec4::new(ndc_x, ndc_y, -1.0, 1.0);
        let world_far = inv_vp * Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
        let near_pt = Vec3::new(
            world_near.x / world_near.w,
            world_near.y / world_near.w,
            world_near.z / world_near.w,
        );
        let far_pt = Vec3::new(
            world_far.x / world_far.w,
            world_far.y / world_far.w,
            world_far.z / world_far.w,
        );
        (far_pt - near_pt).normalize()
    }

    fn update_eye_from_orbit(&mut self) {
        if self.up == Vec3::Z {
            // Z-up: yaw rotates around Z, pitch elevates from XY plane
            let x = self.orbit_distance * self.pitch.cos() * self.yaw.cos();
            let y = self.orbit_distance * self.pitch.cos() * self.yaw.sin();
            let z = self.orbit_distance * self.pitch.sin();
            self.eye = self.target + Vec3::new(x, y, z);
        } else {
            // Y-up: yaw rotates around Y, pitch elevates from XZ plane
            let x = self.orbit_distance * self.pitch.cos() * self.yaw.sin();
            let y = self.orbit_distance * self.pitch.sin();
            let z = self.orbit_distance * self.pitch.cos() * self.yaw.cos();
            self.eye = self.target + Vec3::new(x, y, z);
        }
    }
}
