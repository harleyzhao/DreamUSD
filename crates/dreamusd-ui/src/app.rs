mod commands;
mod render;
mod selection;

use eframe::egui;
use egui::Frame;

use crate::app::selection::find_prim_recursive;
use crate::panels::{HierarchyPanel, PropertiesPanel};
use dreamusd_core::{
    DisplayMode, HydraEngine, Prim, RendererSetting, RendererSettingType, Stage,
};
use dreamusd_render::glam::{EulerRot, Quat, Vec3};
use dreamusd_render::ViewportCamera;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

const DISPLAY_MODES: &[(&str, DisplayMode)] = &[
    ("Smooth Shaded", DisplayMode::SmoothShaded),
    ("Wireframe", DisplayMode::Wireframe),
    ("Wireframe on Shaded", DisplayMode::WireframeOnShaded),
    ("Flat Shaded", DisplayMode::FlatShaded),
    ("Points", DisplayMode::Points),
    ("Textured", DisplayMode::Textured),
    ("Geom Only", DisplayMode::GeomOnly),
    ("Geom Flat", DisplayMode::GeomFlat),
    ("Geom Smooth", DisplayMode::GeomSmooth),
];

const VIEWPORT_COMPLEXITIES: &[(&str, f32)] = &[
    ("Low", 1.0),
    ("Medium", 1.1),
    ("High", 1.2),
    ("Very High", 1.3),
];

fn usdview_selection_yellow() -> egui::Color32 {
    egui::Color32::from_rgb(255, 255, 0)
}

fn usdview_selection_yellow_fill() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(255, 255, 0, 96)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AntiAliasMode {
    Off,
    Msaa,
    Ssaa1_5x,
    Ssaa2x,
}

impl AntiAliasMode {
    fn label(self) -> &'static str {
        match self {
            AntiAliasMode::Off => "Off",
            AntiAliasMode::Msaa => "MSAA",
            AntiAliasMode::Ssaa1_5x => "SSAA 1.5x",
            AntiAliasMode::Ssaa2x => "SSAA 2x",
        }
    }

    fn all() -> &'static [AntiAliasMode] {
        &[
            AntiAliasMode::Off,
            AntiAliasMode::Msaa,
            AntiAliasMode::Ssaa1_5x,
            AntiAliasMode::Ssaa2x,
        ]
    }

    fn render_scale(self) -> f32 {
        match self {
            AntiAliasMode::Ssaa1_5x => 1.5,
            AntiAliasMode::Ssaa2x => 2.0,
            _ => 1.0,
        }
    }

    fn uses_msaa(self) -> bool {
        self == AntiAliasMode::Msaa
    }
}

