use eframe::egui;

use crate::panels::{HierarchyPanel, PropertiesPanel};
use dreamusd_core::{DisplayMode, HydraEngine, Prim, Stage};
use dreamusd_render::glam::Vec3;
use dreamusd_render::ViewportCamera;

const DISPLAY_MODES: &[(&str, DisplayMode)] = &[
    ("Smooth Shaded", DisplayMode::SmoothShaded),
    ("Wireframe", DisplayMode::Wireframe),
    ("Wireframe on Shaded", DisplayMode::WireframeOnShaded),
    ("Flat Shaded", DisplayMode::FlatShaded),
    ("Points", DisplayMode::Points),
    ("Textured", DisplayMode::Textured),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoMode {
    Translate,
    Rotate,
    Scale,
}

impl GizmoMode {
    fn label(self) -> &'static str {
        match self {
            GizmoMode::Translate => "W: Translate",
            GizmoMode::Rotate => "E: Rotate",
            GizmoMode::Scale => "R: Scale",
        }
    }
}

pub struct DreamUsdApp {
    stage: Option<Stage>,
    hydra: Option<HydraEngine>,
    hierarchy: HierarchyPanel,
    camera: ViewportCamera,
    current_display_mode: usize,
    show_grid: bool,
    show_axis: bool,
    show_shadows: bool,
    show_lights: bool,
    status_message: String,
    gizmo_mode: GizmoMode,
    viewport_texture: Option<egui::TextureHandle>,
    hydra_error: Option<String>,
    // Gizmo interaction state
    dragging_axis: Option<usize>, // 0=X, 1=Y, 2=Z
    drag_start_pos: Option<Vec3>,
    viewport_rect: egui::Rect,
}

impl Default for DreamUsdApp {
    fn default() -> Self {
        Self {
            stage: None,
            hydra: None,
            hierarchy: HierarchyPanel::new(),
            camera: ViewportCamera::default(),
            current_display_mode: 0,
            show_grid: true,
            show_axis: true,
            show_shadows: false,
            show_lights: true,
            status_message: "Ready".to_string(),
            gizmo_mode: GizmoMode::Translate,
            viewport_texture: None,
            hydra_error: None,
            dragging_axis: None,
            drag_start_pos: None,
            viewport_rect: egui::Rect::NOTHING,
        }
    }
}

