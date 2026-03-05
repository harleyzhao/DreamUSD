use eframe::egui;

use crate::panels::{HierarchyPanel, PropertiesPanel};
use dreamusd_core::{DisplayMode, HydraEngine, Prim, Stage};
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
    status_message: String,
    gizmo_mode: GizmoMode,
    viewport_texture: Option<egui::TextureHandle>,
    hydra_error: Option<String>,
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
            status_message: "Ready".to_string(),
            gizmo_mode: GizmoMode::Translate,
            viewport_texture: None,
            hydra_error: None,
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

            // Set display mode
            let (_, mode) = DISPLAY_MODES[self.current_display_mode];
            let _ = hydra.set_display_mode(mode);

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

            // Handle mouse input for camera
            let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());

            if response.dragged_by(egui::PointerButton::Middle) {
                let delta = response.drag_delta();
                if ui.input(|i| i.modifiers.shift) {
                    self.camera.pan(delta.x, delta.y);
                } else {
                    self.camera.orbit(delta.x, delta.y);
                }
            }

            let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
            if scroll_delta != 0.0 {
                self.camera.zoom(scroll_delta);
            }

            // Render via Hydra
            self.render_viewport(ctx, rect);

            // Display the viewport texture or placeholder
            if let Some(ref tex) = self.viewport_texture {
                ui.painter().image(
                    tex.id(),
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
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