impl GizmoSpace {
    fn label(self) -> &'static str {
        match self {
            GizmoSpace::Local => "Local",
            GizmoSpace::World => "World",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoMode {
    Select,
    Translate,
    Rotate,
    Scale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoSpace {
    Local,
    World,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GizmoHandle {
    Axis(usize),
    Plane(usize, usize),
    Center,
}

enum ViewportTexture {
    Cpu(egui::TextureHandle),
    Native(egui::TextureId),
}

impl ViewportTexture {
    fn id(&self) -> egui::TextureId {
        match self {
            Self::Cpu(texture) => texture.id(),
            Self::Native(texture_id) => *texture_id,
        }
    }
}

#[cfg(target_os = "macos")]
struct ViewportNativeTexture {
    texture_id: egui::TextureId,
    raw_handle: u64,
    width: u32,
    height: u32,
    _texture: eframe::wgpu::Texture,
    _view: eframe::wgpu::TextureView,
}

#[cfg(target_os = "macos")]
struct RetiredViewportNativeTexture {
    frames_left: u8,
    texture: ViewportNativeTexture,
}

pub struct DreamUsdApp {
    stage: Option<Stage>,
    hydra: Option<HydraEngine>,
    hierarchy: HierarchyPanel,
    camera: ViewportCamera,
    current_display_mode: usize,
    current_complexity: usize,
    show_grid: bool,
    show_axis: bool,
    show_shadows: bool,
    show_lights: bool,
    show_guides: bool,
    show_proxy: bool,
    show_render: bool,
    auto_compute_clipping_planes: bool,
    cull_backfaces: bool,
    enable_scene_materials: bool,
    dome_light_textures_visible: bool,
    status_message: String,
    gizmo_mode: GizmoMode,
    gizmo_space: GizmoSpace,
    viewport_texture: Option<ViewportTexture>,
    viewport_texture_size: Option<(u32, u32)>,
    render_state: Option<eframe::egui_wgpu::RenderState>,
    #[cfg(target_os = "macos")]
    viewport_native_texture: Option<ViewportNativeTexture>,
    #[cfg(target_os = "macos")]
    retired_native_textures: Vec<RetiredViewportNativeTexture>,
    hydra_error: Option<String>,
    // Gizmo interaction state
    dragging_handle: Option<GizmoHandle>,
    drag_start_pos: Option<Vec3>,
    drag_start_values: Option<[f64; 3]>,
    drag_start_local_rotation: Option<Quat>,
    drag_start_world_rotation: Option<Quat>,
    drag_start_pointer: Option<egui::Pos2>,
    drag_screen_dir: Option<egui::Vec2>,
    drag_screen_axis_len: Option<f32>,
    drag_world_axis_len: Option<f32>,
    viewport_rect: egui::Rect,
    viewport_pixels_per_point: f32,
    last_viewport_render_size: Option<(u32, u32)>,
    aa_mode: AntiAliasMode,
    viewport_interaction_frames: u8,
    last_frame_instant: Option<Instant>,
    smoothed_frame_time: Option<f32>,
    smoothed_hydra_render_time: Option<f32>,
    smoothed_viewport_present_time: Option<f32>,
    viewport_present_path: &'static str,
    renderer_settings_open: bool,
    camera_settings_open: bool,
    renderer_setting_text_edits: HashMap<String, String>,
    manual_clip_planes: (f32, f32),
    auto_clip_target: Option<(f32, f32)>,
    auto_clip_reference_distance: Option<f32>,
    auto_clip_needs_recompute: bool,
}

impl Default for DreamUsdApp {
    fn default() -> Self {
        Self {
            stage: None,
            hydra: None,
            hierarchy: HierarchyPanel::new(),
            camera: ViewportCamera::default(),
            current_display_mode: 0,
            current_complexity: 0,
            show_grid: true,
            show_axis: true,
            show_shadows: false,
            show_lights: true,
            show_guides: false,
            show_proxy: true,
            show_render: true,
            auto_compute_clipping_planes: false,
            cull_backfaces: false,
            enable_scene_materials: true,
            dome_light_textures_visible: true,
            status_message: "Ready".to_string(),
            gizmo_mode: GizmoMode::Select,
            gizmo_space: GizmoSpace::Local,
            viewport_texture: None,
            viewport_texture_size: None,
            render_state: None,
            #[cfg(target_os = "macos")]
            viewport_native_texture: None,
            #[cfg(target_os = "macos")]
            retired_native_textures: Vec::new(),
            hydra_error: None,
            dragging_handle: None,
            drag_start_pos: None,
            drag_start_values: None,
            drag_start_local_rotation: None,
            drag_start_world_rotation: None,
            drag_start_pointer: None,
            drag_screen_dir: None,
            drag_screen_axis_len: None,
            drag_world_axis_len: None,
            viewport_rect: egui::Rect::NOTHING,
            viewport_pixels_per_point: 1.0,
            last_viewport_render_size: None,
            aa_mode: AntiAliasMode::Msaa,
            viewport_interaction_frames: 0,
            last_frame_instant: None,
            smoothed_frame_time: None,
            smoothed_hydra_render_time: None,
            smoothed_viewport_present_time: None,
            viewport_present_path: "---",
            renderer_settings_open: false,
            camera_settings_open: false,
            renderer_setting_text_edits: HashMap::new(),
            manual_clip_planes: (1.0, 2_000_000.0),
            auto_clip_target: None,
            auto_clip_reference_distance: None,
            auto_clip_needs_recompute: false,
        }
    }
}

/// Light kind for viewport icon drawing.
#[derive(Clone, Copy, PartialEq)]
enum LightKind {
    Directional, // DistantLight, KeyLight, FillLight — parallel beams
    Point,       // SphereLight — bulb
    Dome,        // DomeLight — hemisphere
    Area,        // RectLight, DiskLight, CylinderLight — rectangle
}

struct LightInfo {
    pos: Vec3,
    path: String,
    kind: LightKind,
}

enum RendererSettingUpdate {
    Bool(String, bool),
    Int(String, i32),
    Float(String, f32),
    String(String, String),
}

fn light_kind(type_name: &str, name: &str) -> Option<LightKind> {
    match type_name {
        "DistantLight" => Some(LightKind::Directional),
        "DomeLight" | "DomeLight_1" => Some(LightKind::Dome),
        "SphereLight" | "PortalLight" => Some(LightKind::Point),
        "RectLight" | "DiskLight" | "CylinderLight" => Some(LightKind::Area),
        _ => match name {
            "KeyLight" | "FillLight" => Some(LightKind::Directional),
            "AmbientLight" => Some(LightKind::Dome),
            _ => None,
        },
    }
}

impl DreamUsdApp {
    fn set_gizmo_space(&mut self, space: GizmoSpace) {
        self.gizmo_space = space;
        self.status_message = format!("Gizmo space: {}", self.effective_gizmo_space().label());
    }

    fn toggle_gizmo_space(&mut self) {
        let next = match self.gizmo_space {
            GizmoSpace::Local => GizmoSpace::World,
            GizmoSpace::World => GizmoSpace::Local,
        };
        self.set_gizmo_space(next);
    }

    pub fn new(cc: &eframe::CreationContext<'_>, initial_scene: Option<PathBuf>) -> Self {
        crate::theme::apply(&cc.egui_ctx);
        let mut app = Self::default();
        app.render_state = cc.wgpu_render_state.clone();
        if let Some(path) = initial_scene {
            app.open_path(&path);
        }
        app
    }

    fn clear_viewport_texture(&mut self) {
        #[cfg(target_os = "macos")]
        if let Some(native_texture) = self.viewport_native_texture.take() {
            if let Some(render_state) = self.render_state.as_ref() {
                render_state
                    .renderer
                    .write()
                    .free_texture(&native_texture.texture_id);
            }
        }

        #[cfg(target_os = "macos")]
        if let Some(render_state) = self.render_state.as_ref() {
            let mut renderer = render_state.renderer.write();
            for retired in self.retired_native_textures.drain(..) {
                renderer.free_texture(&retired.texture.texture_id);
            }
        } else {
            #[cfg(target_os = "macos")]
            self.retired_native_textures.clear();
        }

        self.viewport_texture = None;
        self.viewport_texture_size = None;
    }

    #[cfg(target_os = "macos")]
    fn drain_retired_native_textures(&mut self) {
        let Some(render_state) = self.render_state.as_ref() else {
            self.retired_native_textures.clear();
            return;
        };

        let mut renderer = render_state.renderer.write();
        let mut still_retained = Vec::with_capacity(self.retired_native_textures.len());
        for mut retired in self.retired_native_textures.drain(..) {
            if retired.frames_left == 0 {
                renderer.free_texture(&retired.texture.texture_id);
            } else {
                retired.frames_left -= 1;
                still_retained.push(retired);
            }
        }
        self.retired_native_textures = still_retained;
    }

    fn update_frame_timing(&mut self) {
        let now = Instant::now();
        if let Some(last_frame) = self.last_frame_instant.replace(now) {
            let frame_time = (now - last_frame).as_secs_f32().clamp(1.0 / 240.0, 0.25);
            Self::update_smoothed_metric(&mut self.smoothed_frame_time, frame_time);
        }
    }

    fn fps_label(&self) -> String {
        match self.smoothed_frame_time {
            Some(frame_time) if frame_time > 0.0 => {
                format!("FPS: {:>3.0}  {:>4.1} ms", 1.0 / frame_time, frame_time * 1000.0)
            }
            _ => "FPS: ---".to_string(),
        }
    }

    fn update_smoothed_metric(slot: &mut Option<f32>, sample: f32) {
        *slot = Some(match *slot {
            Some(previous) => previous * 0.9 + sample * 0.1,
            None => sample,
        });
    }

    fn render_stats_label(&self) -> String {
        let hydra_ms = self
            .smoothed_hydra_render_time
            .map(|seconds| seconds * 1000.0)
            .unwrap_or(0.0);
        let present_ms = self
            .smoothed_viewport_present_time
            .map(|seconds| seconds * 1000.0)
            .unwrap_or(0.0);
        format!(
            "Hydra: {:>4.1} ms  Present: {:>4.1} ms  {}",
            hydra_ms,
            present_ms,
            self.viewport_present_path
        )
    }

    fn clamp_camera_lens(&mut self) {
        self.camera.fov = self.camera.fov.clamp(5.0_f32.to_radians(), 150.0_f32.to_radians());
        self.camera.near = self.camera.near.max(0.0001);
        self.camera.far = self.camera.far.max(self.camera.near + 0.001);
    }

    fn smooth_positive_toward(current: f32, target: f32, alpha: f32) -> f32 {
        if current <= 0.0 || target <= 0.0 {
            return target;
        }
        let alpha = alpha.clamp(0.0, 1.0);
        (current.ln() * (1.0 - alpha) + target.ln() * alpha).exp()
    }

    fn sync_manual_clip_from_camera(&mut self) {
        self.manual_clip_planes = (self.camera.near, self.camera.far);
    }

    fn restore_manual_clip(&mut self) {
        self.camera.near = self.manual_clip_planes.0;
        self.camera.far = self.manual_clip_planes.1;
        self.clamp_camera_lens();
    }

    fn invalidate_auto_clip(&mut self) {
        self.auto_clip_target = None;
        self.auto_clip_reference_distance = None;
        self.auto_clip_needs_recompute = self.auto_compute_clipping_planes;
    }

    fn update_auto_clip_target(&mut self, interactive: bool) {
        if !self.auto_compute_clipping_planes {
            self.auto_clip_target = None;
            self.auto_clip_reference_distance = None;
            self.auto_clip_needs_recompute = false;
            self.restore_manual_clip();
            return;
        }

        let camera_distance = (self.camera.eye - self.camera.target).length().max(0.001);
        if self.auto_clip_needs_recompute || self.auto_clip_target.is_none() {
            let target = self.hydra.as_ref().and_then(|hydra| {
                let _ = hydra.set_camera(
                    self.camera.eye_as_f64(),
                    self.camera.target_as_f64(),
                    self.camera.up_as_f64(),
                );
                hydra.compute_auto_clip().ok()
            });

            if let Some((near_plane, far_plane)) = target {
                let target = (near_plane as f32, far_plane as f32);
                self.auto_clip_target = Some(target);
                self.auto_clip_reference_distance = Some(camera_distance);
                self.auto_clip_needs_recompute = false;
            }
        } else if let Some(reference_distance) = self.auto_clip_reference_distance {
            let ratio = if reference_distance > 0.0 {
                camera_distance / reference_distance
            } else {
                1.0
            };
            if let Some((target_near, target_far)) = self.auto_clip_target.as_mut() {
                if (ratio - 1.0).abs() > 0.001 {
                    *target_near = (*target_near * ratio).max(1.0);
                    *target_far = (*target_far * ratio).max(*target_near + 1.0);
                    self.auto_clip_reference_distance = Some(camera_distance);
                }
            }
        }

        if let Some(target) = self.auto_clip_target {
            let alpha = if interactive { 0.28 } else { 0.16 };
            self.camera.near = Self::smooth_positive_toward(self.camera.near, target.0, alpha);
            self.camera.far = Self::smooth_positive_toward(self.camera.far, target.1, alpha);
            self.clamp_camera_lens();
        }
    }

    fn auto_clip_is_animating(&self) -> bool {
        let Some((target_near, target_far)) = self.auto_clip_target else {
            return false;
        };
        let near_error = (self.camera.near - target_near).abs() / target_near.max(1.0);
        let far_error = (self.camera.far - target_far).abs() / target_far.max(1.0);
        near_error > 0.01 || far_error > 0.01
    }

    fn reset_camera(&mut self) {
        self.reset_camera_to_stage_up_axis();
    }

    fn draw_renderer_aov_combo(
        &mut self,
        ui: &mut egui::Ui,
        id_salt: &'static str,
    ) {
        let Some((aovs, current_aov)) = self.hydra.as_ref().and_then(|hydra| {
            let aovs = hydra.list_renderer_aovs().ok()?;
            let current_aov = hydra.current_renderer_aov().ok()?;
            Some((aovs, current_aov))
        }) else {
            return;
        };

        if aovs.is_empty() {
            return;
        }

        let mut requested_aov = None;
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(format!("AOV: {current_aov}"))
            .show_ui(ui, |ui| {
                for aov in &aovs {
                    if ui.selectable_label(*aov == current_aov, aov).clicked() && *aov != current_aov
                    {
                        requested_aov = Some(aov.clone());
                        ui.close_menu();
                    }
                }
            });

        if let Some(aov) = requested_aov {
            if let Some(hydra) = self.hydra.as_ref() {
                match hydra.set_renderer_aov(&aov) {
                    Ok(()) => {
                        self.clear_viewport_texture();
                        self.status_message = format!("AOV: {aov}");
                    }
                    Err(err) => {
                        self.status_message = format!("AOV switch failed: {err}");
                    }
                }
            }
        }
    }

    fn draw_render_delegate_combo(
        &mut self,
        ui: &mut egui::Ui,
        id_salt: &'static str,
    ) {
        let Some((delegates, current_delegate)) = self.hydra.as_ref().map(|hydra| {
            (
                HydraEngine::list_render_delegates().unwrap_or_default(),
                hydra.current_render_delegate()
                    .unwrap_or_else(|_| "Unavailable".to_string()),
            )
        }) else {
            return;
        };

        let mut requested_delegate = None;
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(format!("Delegate: {current_delegate}"))
            .show_ui(ui, |ui| {
                for delegate in delegates {
                    let selected = delegate == current_delegate;
                    if ui.selectable_label(selected, &delegate).clicked() && !selected {
                        requested_delegate = Some(delegate.clone());
                        ui.close_menu();
                    }
                }
            });

        if let Some(delegate) = requested_delegate {
            if let Some(hydra) = self.hydra.as_ref() {
                match hydra.set_render_delegate(&delegate) {
                    Ok(()) => {
                        self.clear_viewport_texture();
                        self.renderer_setting_text_edits.clear();
                        self.status_message = format!("Render delegate: {delegate}");
                    }
                    Err(err) => {
                        self.status_message = format!("Delegate switch failed: {err}");
                    }
                }
            }
        }
    }

    fn show_renderer_settings_window(&mut self, ctx: &egui::Context) {
        if !self.renderer_settings_open {
            return;
        }

        let settings_result = self
            .hydra
            .as_ref()
            .ok_or_else(|| "Renderer unavailable".to_string())
            .and_then(|hydra| hydra.renderer_settings().map_err(|err| err.to_string()));

        let mut updates = Vec::new();
        let mut open = self.renderer_settings_open;
        egui::Window::new("Renderer Settings")
            .open(&mut open)
            .default_width(360.0)
            .show(ctx, |ui| match settings_result {
                Ok(ref settings) if !settings.is_empty() => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for setting in settings {
                            self.draw_renderer_setting_control(ui, setting, &mut updates);
                            ui.separator();
                        }
                    });
                }
                Ok(_) => {
                    ui.label("Current render delegate does not expose renderer settings.");
                }
                Err(ref err) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }
            });
        self.renderer_settings_open = open;

        if updates.is_empty() {
            return;
        }

        if let Some(hydra) = self.hydra.as_ref() {
            for update in updates {
                let result = match update {
                    RendererSettingUpdate::Bool(key, value) => {
                        hydra.set_renderer_setting_bool(&key, value)
                    }
                    RendererSettingUpdate::Int(key, value) => {
                        hydra.set_renderer_setting_int(&key, value)
                    }
                    RendererSettingUpdate::Float(key, value) => {
                        hydra.set_renderer_setting_float(&key, value)
                    }
                    RendererSettingUpdate::String(key, value) => {
                        hydra.set_renderer_setting_string(&key, &value)
                    }
                };
                if let Err(err) = result {
                    self.status_message = format!("Renderer setting failed: {err}");
                }
            }
        }
    }

