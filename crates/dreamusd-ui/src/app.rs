use eframe::egui;

use crate::panels::{HierarchyPanel, PropertiesPanel};
use dreamusd_core::{Prim, Stage};
use dreamusd_render::ViewportCamera;

const DISPLAY_MODES: &[&str] = &[
    "Smooth Shaded",
    "Wireframe",
    "Wireframe on Shaded",
    "Flat Shaded",
    "Points",
    "Textured",
];

/// Gizmo manipulation mode.
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
    hierarchy: HierarchyPanel,
    camera: ViewportCamera,
    current_display_mode: usize,
    show_grid: bool,
    show_axis: bool,
    status_message: String,
    gizmo_mode: GizmoMode,
}

impl Default for DreamUsdApp {
    fn default() -> Self {
        Self {
            stage: None,
            hierarchy: HierarchyPanel::new(),
            camera: ViewportCamera::default(),
            current_display_mode: 0,
            show_grid: true,
            show_axis: true,
            status_message: "Ready".to_string(),
            gizmo_mode: GizmoMode::Translate,
        }
    }
}

impl DreamUsdApp {
    // ── File operations ──────────────────────────────────────────────

    pub fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open USD File")
            .add_filter("USD Files", &["usd", "usda", "usdc", "usdz"])
            .pick_file()
        {
            match Stage::open(&path) {
                Ok(stage) => {
                    self.stage = Some(stage);
                    self.hierarchy = HierarchyPanel::new();
                    self.status_message = format!("Opened: {}", path.display());
                    tracing::info!("Opened file: {}", path.display());
                }
                Err(e) => {
                    self.status_message = format!("Failed to open: {e}");
                    tracing::error!("Failed to open {}: {e}", path.display());
                }
            }
        }
    }

    pub fn save_file(&mut self) {
        if let Some(ref stage) = self.stage {
            match stage.save() {
                Ok(()) => {
                    self.status_message = "Saved".to_string();
                }
                Err(e) => {
                    self.status_message = format!("Save failed: {e}");
                }
            }
        } else {
            self.status_message = "No stage to save".to_string();
        }
    }

    pub fn save_file_as(&mut self) {
        if let Some(ref stage) = self.stage {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Save USD File As")
                .add_filter("USD Files", &["usd", "usda", "usdc", "usdz"])
                .save_file()
            {
                match stage.export(&path) {
                    Ok(()) => {
                        self.status_message = format!("Exported to: {}", path.display());
                    }
                    Err(e) => {
                        self.status_message = format!("Export failed: {e}");
                    }
                }
            }
        } else {
            self.status_message = "No stage to export".to_string();
        }
    }

    // ── Undo / Redo ──────────────────────────────────────────────────

    pub fn undo(&mut self) {
        if let Some(ref stage) = self.stage {
            match stage.undo() {
                Ok(()) => {
                    self.status_message = "Undo".to_string();
                }
                Err(e) => {
                    self.status_message = format!("Undo failed: {e}");
                }
            }
        }
    }

    pub fn redo(&mut self) {
        if let Some(ref stage) = self.stage {
            match stage.redo() {
                Ok(()) => {
                    self.status_message = "Redo".to_string();
                }
                Err(e) => {
                    self.status_message = format!("Redo failed: {e}");
                }
            }
        }
    }

    // ── Keyboard shortcuts ───────────────────────────────────────────

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let modifiers = ctx.input(|i| i.modifiers);

        ctx.input(|i| {
            // Ctrl+O → open
            if i.key_pressed(egui::Key::O) && modifiers.command && !modifiers.shift {
                // Handled after borrow ends
            }
        });
        // We check individually to avoid borrow issues with &mut self
        let ctrl_o = ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.command && !i.modifiers.shift);
        let ctrl_s = ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.command && !i.modifiers.shift);
        let ctrl_shift_s = ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.command && i.modifiers.shift);
        let ctrl_z = ctx.input(|i| i.key_pressed(egui::Key::Z) && i.modifiers.command && !i.modifiers.shift);
        let ctrl_shift_z = ctx.input(|i| i.key_pressed(egui::Key::Z) && i.modifiers.command && i.modifiers.shift);
        let key_w = ctx.input(|i| i.key_pressed(egui::Key::W) && !i.modifiers.command);
        let key_e = ctx.input(|i| i.key_pressed(egui::Key::E) && !i.modifiers.command);
        let key_r = ctx.input(|i| i.key_pressed(egui::Key::R) && !i.modifiers.command);
        let key_delete = ctx.input(|i| i.key_pressed(egui::Key::Delete));
        let key_f = ctx.input(|i| i.key_pressed(egui::Key::F) && !i.modifiers.command);

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

        if key_delete {
            self.status_message = "Delete: not yet implemented".to_string();
        }

        if key_f {
            self.status_message = "Focus: not yet implemented".to_string();
        }
    }

}

impl eframe::App for DreamUsdApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle keyboard shortcuts first
        self.handle_shortcuts(ctx);

        // ── Top panel: Menu bar ──────────────────────────────────────
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

        // ── Bottom panel: Status bar ─────────────────────────────────
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(self.gizmo_mode.label());
                    ui.separator();
                    egui::ComboBox::from_id_salt("display_mode")
                        .selected_text(DISPLAY_MODES[self.current_display_mode])
                        .show_ui(ui, |ui| {
                            for (i, mode) in DISPLAY_MODES.iter().enumerate() {
                                ui.selectable_value(&mut self.current_display_mode, i, *mode);
                            }
                        });
                });
            });
        });

        // ── Left panel: Scene Hierarchy ──────────────────────────────
        egui::SidePanel::left("scene_hierarchy")
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.heading("Scene Hierarchy");
                ui.separator();
                self.hierarchy.show(ui, self.stage.as_ref());
            });

        // ── Right panel: Properties ──────────────────────────────────
        // Look up selected prim if we have a stage and a selection
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

        // ── Central panel: Viewport ──────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();

            // Dark background
            ui.painter().rect_filled(
                rect,
                0.0,
                egui::Color32::from_rgb(30, 30, 30),
            );

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

            // Scroll for zoom
            let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
            if scroll_delta != 0.0 {
                self.camera.zoom(scroll_delta);
            }

            // Placeholder text
            let text = if self.stage.is_some() {
                "Viewport \u{2014} Hydra rendering pending"
            } else {
                "No stage loaded"
            };

            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(18.0),
                egui::Color32::from_rgb(140, 140, 140),
            );
        });
    }
}

/// Recursively find a prim by path starting from the stage root.
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
