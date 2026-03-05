use glam::Vec3;

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
            fov: 45.0_f32.to_radians(),
            near: 0.01,
            far: 10000.0,
            mode: CameraMode::Orbit,
            orbit_distance,
            yaw,
            pitch,
        }
    }
}

impl ViewportCamera {
    /// Orbit the camera around the target by the given screen-space deltas.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        let sensitivity = 0.005;
        self.yaw += dx * sensitivity;
        self.pitch = (self.pitch - dy * sensitivity).clamp(
            -std::f32::consts::FRAC_PI_2 + 0.01,
            std::f32::consts::FRAC_PI_2 - 0.01,
        );
        self.update_eye_from_orbit();
    }

    /// Pan the camera (translate both eye and target) by the given screen-space deltas.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let forward = (self.target - self.eye).normalize();
        let right = forward.cross(self.up).normalize();
        let cam_up = right.cross(forward).normalize();
        let sensitivity = 0.01 * self.orbit_distance;
        let offset = right * (-dx * sensitivity) + cam_up * (dy * sensitivity);
        self.eye += offset;
        self.target += offset;
    }

    /// Zoom (dolly) the camera toward or away from the target.
    pub fn zoom(&mut self, delta: f32) {
        let factor = 1.0 - delta * 0.1;
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

    fn update_eye_from_orbit(&mut self) {
        let x = self.orbit_distance * self.pitch.cos() * self.yaw.sin();
        let y = self.orbit_distance * self.pitch.sin();
        let z = self.orbit_distance * self.pitch.cos() * self.yaw.cos();
        self.eye = self.target + Vec3::new(x, y, z);
    }
}