    fn draw_renderer_setting_control(
        &mut self,
        ui: &mut egui::Ui,
        setting: &RendererSetting,
        updates: &mut Vec<RendererSettingUpdate>,
    ) {
        ui.label(egui::RichText::new(&setting.name).strong());
        ui.small(format!("{}  default: {}", setting.key, setting.default_value));

        match setting.setting_type {
            RendererSettingType::Flag => {
                let mut value = matches!(
                    setting.current_value.to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
                if ui.checkbox(&mut value, "Enabled").changed() {
                    updates.push(RendererSettingUpdate::Bool(setting.key.clone(), value));
                }
            }
            RendererSettingType::Int => {
                let mut value = setting.current_value.parse::<i32>().unwrap_or_default();
                if ui.add(egui::DragValue::new(&mut value).speed(1.0)).changed() {
                    updates.push(RendererSettingUpdate::Int(setting.key.clone(), value));
                }
            }
            RendererSettingType::Float => {
                let mut value = setting.current_value.parse::<f32>().unwrap_or_default();
                if ui
                    .add(egui::DragValue::new(&mut value).speed(0.01).max_decimals(4))
                    .changed()
                {
                    updates.push(RendererSettingUpdate::Float(setting.key.clone(), value));
                }
            }
            RendererSettingType::String => {
                let draft = self
                    .renderer_setting_text_edits
                    .entry(setting.key.clone())
                    .or_insert_with(|| setting.current_value.clone());
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(draft);
                    if ui.button("Apply").clicked() {
                        updates.push(RendererSettingUpdate::String(
                            setting.key.clone(),
                            draft.clone(),
                        ));
                    }
                    if ui.button("Reset").clicked() {
                        *draft = setting.default_value.clone();
                        updates.push(RendererSettingUpdate::String(
                            setting.key.clone(),
                            draft.clone(),
                        ));
                    }
                });
            }
        }
    }

    fn show_camera_settings_window(&mut self, ctx: &egui::Context) {
        if !self.camera_settings_open {
            return;
        }

        let mut open = self.camera_settings_open;
        egui::Window::new("Camera")
            .open(&mut open)
            .default_width(280.0)
            .show(ctx, |ui| {
                let mut changed = false;
                let mut fov_degrees = self.camera.fov.to_degrees();
                ui.horizontal(|ui| {
                    ui.label("FOV");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut fov_degrees)
                                .range(5.0..=150.0)
                                .speed(0.25)
                                .suffix(" deg"),
                        )
                        .changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Near");
                    changed |= ui
                        .add_enabled(
                            !self.auto_compute_clipping_planes,
                            egui::DragValue::new(&mut self.camera.near)
                                .range(0.0001..=1_000.0)
                                .speed(0.001),
                        )
                        .changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Far");
                    changed |= ui
                        .add_enabled(
                            !self.auto_compute_clipping_planes,
                            egui::DragValue::new(&mut self.camera.far)
                                .range(0.001..=2_000_000.0)
                                .speed(1.0),
                        )
                        .changed();
                });
                if changed {
                    self.camera.fov = fov_degrees.to_radians();
                    self.clamp_camera_lens();
                    if self.auto_compute_clipping_planes {
                        self.invalidate_auto_clip();
                    } else {
                        self.sync_manual_clip_from_camera();
                    }
                    self.viewport_interaction_frames = 2;
                    ctx.request_repaint();
                }

                if self.auto_compute_clipping_planes {
                    ui.small("Auto clipping is active. Near/Far are computed from the current view.");
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Frame Selected").clicked() {
                        self.focus_selected_prim();
                    }
                    if ui.button("Reset Camera").clicked() {
                        self.reset_camera();
                    }
                });
                ui.small("These controls drive the Hydra free camera lens and clipping planes.");
            });
        self.camera_settings_open = open;
    }

    fn viewport_scale_factor(&self) -> f32 {
        self.viewport_pixels_per_point.max(1.0) * self.aa_mode.render_scale()
    }

    fn viewport_render_size(&self, rect: egui::Rect) -> (u32, u32) {
        let scale = self.viewport_scale_factor();
        let width = (rect.width().max(1.0) * scale).round().max(1.0) as u32;
        let height = (rect.height().max(1.0) * scale).round().max(1.0) as u32;
        (width, height)
    }

    fn viewport_screen_to_render(&self, rect: egui::Rect, pos: egui::Pos2) -> (f64, f64) {
        let scale = self.viewport_scale_factor();
        let x = (pos.x - rect.left()).clamp(0.0, rect.width()) * scale;
        let y = (pos.y - rect.top()).clamp(0.0, rect.height()) * scale;
        (x as f64, y as f64)
    }

    fn viewport_image_rect(&self, rect: egui::Rect) -> egui::Rect {
        let Some((width, height)) = self.viewport_texture_size else {
            return rect;
        };
        if width == 0 || height == 0 {
            return rect;
        }

        let texture_aspect = width as f32 / height as f32;
        let rect_aspect = rect.width() / rect.height().max(1.0);

        if (texture_aspect - rect_aspect).abs() < 0.001 {
            return rect;
        }

        if texture_aspect > rect_aspect {
            let fitted_height = rect.width() / texture_aspect;
            let top = rect.center().y - fitted_height * 0.5;
            egui::Rect::from_min_size(
                egui::pos2(rect.left(), top),
                egui::vec2(rect.width(), fitted_height),
            )
        } else {
            let fitted_width = rect.height() * texture_aspect;
            let left = rect.center().x - fitted_width * 0.5;
            egui::Rect::from_min_size(
                egui::pos2(left, rect.top()),
                egui::vec2(fitted_width, rect.height()),
            )
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let editing_text = ctx.wants_keyboard_input() || ctx.memory(|m| m.focused().is_some());
        let pointer_over_viewport = ctx.input(|i| {
            i.pointer
                .hover_pos()
                .is_some_and(|pos| self.viewport_rect.contains(pos))
        });
        let allow_viewport_shortcuts = pointer_over_viewport && !editing_text;
        let ctrl_o =
            ctx.input(|i| {
                !editing_text
                    && i.key_pressed(egui::Key::O)
                    && i.modifiers.command
                    && !i.modifiers.shift
            });
        let ctrl_s =
            ctx.input(|i| {
                !editing_text
                    && i.key_pressed(egui::Key::S)
                    && i.modifiers.command
                    && !i.modifiers.shift
            });
        let ctrl_shift_s =
            ctx.input(|i| {
                !editing_text
                    && i.key_pressed(egui::Key::S)
                    && i.modifiers.command
                    && i.modifiers.shift
            });
        let ctrl_z =
            ctx.input(|i| {
                !editing_text
                    && i.key_pressed(egui::Key::Z)
                    && i.modifiers.command
                    && !i.modifiers.shift
            });
        let ctrl_shift_z =
            ctx.input(|i| {
                !editing_text
                    && i.key_pressed(egui::Key::Z)
                    && i.modifiers.command
                    && i.modifiers.shift
            });
        let key_q = ctx.input(|i| allow_viewport_shortcuts && i.key_pressed(egui::Key::Q) && !i.modifiers.command);
        let key_w = ctx.input(|i| allow_viewport_shortcuts && i.key_pressed(egui::Key::W) && !i.modifiers.command);
        let key_e = ctx.input(|i| allow_viewport_shortcuts && i.key_pressed(egui::Key::E) && !i.modifiers.command);
        let key_r = ctx.input(|i| allow_viewport_shortcuts && i.key_pressed(egui::Key::R) && !i.modifiers.command);
        let key_x = ctx.input(|i| allow_viewport_shortcuts && i.key_pressed(egui::Key::X) && !i.modifiers.command);
        let key_f = ctx.input(|i| allow_viewport_shortcuts && i.key_pressed(egui::Key::F) && !i.modifiers.command);
        let delete_selected = ctx.input(|i| {
            allow_viewport_shortcuts
                && (i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
        });

        if ctrl_o {
            self.open_file();
        } else if ctrl_shift_s {
            self.save_file_as();
        } else if ctrl_s {
            self.save_file();
        } else if ctrl_shift_z {
            self.redo();
        } else if ctrl_z {
            self.undo();
        }

        if key_q {
            self.gizmo_mode = GizmoMode::Select;
            self.dragging_handle = None;
        } else if key_w {
            self.gizmo_mode = GizmoMode::Translate;
        } else if key_e {
            self.gizmo_mode = GizmoMode::Rotate;
        } else if key_r {
            self.gizmo_mode = GizmoMode::Scale;
        } else if key_x {
            if self.gizmo_mode != GizmoMode::Scale {
                self.toggle_gizmo_space();
            }
        } else if key_f {
            self.focus_selected_prim();
        } else if delete_selected {
            self.delete_selected_prim();
        }
    }

    fn draw_axis_gizmo(&self, ui: &egui::Ui, viewport_rect: egui::Rect) {
        let painter = ui.painter();
        let axis_len = 40.0_f32;
        let margin = 50.0_f32;
        let center = egui::pos2(
            viewport_rect.left() + margin,
            viewport_rect.bottom() - margin,
        );

        // Compute camera-relative axis directions using camera vectors
        let eye = self.camera.eye;
        let target = self.camera.target;
        let up = self.camera.up;
        let forward = (target - eye).normalize();
        let right = forward.cross(up).normalize();
        let cam_up = right.cross(forward).normalize();

        // World axes with colors
        let world_axes: [(dreamusd_render::glam::Vec3, egui::Color32, &str); 3] = [
            (dreamusd_render::glam::Vec3::X, egui::Color32::from_rgb(230, 60, 60), "X"),
            (dreamusd_render::glam::Vec3::Y, egui::Color32::from_rgb(60, 200, 60), "Y"),
            (dreamusd_render::glam::Vec3::Z, egui::Color32::from_rgb(60, 100, 230), "Z"),
        ];

        // Sort by depth (draw far axes first)
        let mut sorted: Vec<_> = world_axes
            .iter()
            .map(|(dir, color, label)| {
                let screen_x = dir.dot(right);
                let screen_y = dir.dot(cam_up);
                let depth = dir.dot(forward);
                (screen_x, screen_y, depth, *color, *label)
            })
            .collect();
        sorted.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());

        for (sx, sy, _depth, color, label) in &sorted {
            let end = egui::pos2(
                center.x + sx * axis_len,
                center.y - sy * axis_len,
            );
            painter.line_segment([center, end], egui::Stroke::new(2.5, *color));
            painter.text(
                end,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(12.0),
                *color,
            );
        }
    }

    fn collect_lights(prim: &Prim, out: &mut Vec<LightInfo>) {
        let type_name = prim.type_name().unwrap_or_default();
        let name = prim.name().unwrap_or_default();
        if let Some(kind) = light_kind(&type_name, &name) {
            if let Ok(mat) = prim.get_local_matrix() {
                let pos = Vec3::new(mat[12] as f32, mat[13] as f32, mat[14] as f32);
                let path = prim.path().unwrap_or_default();
                out.push(LightInfo { pos, path, kind });
            }
        }
        if let Ok(children) = prim.children() {
            for child in children {
                DreamUsdApp::collect_lights(&child, out);
            }
        }
    }

    fn draw_light_icons(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        selected_path: Option<&str>,
    ) {
        let stage = match self.stage.as_ref() {
            Some(s) => s,
            None => return,
        };
        let root = match stage.root_prim() {
            Ok(r) => r,
            Err(_) => return,
        };

        let mut lights = Vec::new();
        DreamUsdApp::collect_lights(&root, &mut lights);

        let painter = ui.painter();

        for light in &lights {
            let Some(center) = self.hydra_project(light.pos, rect) else {
                continue;
            };
            let is_selected = selected_path == Some(light.path.as_str());
            let icon_color = if is_selected {
                egui::Color32::from_rgb(255, 220, 50)
            } else {
                egui::Color32::from_rgb(255, 200, 60)
            };
            let outline_color = egui::Color32::from_rgb(180, 140, 30);
            let stroke = egui::Stroke::new(if is_selected { 2.0 } else { 1.5 }, icon_color);
            let outline = egui::Stroke::new(1.5, outline_color);

            match light.kind {
                LightKind::Directional => {
                    // Parallel beams icon showing light direction
                    // Direction: from light position toward origin
                    let dir_3d = if light.pos.length() > 0.01 {
                        -light.pos.normalize()
                    } else {
                        Vec3::new(0.0, -1.0, 0.0)
                    };
                    // Project direction to screen
                    let end_3d = light.pos + dir_3d * 2.0;
                    let dir_2d = if let Some(end_2d) = self.hydra_project(end_3d, rect) {
                        Self::normalize_screen_vec(end_2d - center)
                    } else {
                        egui::vec2(0.0, 1.0)
                    };
                    let perp = egui::vec2(-dir_2d.y, dir_2d.x);

                    // Draw circle (sun body)
                    let r = if is_selected { 8.0 } else { 6.0 };
                    painter.circle_filled(center, r, icon_color);
                    painter.circle_stroke(center, r, outline);

                    // Draw 3 parallel beam lines
                    let beam_start = r + 3.0;
                    let beam_len = 18.0;
                    for offset in [-6.0_f32, 0.0, 6.0] {
                        let base = center + perp * offset;
                        let s = base + dir_2d * beam_start;
                        let e = base + dir_2d * (beam_start + beam_len);
                        painter.line_segment([s, e], stroke);
                        // Arrowhead
                        let arrow = 4.0_f32;
                        let t1 = e - dir_2d * arrow + perp * arrow * 0.4;
                        let t2 = e - dir_2d * arrow - perp * arrow * 0.4;
                        painter.add(egui::Shape::convex_polygon(
                            vec![e, t1, t2],
                            icon_color,
                            egui::Stroke::NONE,
                        ));
                    }
                }

                LightKind::Point => {
                    // Light bulb icon
                    let r = if is_selected { 9.0 } else { 7.0 };
                    // Bulb (circle)
                    painter.circle_filled(center, r, icon_color);
                    painter.circle_stroke(center, r, outline);
                    // Base (small rectangle below)
                    let base_top = center.y + r * 0.6;
                    let base_w = r * 0.6;
                    let base_h = r * 0.5;
                    painter.rect_filled(
                        egui::Rect::from_center_size(
                            egui::pos2(center.x, base_top + base_h * 0.5),
                            egui::vec2(base_w * 2.0, base_h),
                        ),
                        2.0,
                        outline_color,
                    );
                    // Rays
                    let ray_len = r * 0.6;
                    let ray_gap = r + 2.0;
                    for i in 0..6 {
                        let angle = i as f32 * std::f32::consts::FRAC_PI_3 - std::f32::consts::FRAC_PI_2;
                        let dx = angle.cos();
                        let dy = angle.sin();
                        // Skip rays in the bottom direction (base area)
                        if dy > 0.5 { continue; }
                        let s = egui::pos2(center.x + dx * ray_gap, center.y + dy * ray_gap);
                        let e = egui::pos2(
                            center.x + dx * (ray_gap + ray_len),
                            center.y + dy * (ray_gap + ray_len),
                        );
                        painter.line_segment([s, e], stroke);
                    }
                }

                LightKind::Dome => {
                    // Hemisphere icon
                    let r = if is_selected { 12.0 } else { 9.0 };
                    // Draw arc (top half of circle)
                    let n_segs = 20;
                    let mut points = Vec::with_capacity(n_segs + 2);
                    for i in 0..=n_segs {
                        let angle = std::f32::consts::PI * (i as f32 / n_segs as f32);
                        points.push(egui::pos2(
                            center.x + angle.cos() * r,
                            center.y - angle.sin() * r,
                        ));
                    }
                    // Close with baseline
                    let baseline_left = egui::pos2(center.x - r, center.y);
                    let baseline_right = egui::pos2(center.x + r, center.y);
                    painter.line_segment([baseline_left, baseline_right], stroke);
                    // Draw arc segments
                    for w in points.windows(2) {
                        painter.line_segment([w[0], w[1]], stroke);
                    }
                    // Small downward arrows around the dome
                    let arrow_len = 6.0_f32;
                    for i in [0.3, 0.5, 0.7] {
                        let angle = std::f32::consts::PI * i;
                        let ax = center.x + angle.cos() * (r + 4.0);
                        let ay = center.y - angle.sin() * (r + 4.0);
                        let s = egui::pos2(ax, ay);
                        let e = egui::pos2(ax, ay + arrow_len);
                        painter.line_segment([s, e], stroke);
                        painter.line_segment(
                            [e, egui::pos2(e.x - 2.0, e.y - 3.0)],
                            stroke,
                        );
                        painter.line_segment(
                            [e, egui::pos2(e.x + 2.0, e.y - 3.0)],
                            stroke,
                        );
                    }
                }

                LightKind::Area => {
                    // Rectangle icon
                    let w = if is_selected { 16.0 } else { 12.0 };
                    let h = w * 0.7;
                    let area_rect = egui::Rect::from_center_size(center, egui::vec2(w, h));
                    painter.rect_filled(area_rect, 2.0, icon_color);
                    painter.rect_stroke(area_rect, 2.0, outline, egui::StrokeKind::Outside);
                    // Rays from center
                    let ray_len = 8.0_f32;
                    for angle in [0.0_f32, 0.8, -0.8] {
                        let dx = angle.sin();
                        let dy = angle.cos();
                        let s = egui::pos2(center.x + dx * (h * 0.5 + 2.0), center.y + dy * (h * 0.5 + 2.0));
                        let e = egui::pos2(center.x + dx * (h * 0.5 + ray_len), center.y + dy * (h * 0.5 + ray_len));
                        painter.line_segment([s, e], stroke);
                    }
                }
            }
        }
    }

    fn get_prim_position(&self, prim: &Prim) -> Option<Vec3> {
        selection::prim_position(prim)
    }

    fn local_gizmo_axes(&self, prim: &Prim) -> [Vec3; 3] {
        let fallback = [Vec3::X, Vec3::Y, Vec3::Z];
        let Ok(matrix) = prim.get_world_matrix() else {
            return fallback;
        };
        let Some(world_rotation) = Self::quat_from_matrix(matrix) else {
            return fallback;
        };
        [
            world_rotation * Vec3::X,
            world_rotation * Vec3::Y,
            world_rotation * Vec3::Z,
        ]
    }

    fn effective_gizmo_space(&self) -> GizmoSpace {
        if self.gizmo_mode == GizmoMode::Scale {
            GizmoSpace::Local
        } else {
            self.gizmo_space
        }
    }

    fn gizmo_axes(&self, prim: &Prim) -> [Vec3; 3] {
        match self.effective_gizmo_space() {
            GizmoSpace::Local => self.local_gizmo_axes(prim),
            GizmoSpace::World => [Vec3::X, Vec3::Y, Vec3::Z],
        }
    }

    fn canonical_axis(axis: usize) -> Vec3 {
        match axis {
            0 => Vec3::X,
            1 => Vec3::Y,
            _ => Vec3::Z,
        }
    }

    fn plane_handles() -> [(usize, usize); 3] {
        [(0, 1), (1, 2), (0, 2)]
    }

    fn quat_from_matrix(matrix: [f64; 16]) -> Option<Quat> {
        let original_x = Vec3::new(matrix[0] as f32, matrix[1] as f32, matrix[2] as f32);
        let original_y = Vec3::new(matrix[4] as f32, matrix[5] as f32, matrix[6] as f32);
        let original_z = Vec3::new(matrix[8] as f32, matrix[9] as f32, matrix[10] as f32);
        if original_x.length_squared() <= 1.0e-6
            || original_y.length_squared() <= 1.0e-6
            || original_z.length_squared() <= 1.0e-6
        {
            return None;
        }

        let x = original_x.normalize();
        let mut y = original_y - x * x.dot(original_y);
        if y.length_squared() <= 1.0e-6 {
            y = original_z.cross(x);
        }
        if y.length_squared() <= 1.0e-6 {
            return None;
        }
        y = y.normalize();

        let mut z = x.cross(y);
        if z.length_squared() <= 1.0e-6 {
            return None;
        }
        z = z.normalize();
        if z.dot(original_z) < 0.0 {
            z = -z;
            y = -y;
        }

        Some(Quat::from_mat3(&dreamusd_render::glam::Mat3::from_cols(x, y, z)))
    }

    fn rotation_order_to_euler(order: i32) -> EulerRot {
        match order {
            1 => EulerRot::XZY,
            2 => EulerRot::YXZ,
            3 => EulerRot::YZX,
            4 => EulerRot::ZXY,
            5 => EulerRot::ZYX,
            _ => EulerRot::XYZ,
        }
    }

    fn euler_order_for_prim(prim: &Prim) -> EulerRot {
        prim.get_rotation_order()
            .map(Self::rotation_order_to_euler)
            .unwrap_or(EulerRot::XYZ)
    }

    fn plane_handle_quad(
        &self,
        rect: egui::Rect,
        world_pos: Vec3,
        axis_a: Vec3,
        axis_b: Vec3,
        axis_len: f32,
    ) -> Option<[egui::Pos2; 4]> {
        let inner = axis_len * 0.18;
        let outer = axis_len * 0.38;
        Some([
            self.hydra_project(world_pos + axis_a * inner + axis_b * inner, rect)?,
            self.hydra_project(world_pos + axis_a * outer + axis_b * inner, rect)?,
            self.hydra_project(world_pos + axis_a * outer + axis_b * outer, rect)?,
            self.hydra_project(world_pos + axis_a * inner + axis_b * outer, rect)?,
        ])
    }

    fn plane_screen_basis(
        &self,
        rect: egui::Rect,
        world_pos: Vec3,
        axis_a: Vec3,
        axis_b: Vec3,
        axis_len: f32,
    ) -> Option<(egui::Vec2, egui::Vec2)> {
        let center = self.hydra_project(world_pos, rect)?;
        let axis_a_end = self.hydra_project(world_pos + axis_a * axis_len, rect)?;
        let axis_b_end = self.hydra_project(world_pos + axis_b * axis_len, rect)?;
        Some((axis_a_end - center, axis_b_end - center))
    }

    fn point_in_triangle(
        point: egui::Pos2,
        a: egui::Pos2,
        b: egui::Pos2,
        c: egui::Pos2,
    ) -> bool {
        let sign = |p1: egui::Pos2, p2: egui::Pos2, p3: egui::Pos2| {
            (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
        };
        let d1 = sign(point, a, b);
        let d2 = sign(point, b, c);
        let d3 = sign(point, c, a);
        let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(has_neg && has_pos)
    }

    fn point_in_quad(point: egui::Pos2, quad: &[egui::Pos2; 4]) -> bool {
        Self::point_in_triangle(point, quad[0], quad[1], quad[2])
            || Self::point_in_triangle(point, quad[0], quad[2], quad[3])
    }

    fn normalize_screen_vec(vec: egui::Vec2) -> egui::Vec2 {
        let len = vec.length();
        if len > 1.0e-4 {
            vec / len
        } else {
            egui::Vec2::ZERO
        }
    }

    fn solve_plane_drag(
        &self,
        axis_a_screen: egui::Vec2,
        axis_b_screen: egui::Vec2,
        pointer_delta: egui::Vec2,
    ) -> Option<(f32, f32)> {
        let det = axis_a_screen.x * axis_b_screen.y - axis_a_screen.y * axis_b_screen.x;
        if det.abs() <= 1.0e-4 {
            return None;
        }
        let u = (pointer_delta.x * axis_b_screen.y - pointer_delta.y * axis_b_screen.x) / det;
        let v = (axis_a_screen.x * pointer_delta.y - axis_a_screen.y * pointer_delta.x) / det;
        Some((u, v))
    }

    fn apply_axis_rotation(&self, prim: &Prim, axis: usize, angle_deg: f32) {
        let Some(local_start) = self
            .drag_start_local_rotation
            .or_else(|| prim.get_local_matrix().ok().and_then(Self::quat_from_matrix))
        else {
            return;
        };

        let angle_radians = angle_deg.to_radians();
        let new_local = match self.effective_gizmo_space() {
            GizmoSpace::Local => {
                let local_delta = Quat::from_axis_angle(Self::canonical_axis(axis), angle_radians);
                (local_start * local_delta).normalize()
            }
            GizmoSpace::World => {
                let Some(world_start) = self
                    .drag_start_world_rotation
                    .or_else(|| prim.get_world_matrix().ok().and_then(Self::quat_from_matrix))
                else {
                    return;
                };
                let world_delta = Quat::from_axis_angle(Self::canonical_axis(axis), angle_radians);
                let parent_world = (world_start * local_start.inverse()).normalize();
                (parent_world.inverse() * (world_delta * world_start)).normalize()
            }
        };

        let (x, y, z) = new_local.to_euler(Self::euler_order_for_prim(prim));
        let _ = prim.set_rotate(x.to_degrees() as f64, y.to_degrees() as f64, z.to_degrees() as f64);
    }

    /// Project a world-space point to screen coordinates using the Hydra engine's
    /// exact view/projection matrices for perfect alignment with the rendered scene.
    fn hydra_project(
        &self,
        world_pos: Vec3,
        rect: egui::Rect,
    ) -> Option<egui::Pos2> {
        let hydra = self.hydra.as_ref()?;
        let (w, h) = self.viewport_render_size(rect);
        let (sx, sy) = hydra.project_point(
            [world_pos.x as f64, world_pos.y as f64, world_pos.z as f64],
            w, h,
        )?;
        let scale = self.viewport_scale_factor();
        Some(egui::pos2(
            rect.left() + sx as f32 / scale,
            rect.top() + sy as f32 / scale,
        ))
    }

    /// Detect which gizmo handle the mouse is hovering over (no drawing).
    fn detect_hovered_handle(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        world_pos: Vec3,
        axis_dirs: [Vec3; 3],
    ) -> Option<GizmoHandle> {
        let axis_len = self.gizmo_axis_length(world_pos);
        let mouse_pos = ui.input(|i| i.pointer.hover_pos()).unwrap_or(egui::Pos2::ZERO);
        let center_2d = self.hydra_project(world_pos, rect)?;

        if matches!(self.gizmo_mode, GizmoMode::Translate | GizmoMode::Scale) {
            let center_rect = egui::Rect::from_center_size(center_2d, egui::vec2(14.0, 14.0));
            if center_rect.contains(mouse_pos) {
                return Some(GizmoHandle::Center);
            }

            for (axis_a, axis_b) in Self::plane_handles() {
                if let Some(quad) = self.plane_handle_quad(
                    rect,
                    world_pos,
                    axis_dirs[axis_a],
                    axis_dirs[axis_b],
                    axis_len,
                ) {
                    let edge_hit = Self::distance_to_polyline(mouse_pos, &quad, true)
                        .is_some_and(|distance| distance < 6.0);
                    if Self::point_in_quad(mouse_pos, &quad) || edge_hit {
                        return Some(GizmoHandle::Plane(axis_a, axis_b));
                    }
                }
            }
        }

        for (i, dir) in axis_dirs.iter().enumerate() {
            if let Some((center_2d, end_2d)) =
                self.axis_screen_segment(rect, world_pos, *dir, axis_len)
            {
                let hovered = match self.gizmo_mode {
                    GizmoMode::Select => false,
                    GizmoMode::Translate => {
                        Self::distance_to_segment(mouse_pos, center_2d, end_2d)
                            .is_some_and(|distance| distance < 8.0)
                    }
                    GizmoMode::Rotate => self
                        .projected_rotation_ring(rect, world_pos, axis_dirs, i, axis_len)
                        .and_then(|points| Self::distance_to_polyline(mouse_pos, &points, true))
                        .is_some_and(|distance| distance < 10.0),
                    GizmoMode::Scale => {
                        egui::Rect::from_center_size(end_2d, egui::vec2(12.0, 12.0))
                            .contains(mouse_pos)
                    }
                };
                if hovered {
                    return Some(GizmoHandle::Axis(i));
                }
            }
        }
        None
    }

    fn gizmo_axis_length(&self, world_pos: Vec3) -> f32 {
        let cam_dist = (self.camera.eye - world_pos).length();
        match self.gizmo_mode {
            GizmoMode::Select => cam_dist * 0.15,
            GizmoMode::Translate | GizmoMode::Scale => cam_dist * 0.15,
            GizmoMode::Rotate => cam_dist * 0.12,
        }
    }

    fn axis_screen_segment(
        &self,
        rect: egui::Rect,
        world_pos: Vec3,
        axis_dir: Vec3,
        axis_len: f32,
    ) -> Option<(egui::Pos2, egui::Pos2)> {
        let start = self.hydra_project(world_pos, rect)?;
        let end = self.hydra_project(world_pos + axis_dir * axis_len, rect)?;
        Some((start, end))
    }

    fn projected_rotation_ring(
        &self,
        rect: egui::Rect,
        world_pos: Vec3,
        axis_dirs: [Vec3; 3],
        axis_index: usize,
        radius: f32,
    ) -> Option<Vec<egui::Pos2>> {
        let basis_a = axis_dirs[(axis_index + 1) % 3];
        let basis_b = axis_dirs[(axis_index + 2) % 3];
        let mut points = Vec::with_capacity(64);
        for step in 0..64 {
            let angle = (step as f32 / 64.0) * std::f32::consts::TAU;
            let world_point =
                world_pos + basis_a * angle.cos() * radius + basis_b * angle.sin() * radius;
            points.push(self.hydra_project(world_point, rect)?);
        }
        Some(points)
    }

    fn distance_to_polyline(
        point: egui::Pos2,
        points: &[egui::Pos2],
        closed: bool,
    ) -> Option<f32> {
        if points.len() < 2 {
            return None;
        }
        let mut best: Option<f32> = None;
        for segment in points.windows(2) {
            if let Some(distance) = Self::distance_to_segment(point, segment[0], segment[1]) {
                best = Some(best.map_or(distance, |current| current.min(distance)));
            }
        }
        if closed {
            if let Some(distance) =
                Self::distance_to_segment(point, *points.last()?, points[0])
            {
                best = Some(best.map_or(distance, |current| current.min(distance)));
            }
        }
        best
    }

    fn distance_to_segment(
        point: egui::Pos2,
        start: egui::Pos2,
        end: egui::Pos2,
    ) -> Option<f32> {
        let segment = end - start;
        let len_sq = segment.length_sq();
        if len_sq <= 1.0 {
            return None;
        }
        let t = ((point - start).dot(segment) / len_sq).clamp(0.0, 1.0);
        let closest = start + segment * t;
        Some(point.distance(closest))
    }

    fn draw_translate_gizmo(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        world_pos: Vec3,
        axis_dirs: [Vec3; 3],
        hovered_handle: Option<GizmoHandle>,
    ) {
        let painter = ui.painter();
        let cam_dist = (self.camera.eye - world_pos).length();
        let axis_len = cam_dist * 0.15;

        let axes: [(Vec3, egui::Color32); 3] = [
            (axis_dirs[0], egui::Color32::from_rgb(230, 60, 60)),
            (axis_dirs[1], egui::Color32::from_rgb(60, 200, 60)),
            (axis_dirs[2], egui::Color32::from_rgb(60, 100, 230)),
        ];

        let center_2d = match self.hydra_project(world_pos, rect) {
            Some(p) => p,
            None => return,
        };
        let center_active =
            self.dragging_handle == Some(GizmoHandle::Center)
                || hovered_handle == Some(GizmoHandle::Center);

        painter.rect_filled(
            egui::Rect::from_center_size(
                center_2d,
                egui::vec2(if center_active { 12.0 } else { 10.0 }, if center_active { 12.0 } else { 10.0 }),
            ),
            2.0,
            if center_active {
                egui::Color32::YELLOW
            } else {
                egui::Color32::from_rgb(230, 230, 120)
            },
        );

        for (axis_a, axis_b) in Self::plane_handles() {
            if let Some(quad) = self.plane_handle_quad(
                rect,
                world_pos,
                axis_dirs[axis_a],
                axis_dirs[axis_b],
                axis_len,
            ) {
                let is_active = self.dragging_handle == Some(GizmoHandle::Plane(axis_a, axis_b))
                    || hovered_handle == Some(GizmoHandle::Plane(axis_a, axis_b));
                let plane_color = match (axis_a, axis_b) {
                    (0, 1) => egui::Color32::from_rgba_unmultiplied(235, 210, 72, if is_active { 112 } else { 72 }),
                    (1, 2) => egui::Color32::from_rgba_unmultiplied(72, 225, 160, if is_active { 112 } else { 72 }),
                    _ => egui::Color32::from_rgba_unmultiplied(120, 170, 255, if is_active { 112 } else { 72 }),
                };
                let stroke_color = if is_active {
                    egui::Color32::YELLOW
                } else {
                    plane_color.gamma_multiply(1.8)
                };
                painter.add(egui::Shape::convex_polygon(
                    quad.to_vec(),
                    plane_color,
                    egui::Stroke::new(if is_active { 2.0 } else { 1.0 }, stroke_color),
                ));
            }
        }

        for (i, (dir, color)) in axes.iter().enumerate() {
            let end_world = world_pos + *dir * axis_len;
            if let Some(end_2d) = self.hydra_project(end_world, rect) {
                let is_active = self.dragging_handle == Some(GizmoHandle::Axis(i))
                    || hovered_handle == Some(GizmoHandle::Axis(i));
                let stroke_width = if is_active { 4.0 } else { 2.5 };
                let draw_color = if is_active {
                    egui::Color32::YELLOW
                } else {
                    *color
                };

                painter.line_segment([center_2d, end_2d], egui::Stroke::new(stroke_width, draw_color));

                // Arrow head
                let arrow_size = 8.0_f32;
                let dir_2d = Self::normalize_screen_vec(end_2d - center_2d);
                let perp = egui::vec2(-dir_2d.y, dir_2d.x);
                let tip1 = end_2d - dir_2d * arrow_size + perp * arrow_size * 0.4;
                let tip2 = end_2d - dir_2d * arrow_size - perp * arrow_size * 0.4;
                painter.add(egui::Shape::convex_polygon(
                    vec![end_2d, tip1, tip2],
                    draw_color,
                    egui::Stroke::NONE,
                ));

                // Axis label
                let labels = ["X", "Y", "Z"];
                painter.text(
                    end_2d + dir_2d * 12.0,
                    egui::Align2::CENTER_CENTER,
                    labels[i],
                    egui::FontId::proportional(11.0),
                    draw_color,
                );
            }
        }
    }

    fn draw_selection_marker(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        prim: &Prim,
    ) {
        let Some(world_pos) = self.get_prim_position(prim) else {
            return;
        };
        let Some(center) = self.hydra_project(world_pos, rect) else {
            return;
        };

        let name = prim.name().unwrap_or_else(|_| "Selected".to_string());
        let painter = ui.painter();
        let outer = usdview_selection_yellow();
        let inner = usdview_selection_yellow_fill();
        let shadow = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 120);

        painter.circle_filled(center, 14.0, inner);
        painter.circle_stroke(center, 14.0, egui::Stroke::new(2.5, outer));
        painter.circle_stroke(center, 7.0, egui::Stroke::new(1.5, outer));
        painter.line_segment(
            [center + egui::vec2(-18.0, 0.0), center + egui::vec2(-8.0, 0.0)],
            egui::Stroke::new(2.0, outer),
        );
        painter.line_segment(
            [center + egui::vec2(8.0, 0.0), center + egui::vec2(18.0, 0.0)],
            egui::Stroke::new(2.0, outer),
        );
        painter.line_segment(
            [center + egui::vec2(0.0, -18.0), center + egui::vec2(0.0, -8.0)],
            egui::Stroke::new(2.0, outer),
        );
        painter.line_segment(
            [center + egui::vec2(0.0, 8.0), center + egui::vec2(0.0, 18.0)],
            egui::Stroke::new(2.0, outer),
        );

        let text_pos = center + egui::vec2(0.0, -28.0);
        let galley = painter.layout_no_wrap(
            name,
            egui::FontId::proportional(12.0),
            egui::Color32::WHITE,
        );
        let label_rect = egui::Rect::from_center_size(
            text_pos,
            galley.size() + egui::vec2(14.0, 8.0),
        );
        painter.rect_filled(label_rect.translate(egui::vec2(0.0, 1.0)), 6.0, shadow);
        painter.rect_filled(
            label_rect,
            6.0,
            egui::Color32::from_rgba_unmultiplied(32, 32, 36, 220),
        );
        painter.galley(
            label_rect.center() - galley.size() * 0.5,
            galley,
            egui::Color32::WHITE,
        );
    }

    fn draw_rotate_gizmo(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        world_pos: Vec3,
        axis_dirs: [Vec3; 3],
        hovered_handle: Option<GizmoHandle>,
    ) {
        let painter = ui.painter();
        let cam_dist = (self.camera.eye - world_pos).length();
        let axis_len = cam_dist * 0.12;
        let colors = [
            egui::Color32::from_rgb(230, 60, 60),
            egui::Color32::from_rgb(60, 200, 60),
            egui::Color32::from_rgb(60, 100, 230),
        ];

        for (i, color) in colors.iter().enumerate() {
            if let Some(points) = self.projected_rotation_ring(rect, world_pos, axis_dirs, i, axis_len) {
                let is_active = self.dragging_handle == Some(GizmoHandle::Axis(i))
                    || hovered_handle == Some(GizmoHandle::Axis(i));
                let draw_color = if is_active {
                    egui::Color32::YELLOW
                } else {
                    *color
                };
                let stroke_width = if is_active { 4.0 } else { 2.5 };

                painter.add(egui::Shape::closed_line(
                    points.clone(),
                    egui::Stroke::new(stroke_width, draw_color),
                ));

                if let Some(label_point) = points
                    .iter()
                    .min_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
                {
                    painter.text(
                        *label_point + egui::vec2(0.0, -12.0),
                        egui::Align2::CENTER_CENTER,
                        ["X", "Y", "Z"][i],
                        egui::FontId::proportional(11.0),
                        draw_color,
                    );
                }
            }
        }
    }

    fn draw_scale_gizmo(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        world_pos: Vec3,
        axis_dirs: [Vec3; 3],
        hovered_handle: Option<GizmoHandle>,
    ) {
        let painter = ui.painter();
        let cam_dist = (self.camera.eye - world_pos).length();
        let axis_len = cam_dist * 0.15;
        let axes: [(Vec3, egui::Color32); 3] = [
            (axis_dirs[0], egui::Color32::from_rgb(230, 60, 60)),
            (axis_dirs[1], egui::Color32::from_rgb(60, 200, 60)),
            (axis_dirs[2], egui::Color32::from_rgb(60, 100, 230)),
        ];

        if let Some(center_2d) = self.hydra_project(world_pos, rect) {
            let center_active =
                self.dragging_handle == Some(GizmoHandle::Center)
                    || hovered_handle == Some(GizmoHandle::Center);
            painter.rect_filled(
                egui::Rect::from_center_size(
                    center_2d,
                    egui::vec2(if center_active { 12.0 } else { 10.0 }, if center_active { 12.0 } else { 10.0 }),
                ),
                2.0,
                if center_active {
                    egui::Color32::YELLOW
                } else {
                    egui::Color32::from_rgb(230, 230, 120)
                },
            );
        }

        for (axis_a, axis_b) in Self::plane_handles() {
            if let Some(quad) = self.plane_handle_quad(
                rect,
                world_pos,
                axis_dirs[axis_a],
                axis_dirs[axis_b],
                axis_len,
            ) {
                let is_active = self.dragging_handle == Some(GizmoHandle::Plane(axis_a, axis_b))
                    || hovered_handle == Some(GizmoHandle::Plane(axis_a, axis_b));
                painter.add(egui::Shape::convex_polygon(
                    quad.to_vec(),
                    egui::Color32::from_rgba_unmultiplied(
                        220,
                        220,
                        220,
                        if is_active { 80 } else { 44 },
                    ),
                    egui::Stroke::new(
                        if is_active { 2.0 } else { 1.0 },
                        if is_active {
                            egui::Color32::YELLOW
                        } else {
                            egui::Color32::from_gray(210)
                        },
                    ),
                ));
            }
        }

        for (i, (dir, color)) in axes.iter().enumerate() {
            if let Some((center_2d, end_2d)) =
                self.axis_screen_segment(rect, world_pos, *dir, axis_len)
            {
                let is_active = self.dragging_handle == Some(GizmoHandle::Axis(i))
                    || hovered_handle == Some(GizmoHandle::Axis(i));
                let draw_color = if is_active {
                    egui::Color32::YELLOW
                } else {
                    *color
                };
                let stroke_width = if is_active { 4.0 } else { 2.5 };
                let handle_size = if is_active { 10.0 } else { 8.0 };

                painter.line_segment(
                    [center_2d, end_2d],
                    egui::Stroke::new(stroke_width, draw_color),
                );
                painter.rect_filled(
                    egui::Rect::from_center_size(end_2d, egui::vec2(handle_size, handle_size)),
                    1.5,
                    draw_color,
                );
                painter.text(
                    end_2d + Self::normalize_screen_vec(end_2d - center_2d) * 14.0,
                    egui::Align2::CENTER_CENTER,
                    ["X", "Y", "Z"][i],
                    egui::FontId::proportional(11.0),
                    draw_color,
                );
            }
        }
    }

    fn gizmo_drag_amount(
        &self,
        rect: egui::Rect,
        world_pos: Vec3,
        axis_dir: Vec3,
    ) -> Option<(f32, f32, egui::Vec2)> {
        let axis_len = self.gizmo_axis_length(world_pos);
        let (p0, p1) = self.axis_screen_segment(rect, world_pos, axis_dir, axis_len)?;
        let screen_axis = p1 - p0;
        let screen_axis_len = screen_axis.length();
        if screen_axis_len <= 0.1 {
            return None;
        }
        Some((axis_len, screen_axis_len, screen_axis / screen_axis_len))
    }

    fn snap_translate_amount(amount: f32) -> f32 {
        let step = 0.25_f32;
        (amount / step).round() * step
    }

    fn snap_rotation_degrees(angle: f64) -> f64 {
        let step = 15.0_f64;
        (angle / step).round() * step
    }

    fn snap_scale_factor(factor: f64) -> f64 {
        let base = 1.1_f64;
        let snapped_power = (factor.ln() / base.ln()).round();
        base.powf(snapped_power).max(0.001)
    }

    fn draw_drag_overlay(&self, ui: &egui::Ui, rect: egui::Rect, prim: &Prim) {
        let Some(handle) = self.dragging_handle else {
            return;
        };

        let value_text = match self.gizmo_mode {
            GizmoMode::Select => Ok("Selected".to_string()),
            GizmoMode::Translate => prim
                .get_translate()
                .map(|v| format!("T {:.2}, {:.2}, {:.2}", v[0], v[1], v[2])),
            GizmoMode::Rotate => prim
                .get_rotate()
                .map(|v| format!("R {:.1}, {:.1}, {:.1}", v[0], v[1], v[2])),
            GizmoMode::Scale => prim
                .get_scale()
                .map(|v| format!("S {:.3}, {:.3}, {:.3}", v[0], v[1], v[2])),
        }
        .unwrap_or_else(|_| "Editing".to_string());

        let handle_text = match handle {
            GizmoHandle::Axis(0) => "X",
            GizmoHandle::Axis(1) => "Y",
            GizmoHandle::Axis(2) => "Z",
            GizmoHandle::Plane(0, 1) => "XY",
            GizmoHandle::Plane(1, 2) => "YZ",
            GizmoHandle::Plane(0, 2) => "XZ",
            GizmoHandle::Plane(_, _) => "Plane",
            GizmoHandle::Center => "Center",
            GizmoHandle::Axis(_) => "Axis",
        };
        let mode_text = match self.gizmo_mode {
            GizmoMode::Select => "Select",
            GizmoMode::Translate => "Move",
            GizmoMode::Rotate => "Rotate",
            GizmoMode::Scale => "Scale",
        };
        let overlay_text = format!(
            "{mode_text} {handle_text} [{}]  {value_text}",
            self.effective_gizmo_space().label()
        );

        let painter = ui.painter();
        let galley = painter.layout_no_wrap(
            overlay_text,
            egui::FontId::proportional(13.0),
            egui::Color32::WHITE,
        );
        let panel_rect = egui::Rect::from_min_size(
            rect.left_bottom() + egui::vec2(16.0, -52.0),
            galley.size() + egui::vec2(18.0, 12.0),
        );
        painter.rect_filled(
            panel_rect,
            8.0,
            egui::Color32::from_rgba_unmultiplied(24, 24, 28, 220),
        );
        painter.rect_stroke(
            panel_rect,
            8.0,
            egui::Stroke::new(1.0, usdview_selection_yellow()),
            egui::StrokeKind::Outside,
        );
        painter.galley(
            panel_rect.min + egui::vec2(9.0, 6.0),
            galley,
            egui::Color32::WHITE,
        );
    }

    fn handle_gizmo_drag(
        &mut self,
        response: &egui::Response,
        selected_prim: &Option<Prim>,
        rect: egui::Rect,
        hovered_handle: Option<GizmoHandle>,
    ) {
        if self.gizmo_mode == GizmoMode::Select {
            return;
        }

        let prim = match selected_prim {
            Some(p) => p,
            None => return,
        };

        let prim_pos = match self.get_prim_position(prim) {
            Some(p) => p,
            None => return,
        };
        let axis_dirs = self.gizmo_axes(prim);

        // Start drag
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(handle) = hovered_handle {
                if let Some(ref stage) = self.stage {
                    let _ = stage.undo_begin();
                }
                let (world_axis_len, screen_axis_len, screen_dir) = match handle {
                    GizmoHandle::Axis(axis) => {
                        self.gizmo_drag_amount(rect, prim_pos, axis_dirs[axis])
                            .unwrap_or((1.0, 1.0, egui::vec2(1.0, 0.0)))
                    }
                    GizmoHandle::Plane(axis_a, axis_b) => {
                        let axis_len = self.gizmo_axis_length(prim_pos);
                        let (axis_a_screen, axis_b_screen) = self
                            .plane_screen_basis(rect, prim_pos, axis_dirs[axis_a], axis_dirs[axis_b], axis_len)
                            .unwrap_or((egui::vec2(1.0, 0.0), egui::vec2(0.0, 1.0)));
                        let diagonal = Self::normalize_screen_vec(
                            Self::normalize_screen_vec(axis_a_screen)
                                + Self::normalize_screen_vec(axis_b_screen),
                        );
                        (axis_len, diagonal.length().max(1.0), diagonal)
                    }
                    GizmoHandle::Center => (1.0, 1.0, egui::vec2(1.0, 0.0)),
                };
                self.dragging_handle = Some(handle);
                self.drag_start_pos = Some(prim_pos);
                self.drag_start_values = match self.gizmo_mode {
                    GizmoMode::Select => None,
                    GizmoMode::Translate => None,
                    GizmoMode::Rotate => prim.get_rotate().ok(),
                    GizmoMode::Scale => prim.get_scale().ok(),
                };
                self.drag_start_local_rotation = prim
                    .get_local_matrix()
                    .ok()
                    .and_then(Self::quat_from_matrix);
                self.drag_start_world_rotation = prim
                    .get_world_matrix()
                    .ok()
                    .and_then(Self::quat_from_matrix);
                self.drag_start_pointer = response.interact_pointer_pos();
                self.drag_screen_dir = Some(screen_dir);
                self.drag_screen_axis_len = Some(screen_axis_len);
                self.drag_world_axis_len = Some(world_axis_len);
            }
        }

        // During drag
        if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(handle) = self.dragging_handle {
                let snap = response.ctx.input(|i| i.modifiers.shift);
                let pointer_delta = match (self.drag_start_pointer, response.interact_pointer_pos()) {
                    (Some(start), Some(current)) => current - start,
                    _ => response.drag_delta(),
                };
                match handle {
                    GizmoHandle::Axis(axis) => {
                        let axis_dir = axis_dirs[axis];
                        let reference_pos = self.drag_start_pos.unwrap_or(prim_pos);
                        let fallback = self.gizmo_drag_amount(rect, reference_pos, axis_dir);
                        let axis_len = self
                            .drag_world_axis_len
                            .or_else(|| fallback.map(|v| v.0))
                            .unwrap_or(1.0);
                        let screen_axis_len = self
                            .drag_screen_axis_len
                            .or_else(|| fallback.map(|v| v.1))
                            .unwrap_or(1.0);
                        let screen_dir = self
                            .drag_screen_dir
                            .or_else(|| fallback.map(|v| v.2))
                            .unwrap_or(egui::vec2(1.0, 0.0));
                        if screen_axis_len > 0.1 {
                            match self.gizmo_mode {
                                GizmoMode::Select => {}
                                GizmoMode::Translate => {
                                    let mut drag_amount =
                                        (pointer_delta.dot(screen_dir) / screen_axis_len) * axis_len;
                                    if snap {
                                        drag_amount = Self::snap_translate_amount(drag_amount);
                                    }
                                    let start_pos = self.drag_start_pos.unwrap_or(reference_pos);
                                    let new_pos = start_pos + axis_dir * drag_amount;
                                    let _ = prim.set_translate_world(
                                        new_pos.x as f64,
                                        new_pos.y as f64,
                                        new_pos.z as f64,
                                    );
                                }
                                GizmoMode::Rotate => {
                                    if let (Some(start_pointer), Some(current_pointer), Some(center)) = (
                                        self.drag_start_pointer,
                                        response.interact_pointer_pos(),
                                        self.hydra_project(reference_pos, rect),
                                    ) {
                                        let start_vec = start_pointer - center;
                                        let current_vec = current_pointer - center;
                                        let start_len = start_vec.length();
                                        let current_len = current_vec.length();
                                        if start_len > 4.0 && current_len > 4.0 {
                                            let start_dir = start_vec / start_len;
                                            let current_dir = current_vec / current_len;
                                            let cross = start_dir.x * current_dir.y - start_dir.y * current_dir.x;
                                            let dot = start_dir.dot(current_dir).clamp(-1.0, 1.0);
                                            let mut drag_amount_deg =
                                                cross.atan2(dot).to_degrees() as f32;
                                            let camera_forward =
                                                (self.camera.target - self.camera.eye).normalize();
                                            if axis_dir.dot(camera_forward) < 0.0 {
                                                drag_amount_deg = -drag_amount_deg;
                                            }
                                            if snap {
                                                drag_amount_deg =
                                                    Self::snap_rotation_degrees(drag_amount_deg as f64)
                                                        as f32;
                                            }
                                            self.apply_axis_rotation(prim, axis, drag_amount_deg);
                                        }
                                    }
                                }
                                GizmoMode::Scale => {
                                    let drag_amount = pointer_delta.dot(screen_dir) * 0.01;
                                    let start = self.drag_start_values.unwrap_or_else(|| {
                                        prim.get_scale().unwrap_or([1.0, 1.0, 1.0])
                                    });
                                    let mut next = start;
                                    let mut factor = f64::exp(drag_amount as f64);
                                    if snap {
                                        factor = Self::snap_scale_factor(factor);
                                    }
                                    next[axis] = (start[axis] * factor).max(0.001);
                                    let _ = prim.set_scale(next[0], next[1], next[2]);
                                }
                            }
                        }
                    }
                    GizmoHandle::Plane(axis_a, axis_b) => {
                        let reference_pos = self.drag_start_pos.unwrap_or(prim_pos);
                        let axis_len = self
                            .drag_world_axis_len
                            .unwrap_or_else(|| self.gizmo_axis_length(reference_pos));
                        if let Some((screen_a, screen_b)) = self.plane_screen_basis(
                            rect,
                            reference_pos,
                            axis_dirs[axis_a],
                            axis_dirs[axis_b],
                            axis_len,
                        ) {
                            match self.gizmo_mode {
                                GizmoMode::Translate => {
                                    if let Some((mut u, mut v)) =
                                        self.solve_plane_drag(screen_a, screen_b, pointer_delta)
                                    {
                                        u *= axis_len;
                                        v *= axis_len;
                                        if snap {
                                            u = Self::snap_translate_amount(u);
                                            v = Self::snap_translate_amount(v);
                                        }
                                        let start_pos = self.drag_start_pos.unwrap_or(reference_pos);
                                        let new_pos =
                                            start_pos + axis_dirs[axis_a] * u + axis_dirs[axis_b] * v;
                                        let _ = prim.set_translate_world(
                                            new_pos.x as f64,
                                            new_pos.y as f64,
                                            new_pos.z as f64,
                                        );
                                    }
                                }
                                GizmoMode::Scale => {
                                    let diag_dir = Self::normalize_screen_vec(
                                        Self::normalize_screen_vec(screen_a)
                                            + Self::normalize_screen_vec(screen_b),
                                    );
                                    if diag_dir != egui::Vec2::ZERO {
                                        let amount = pointer_delta.dot(diag_dir) * 0.01;
                                        let start = self.drag_start_values.unwrap_or_else(|| {
                                            prim.get_scale().unwrap_or([1.0, 1.0, 1.0])
                                        });
                                        let mut factor = f64::exp(amount as f64).max(0.001);
                                        if snap {
                                            factor = Self::snap_scale_factor(factor);
                                        }
                                        let mut next = start;
                                        next[axis_a] = (start[axis_a] * factor).max(0.001);
                                        next[axis_b] = (start[axis_b] * factor).max(0.001);
                                        let _ = prim.set_scale(next[0], next[1], next[2]);
                                    }
                                }
                                GizmoMode::Select | GizmoMode::Rotate => {}
                            }
                        }
                    }
                    GizmoHandle::Center => match self.gizmo_mode {
                        GizmoMode::Select => {}
                        GizmoMode::Translate => {
                            let start_pos = self.drag_start_pos.unwrap_or(prim_pos);
                            let forward = (self.camera.target - self.camera.eye).normalize();
                            let right = forward.cross(self.camera.up).normalize();
                            let cam_up = right.cross(forward).normalize();
                            let distance = (self.camera.eye - start_pos).length();
                            let world_per_pixel = (2.0
                                * (self.camera.fov * 0.5).tan()
                                * distance)
                                / rect.height().max(1.0);
                            let mut move_world =
                                right * (pointer_delta.x * world_per_pixel)
                                    - cam_up * (pointer_delta.y * world_per_pixel);
                            if snap {
                                move_world.x = Self::snap_translate_amount(move_world.x);
                                move_world.y = Self::snap_translate_amount(move_world.y);
                                move_world.z = Self::snap_translate_amount(move_world.z);
                            }
                            let new_pos = start_pos + move_world;
                            let _ = prim.set_translate_world(
                                new_pos.x as f64,
                                new_pos.y as f64,
                                new_pos.z as f64,
                            );
                        }
                        GizmoMode::Scale => {
                            let start = self.drag_start_values.unwrap_or_else(|| {
                                prim.get_scale().unwrap_or([1.0, 1.0, 1.0])
                            });
                            let amount = (-pointer_delta.y + pointer_delta.x) * 0.01;
                            let mut factor = f64::exp(amount as f64).max(0.001);
                            if snap {
                                factor = Self::snap_scale_factor(factor);
                            }
                            let _ = prim.set_scale(
                                (start[0] * factor).max(0.001),
                                (start[1] * factor).max(0.001),
                                (start[2] * factor).max(0.001),
                            );
                        }
                        GizmoMode::Rotate => {}
                    }
                }
            }
        }

        // End drag
        if response.drag_stopped() {
            if self.dragging_handle.is_some() {
                if let Some(ref stage) = self.stage {
                    let _ = stage.undo_end();
                }
            }
            self.dragging_handle = None;
            self.drag_start_pos = None;
            self.drag_start_values = None;
            self.drag_start_local_rotation = None;
            self.drag_start_world_rotation = None;
            self.drag_start_pointer = None;
            self.drag_screen_dir = None;
            self.drag_screen_axis_len = None;
            self.drag_world_axis_len = None;
        }
    }

}