impl DreamUsdApp {
    fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open USD File")
            .add_filter("USD Files", &["usd", "usda", "usdc", "usdz"])
            .pick_file()
        {
            match Stage::open(&path) {
                Ok(stage) => {
                    // Try to create Hydra engine
                    // Configure camera for stage's up axis
                    let up_axis = stage.up_axis();
                    if up_axis == "Z" {
                        self.camera.set_z_up();
                    } else {
                        self.camera.set_y_up();
                    }

                    match HydraEngine::create(&stage) {
                        Ok(engine) => {
                            self.hydra = Some(engine);
                            self.hydra_error = None;
                            self.status_message = format!("Opened: {}", path.display());
                        }
                        Err(e) => {
                            self.hydra = None;
                            self.hydra_error = Some(format!("{}", e));
                            self.status_message =
                                format!("Opened (no renderer): {}", path.display());
                            tracing::warn!("Hydra init failed: {e}");
                        }
                    }
                    self.stage = Some(stage);
                    self.hierarchy = HierarchyPanel::new();
                    self.viewport_texture = None;
                    tracing::info!("Opened file: {}", path.display());
                }
                Err(e) => {
                    self.status_message = format!("Failed to open: {e}");
                    tracing::error!("Failed to open {}: {e}", path.display());
                }
            }
        }
    }

    fn save_file(&mut self) {
        if let Some(ref stage) = self.stage {
            match stage.save() {
                Ok(()) => self.status_message = "Saved".to_string(),
                Err(e) => self.status_message = format!("Save failed: {e}"),
            }
        }
    }

    fn save_file_as(&mut self) {
        if let Some(ref stage) = self.stage {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Save USD File As")
                .add_filter("USD Files", &["usd", "usda", "usdc", "usdz"])
                .save_file()
            {
                match stage.export(&path) {
                    Ok(()) => self.status_message = format!("Exported to: {}", path.display()),
                    Err(e) => self.status_message = format!("Export failed: {e}"),
                }
            }
        }
    }

    fn undo(&mut self) {
        if let Some(ref stage) = self.stage {
            match stage.undo() {
                Ok(()) => self.status_message = "Undo".to_string(),
                Err(e) => self.status_message = format!("Undo failed: {e}"),
            }
        }
    }

    fn redo(&mut self) {
        if let Some(ref stage) = self.stage {
            match stage.redo() {
                Ok(()) => self.status_message = "Redo".to_string(),
                Err(e) => self.status_message = format!("Redo failed: {e}"),
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let ctrl_o =
            ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.command && !i.modifiers.shift);
        let ctrl_s =
            ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.command && !i.modifiers.shift);
        let ctrl_shift_s =
            ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.command && i.modifiers.shift);
        let ctrl_z =
            ctx.input(|i| i.key_pressed(egui::Key::Z) && i.modifiers.command && !i.modifiers.shift);
        let ctrl_shift_z =
            ctx.input(|i| i.key_pressed(egui::Key::Z) && i.modifiers.command && i.modifiers.shift);
        let key_w = ctx.input(|i| i.key_pressed(egui::Key::W) && !i.modifiers.command);
        let key_e = ctx.input(|i| i.key_pressed(egui::Key::E) && !i.modifiers.command);
        let key_r = ctx.input(|i| i.key_pressed(egui::Key::R) && !i.modifiers.command);

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

        if key_w {
            self.gizmo_mode = GizmoMode::Translate;
        } else if key_e {
            self.gizmo_mode = GizmoMode::Rotate;
        } else if key_r {
            self.gizmo_mode = GizmoMode::Scale;
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

    fn is_light_type(type_name: &str) -> bool {
        matches!(
            type_name,
            "DistantLight" | "DomeLight" | "DomeLight_1" | "SphereLight"
                | "DiskLight" | "RectLight" | "CylinderLight" | "PortalLight"
        )
    }

    fn collect_lights(prim: &Prim, out: &mut Vec<(Vec3, String)>) {
        if let Ok(type_name) = prim.type_name() {
            if DreamUsdApp::is_light_type(&type_name) {
                if let Ok(mat) = prim.get_local_matrix() {
                    let pos = Vec3::new(mat[12] as f32, mat[13] as f32, mat[14] as f32);
                    let path = prim.path().unwrap_or_default();
                    out.push((pos, path));
                }
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

        for (world_pos, path) in &lights {
            if let Some(center) = self.hydra_project(*world_pos, rect) {
                let is_selected = selected_path == Some(path.as_str());
                let icon_color = if is_selected {
                    egui::Color32::from_rgb(255, 220, 50)
                } else {
                    egui::Color32::from_rgb(255, 200, 60)
                };
                let radius = if is_selected { 10.0 } else { 7.0 };

                // Draw sun icon: filled circle + rays
                painter.circle_filled(center, radius, icon_color);
                painter.circle_stroke(
                    center,
                    radius,
                    egui::Stroke::new(1.5, egui::Color32::from_rgb(180, 140, 30)),
                );

                // Draw rays
                let ray_len = radius * 0.7;
                let ray_gap = radius + 2.0;
                for angle_idx in 0..8 {
                    let angle = angle_idx as f32 * std::f32::consts::FRAC_PI_4;
                    let dx = angle.cos();
                    let dy = angle.sin();
                    let start = egui::pos2(center.x + dx * ray_gap, center.y + dy * ray_gap);
                    let end = egui::pos2(
                        center.x + dx * (ray_gap + ray_len),
                        center.y + dy * (ray_gap + ray_len),
                    );
                    painter.line_segment(
                        [start, end],
                        egui::Stroke::new(1.5, icon_color),
                    );
                }
            }
        }
    }

    fn get_prim_position(&self, prim: &Prim) -> Option<Vec3> {
        let mat = prim.get_local_matrix().ok()?;
        // USD GfMatrix4d is row-major: translation at [12], [13], [14]
        Some(Vec3::new(mat[12] as f32, mat[13] as f32, mat[14] as f32))
    }

    /// Project a world-space point to screen coordinates using the Hydra engine's
    /// exact view/projection matrices for perfect alignment with the rendered scene.
    fn hydra_project(
        &self,
        world_pos: Vec3,
        rect: egui::Rect,
    ) -> Option<egui::Pos2> {
        let hydra = self.hydra.as_ref()?;
        let w = rect.width().max(1.0) as u32;
        let h = rect.height().max(1.0) as u32;
        let (sx, sy) = hydra.project_point(
            [world_pos.x as f64, world_pos.y as f64, world_pos.z as f64],
            w, h,
        )?;
        Some(egui::pos2(rect.left() + sx as f32, rect.top() + sy as f32))
    }

    /// Detect which gizmo axis the mouse is hovering over (no drawing).
    fn detect_hovered_axis(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        world_pos: Vec3,
    ) -> Option<usize> {
        let cam_dist = (self.camera.eye - world_pos).length();
        let axis_len = cam_dist * 0.15;
        let axes = [Vec3::X, Vec3::Y, Vec3::Z];

        let center_2d = self.hydra_project(world_pos, rect)?;
        let mouse_pos = ui.input(|i| i.pointer.hover_pos()).unwrap_or(egui::Pos2::ZERO);

        for (i, dir) in axes.iter().enumerate() {
            let end_world = world_pos + *dir * axis_len;
            if let Some(end_2d) = self.hydra_project(end_world, rect) {
                let line_vec = end_2d - center_2d;
                let line_len = line_vec.length();
                if line_len > 1.0 {
                    let mouse_vec = mouse_pos - center_2d;
                    let t = mouse_vec.dot(line_vec) / line_vec.dot(line_vec);
                    if t > 0.0 && t < 1.0 {
                        let closest = center_2d + line_vec * t;
                        let dist = (mouse_pos - closest).length();
                        if dist < 8.0 {
                            return Some(i);
                        }
                    }
                }
            }
        }
        None
    }

    fn draw_translate_gizmo(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        world_pos: Vec3,
        hovered_axis: Option<usize>,
    ) {
        let painter = ui.painter();
        let cam_dist = (self.camera.eye - world_pos).length();
        let axis_len = cam_dist * 0.15;

        let axes: [(Vec3, egui::Color32); 3] = [
            (Vec3::X, egui::Color32::from_rgb(230, 60, 60)),
            (Vec3::Y, egui::Color32::from_rgb(60, 200, 60)),
            (Vec3::Z, egui::Color32::from_rgb(60, 100, 230)),
        ];

        let center_2d = match self.hydra_project(world_pos, rect) {
            Some(p) => p,
            None => return,
        };

        for (i, (dir, color)) in axes.iter().enumerate() {
            let end_world = world_pos + *dir * axis_len;
            if let Some(end_2d) = self.hydra_project(end_world, rect) {
                let is_active = self.dragging_axis == Some(i) || hovered_axis == Some(i);
                let stroke_width = if is_active { 4.0 } else { 2.5 };
                let draw_color = if is_active {
                    egui::Color32::YELLOW
                } else {
                    *color
                };

                painter.line_segment([center_2d, end_2d], egui::Stroke::new(stroke_width, draw_color));

                // Arrow head
                let arrow_size = 8.0_f32;
                let dir_2d = (end_2d - center_2d).normalized();
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

    fn handle_gizmo_drag(
        &mut self,
        response: &egui::Response,
        selected_prim: &Option<Prim>,
        rect: egui::Rect,
        hovered_axis: Option<usize>,
    ) {
        if self.gizmo_mode != GizmoMode::Translate {
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

        // Start drag
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(axis) = hovered_axis {
                self.dragging_axis = Some(axis);
                self.drag_start_pos = Some(prim_pos);
            }
        }

        // During drag
        if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(axis) = self.dragging_axis {
                let delta = response.drag_delta();
                let axis_dirs = [Vec3::X, Vec3::Y, Vec3::Z];
                let axis_dir = axis_dirs[axis];

                // Project axis direction to screen space using Hydra projection
                if let (Some(p0), Some(p1)) = (
                    self.hydra_project(prim_pos, rect),
                    self.hydra_project(prim_pos + axis_dir, rect),
                ) {
                    let screen_axis = p1 - p0;
                    let screen_axis_len = screen_axis.length();
                    if screen_axis_len > 0.1 {
                        let screen_dir = screen_axis / screen_axis_len;
                        let drag_amount = delta.dot(screen_dir) / screen_axis_len;

                        let new_pos = prim_pos + axis_dir * drag_amount;
                        let _ = prim.set_translate(
                            new_pos.x as f64,
                            new_pos.y as f64,
                            new_pos.z as f64,
                        );
                    }
                }
            }
        }

        // End drag
        if response.drag_stopped() {
            self.dragging_axis = None;
            self.drag_start_pos = None;
        }
    }

    fn render_viewport(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        let w = rect.width().max(1.0) as u32;
        let h = rect.height().max(1.0) as u32;

        if let Some(ref hydra) = self.hydra {
            // Update camera
            let _ = hydra.set_camera(
                self.camera.eye_as_f64(),
                self.camera.target_as_f64(),
                self.camera.up_as_f64(),
            );

            // Set display mode, lighting, and shadows
            let (_, mode) = DISPLAY_MODES[self.current_display_mode];
            let _ = hydra.set_display_mode(mode);
            let _ = hydra.set_enable_lighting(self.show_lights);
            let _ = hydra.set_enable_shadows(self.show_shadows);

            // Render
            if hydra.render(w, h).is_ok() {
                if let Ok((pixels, fw, fh)) = hydra.get_framebuffer() {
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [fw as usize, fh as usize],
                        pixels,
                    );
                    let texture = ctx.load_texture("viewport", image, egui::TextureOptions::LINEAR);
                    self.viewport_texture = Some(texture);
                }
            }
        }
    }
}

impl eframe::App for DreamUsdApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ctx);

        // Menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
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
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_grid, "Grid");
                    ui.checkbox(&mut self.show_axis, "Axis");
                    ui.checkbox(&mut self.show_lights, "Lights");
                    ui.checkbox(&mut self.show_shadows, "Shadows");
                });
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(self.gizmo_mode.label());
                    ui.separator();
                    let current_label = DISPLAY_MODES[self.current_display_mode].0;
                    egui::ComboBox::from_id_salt("display_mode")
                        .selected_text(current_label)
                        .show_ui(ui, |ui| {
                            for (i, (name, _)) in DISPLAY_MODES.iter().enumerate() {
                                ui.selectable_value(&mut self.current_display_mode, i, *name);
                            }
                        });
                });
            });
        });

        // Scene hierarchy (left)
        egui::SidePanel::left("scene_hierarchy")
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.heading("Scene Hierarchy");
                ui.separator();
                self.hierarchy.show(ui, self.stage.as_ref());
            });

        // Properties (right)
        let selected_prim: Option<Prim> = (|| {
            let stage = self.stage.as_ref()?;
            let sel_path = self.hierarchy.selected_path.as_deref()?;
            find_prim_recursive(stage, sel_path)
        })();

        egui::SidePanel::right("properties")
            .default_width(250.0)
            .show(ctx, |ui| {
                ui.heading("Properties");
                ui.separator();
                PropertiesPanel::show(ui, selected_prim.as_ref());
            });

        // 3D Viewport (center)
        egui::CentralPanel::default().show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            self.viewport_rect = rect;

            // Handle mouse input for camera
            let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());

            // Detect which gizmo axis is hovered (must happen before drag handling)
            let hovered_axis = if self.gizmo_mode == GizmoMode::Translate {
                if let Some(ref prim) = selected_prim {
                    if let Some(pos) = self.get_prim_position(prim) {
                        self.detect_hovered_axis(ui, rect, pos)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            // Only orbit/pan if not dragging gizmo
            if self.dragging_axis.is_none() {
                if response.dragged_by(egui::PointerButton::Secondary) {
                    let delta = response.drag_delta();
                    if ui.input(|i| i.modifiers.shift) {
                        self.camera.pan(delta.x, delta.y);
                    } else {
                        self.camera.orbit(delta.x, delta.y);
                    }
                }

                if response.dragged_by(egui::PointerButton::Middle) {
                    let delta = response.drag_delta();
                    self.camera.pan(delta.x, delta.y);
                }
            }

            let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
            if scroll_delta != 0.0 {
                self.camera.zoom(scroll_delta);
            }

            // Handle gizmo drag interaction
            self.handle_gizmo_drag(&response, &selected_prim, rect, hovered_axis);

            // Render via Hydra
            self.render_viewport(ctx, rect);

            // Display the viewport texture or placeholder
            if let Some(ref tex) = self.viewport_texture {
                ui.painter().image(
                    tex.id(),
                    rect,
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

            // Draw translate gizmo on top of rendered image
            if self.gizmo_mode == GizmoMode::Translate {
                if let Some(ref prim) = selected_prim {
                    if let Some(pos) = self.get_prim_position(prim) {
                        self.draw_translate_gizmo(ui, rect, pos, hovered_axis);
                    }
                }
            }

            // Draw XYZ axis gizmo in bottom-left corner
            if self.show_axis {
                self.draw_axis_gizmo(ui, rect);
            }
        });

        // Request continuous repaint when rendering
        if self.hydra.is_some() {
            ctx.request_repaint();
        }
    }
}

fn find_prim_recursive(stage: &Stage, target_path: &str) -> Option<Prim> {
    let root = stage.root_prim().ok()?;
    find_in_subtree(root, target_path)
}

fn find_in_subtree(prim: Prim, target_path: &str) -> Option<Prim> {
    let path = prim.path().ok()?;
    if path == target_path {
        return Some(prim);
    }
    let children = prim.children().ok()?;
    for child in children {
        if let Some(found) = find_in_subtree(child, target_path) {
            return Some(found);
        }
    }
    None
}