impl eframe::App for DreamUsdApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_frame_timing();

        // Menu bar
        egui::TopBottomPanel::top("menu_bar")
            .frame(crate::theme::toolbar_frame())
            .show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open        Ctrl+O").clicked() {
                        self.open_file();
                        ui.close_menu();
                    }
                    if ui.button("Save        Ctrl+S").clicked() {
                        self.save_file();
                        ui.close_menu();
                    }
                    if ui.button("Save As     Ctrl+Shift+S").clicked() {
                        self.save_file_as();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo    Ctrl+Z").clicked() {
                        self.undo();
                        ui.close_menu();
                    }
                    if ui.button("Redo    Ctrl+Shift+Z").clicked() {
                        self.redo();
                        ui.close_menu();
                    }
                });
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar")
            .frame(crate::theme::status_bar_frame())
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(crate::theme::subdued(&self.status_message));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.monospace(crate::theme::subdued(&self.fps_label()).small());
                    ui.separator();
                    ui.monospace(crate::theme::subdued(&self.render_stats_label()).small());
                    ui.separator();
                    self.draw_render_delegate_combo(ui, "status_bar_render_delegate");
                    ui.separator();
                    if ui.small_button("Frame").clicked() {
                        self.focus_selected_prim();
                    }
                    ui.separator();
                    for (mode, label) in [
                        (GizmoMode::Scale, "R"),
                        (GizmoMode::Rotate, "E"),
                        (GizmoMode::Translate, "W"),
                        (GizmoMode::Select, "Q"),
                    ] {
                        if ui.selectable_label(self.gizmo_mode == mode, label).clicked() {
                            self.gizmo_mode = mode;
                            self.dragging_handle = None;
                        }
                    }
                    ui.separator();
                    if ui
                        .selectable_label(
                            self.effective_gizmo_space() == GizmoSpace::Local,
                            "Local",
                        )
                        .clicked()
                    {
                        self.set_gizmo_space(GizmoSpace::Local);
                    }
                    ui.add_enabled_ui(self.gizmo_mode != GizmoMode::Scale, |ui| {
                        if ui
                            .selectable_label(
                                self.effective_gizmo_space() == GizmoSpace::World,
                                "World",
                            )
                            .clicked()
                        {
                            self.set_gizmo_space(GizmoSpace::World);
                        }
                    });
                });
            });
        });

        // Scene hierarchy (left)
        egui::SidePanel::left("scene_hierarchy")
            .default_width(220.0)
            .frame(crate::theme::sidebar_frame())
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(crate::theme::panel_title("HIERARCHY"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Focus").clicked() {
                            self.focus_selected_prim();
                        }
                        if ui.small_button("Delete").clicked() {
                            self.delete_selected_prim();
                        }
                    });
                });
                ui.add_space(4.0);
                self.hierarchy.show(ui, self.stage.as_ref());
            });

        // Properties (right)
        let selected_prim: Option<Prim> = (|| {
            let stage = self.stage.as_ref()?;
            let sel_path = self.hierarchy.selected_path.as_deref()?;
            find_prim_recursive(stage, sel_path)
        })();
        let transform_target_prim: Option<Prim> = (|| {
            let stage = self.stage.as_ref()?;
            let sel_path = self.hierarchy.selected_path.as_deref()?;
            let target_path = self.resolve_transform_target_path(sel_path)?;
            find_prim_recursive(stage, &target_path)
        })();

        egui::SidePanel::right("properties")
            .default_width(260.0)
            .frame(crate::theme::sidebar_frame())
            .show(ctx, |ui| {
                ui.label(crate::theme::panel_title("PROPERTIES"));
                ui.add_space(4.0);
                PropertiesPanel::show(
                    ui,
                    self.stage.as_ref(),
                    selected_prim.as_ref(),
                    transform_target_prim.as_ref(),
                    &mut self.hierarchy.selected_path,
                    &mut self.status_message,
                );
            });

        self.show_camera_settings_window(ctx);
        self.show_renderer_settings_window(ctx);

        // 3D Viewport (center)
        egui::CentralPanel::default()
            .frame(Frame::new().fill(crate::theme::app_background()))
            .show(ctx, |ui| {
            // Viewport toolbar
            crate::theme::toolbar_frame().show(ui, |ui: &mut egui::Ui| {
                ui.horizontal(|ui| {
                    let current_label = DISPLAY_MODES[self.current_display_mode].0;
                    egui::ComboBox::from_id_salt("viewport_toolbar_display_mode")
                        .selected_text(current_label)
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for (i, (name, _)) in DISPLAY_MODES.iter().enumerate() {
                                ui.selectable_value(&mut self.current_display_mode, i, *name);
                            }
                        });

                    ui.separator();

                    let current_complexity = VIEWPORT_COMPLEXITIES[self.current_complexity].0;
                    egui::ComboBox::from_id_salt("viewport_toolbar_complexity")
                        .selected_text(current_complexity)
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            for (i, (name, _)) in VIEWPORT_COMPLEXITIES.iter().enumerate() {
                                ui.selectable_value(&mut self.current_complexity, i, *name);
                            }
                        });

                    ui.separator();

                    self.draw_renderer_aov_combo(ui, "viewport_toolbar_aov");

                    ui.separator();

                    ui.menu_button("View", |ui| {
                        ui.checkbox(&mut self.show_grid, "Grid");
                        ui.checkbox(&mut self.show_axis, "Axis");
                        ui.checkbox(&mut self.show_lights, "Lights");
                        ui.checkbox(&mut self.show_shadows, "Shadows");
                        if ui
                            .checkbox(
                                &mut self.auto_compute_clipping_planes,
                                "Auto Clipping",
                            )
                            .changed()
                        {
                            if self.auto_compute_clipping_planes {
                                self.sync_manual_clip_from_camera();
                            }
                            self.invalidate_auto_clip();
                            if !self.auto_compute_clipping_planes {
                                self.restore_manual_clip();
                            }
                            self.viewport_interaction_frames = 2;
                            ui.ctx().request_repaint();
                        }
                        ui.separator();
                        ui.menu_button("Purposes", |ui| {
                            ui.checkbox(&mut self.show_guides, "Guide");
                            ui.checkbox(&mut self.show_proxy, "Proxy");
                            ui.checkbox(&mut self.show_render, "Render");
                        });
                        ui.checkbox(&mut self.enable_scene_materials, "Materials");
                        ui.checkbox(&mut self.cull_backfaces, "Cull Backfaces");
                        ui.checkbox(
                            &mut self.dome_light_textures_visible,
                            "Dome Light Visibility",
                        );
                        ui.separator();
                        ui.menu_button(format!("AA: {}", self.aa_mode.label()), |ui| {
                            for &mode in AntiAliasMode::all() {
                                if ui.selectable_label(self.aa_mode == mode, mode.label()).clicked() {
                                    self.aa_mode = mode;
                                    ui.close_menu();
                                }
                            }
                        });
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Settings").clicked() {
                            self.renderer_settings_open = true;
                        }
                        if ui.small_button("Camera").clicked() {
                            self.camera_settings_open = true;
                        }
                    });
                });
            });

            let rect = ui.available_rect_before_wrap();
            self.viewport_rect = rect;
            self.viewport_pixels_per_point = ctx.pixels_per_point();
            let pointer_delta = ui.input(|i| i.pointer.delta());
            let mut viewport_navigating = false;

            // Handle mouse input for camera
            let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());

            // Detect which gizmo axis is hovered (must happen before drag handling)
            let hovered_handle = if matches!(
                self.gizmo_mode,
                GizmoMode::Translate | GizmoMode::Rotate | GizmoMode::Scale
            ) {
                if let Some(ref prim) = transform_target_prim {
                    if let Some(pos) = self.get_prim_position(prim) {
                        self.detect_hovered_handle(ui, rect, pos, self.gizmo_axes(prim))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            if hovered_handle.is_some() || self.dragging_handle.is_some() {
                ui.output_mut(|o| {
                    o.cursor_icon = egui::CursorIcon::Grab;
                });
            }

            // Only orbit/pan if not dragging gizmo
            if self.dragging_handle.is_none() {
                if response.dragged_by(egui::PointerButton::Secondary) {
                    viewport_navigating = true;
                    if ui.input(|i| i.modifiers.shift) {
                        self.camera
                            .pan_pixels(pointer_delta.x, pointer_delta.y, rect.height());
                    } else {
                        self.camera.orbit(pointer_delta.x, pointer_delta.y);
                    }
                }

                if response.dragged_by(egui::PointerButton::Middle) {
                    viewport_navigating = true;
                    self.camera
                        .pan_pixels(pointer_delta.x, pointer_delta.y, rect.height());
                }
            }

            let pointer_over_viewport = ui.input(|i| {
                i.pointer
                    .hover_pos()
                    .is_some_and(|pos| rect.contains(pos))
            });
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
            if pointer_over_viewport && scroll_delta != 0.0 {
                viewport_navigating = true;
                self.camera.zoom_scroll(scroll_delta);
            }

            if viewport_navigating {
                self.viewport_interaction_frames = 3;
            } else {
                self.viewport_interaction_frames =
                    self.viewport_interaction_frames.saturating_sub(1);
            }

            // Handle gizmo drag interaction
            self.handle_gizmo_drag(&response, &transform_target_prim, rect, hovered_handle);

            if response.clicked_by(egui::PointerButton::Primary)
                && self.dragging_handle.is_none()
                && hovered_handle.is_none()
            {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    self.hierarchy.selected_path = self.pick_prim_in_viewport(rect, pointer_pos);
                }
            }

            // Render via Hydra
            self.render_viewport(ctx, rect);

            // Display the viewport texture or placeholder
            if let Some(ref tex) = self.viewport_texture {
                ui.painter()
                    .rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 30, 30));
                let image_rect = self.viewport_image_rect(rect);
                ui.painter().image(
                    tex.id(),
                    image_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 1.0), egui::pos2(1.0, 0.0)),
                    egui::Color32::WHITE,
                );
            } else {
                ui.painter()
                    .rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 30, 30));

                let text = if self.stage.is_some() {
                    if let Some(ref err) = self.hydra_error {
                        format!("Renderer unavailable: {}", err)
                    } else {
                        "Initializing renderer...".to_string()
                    }
                } else {
                    "No stage loaded — Ctrl+O to open".to_string()
                };

                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::FontId::proportional(16.0),
                    egui::Color32::from_rgb(140, 140, 140),
                );
            }

            // Draw light icons in the viewport
            let sel_path = self.hierarchy.selected_path.as_deref();
            self.draw_light_icons(ui, rect, sel_path);

            if let Some(ref prim) = transform_target_prim {
                if self.gizmo_mode != GizmoMode::Select {
                    self.draw_selection_marker(ui, rect, prim);
                }
                self.draw_drag_overlay(ui, rect, prim);
                if let Some(pos) = self.get_prim_position(prim) {
                    let axis_dirs = self.gizmo_axes(prim);
                    match self.gizmo_mode {
                        GizmoMode::Select => {}
                        GizmoMode::Translate => {
                            self.draw_translate_gizmo(ui, rect, pos, axis_dirs, hovered_handle);
                        }
                        GizmoMode::Rotate => {
                            self.draw_rotate_gizmo(ui, rect, pos, axis_dirs, hovered_handle);
                        }
                        GizmoMode::Scale => {
                            self.draw_scale_gizmo(ui, rect, pos, axis_dirs, hovered_handle);
                        }
                    }
                }
            }

            // Draw XYZ axis gizmo in bottom-left corner
            if self.show_axis {
                self.draw_axis_gizmo(ui, rect);
            }
        });

        self.handle_shortcuts(ctx);

        let async_updates = self
            .hydra
            .as_ref()
            .and_then(|hydra| hydra.poll_async_updates().ok())
            .unwrap_or(false);

        if self.dragging_handle.is_some()
            || self.viewport_interaction_frames > 0
            || async_updates
            || self.auto_clip_is_animating()
        {
            ctx.request_repaint();
        }
    }
}
