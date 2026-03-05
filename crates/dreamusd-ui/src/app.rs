use eframe::egui;

const DISPLAY_MODES: &[&str] = &[
    "Smooth Shaded",
    "Wireframe",
    "Wireframe on Shaded",
    "Flat Shaded",
    "Points",
    "Textured",
];

pub struct DreamUsdApp {
    selected_prim_path: Option<String>,
    show_grid: bool,
    show_axis: bool,
    current_display_mode: usize,
    status_message: String,
}

impl Default for DreamUsdApp {
    fn default() -> Self {
        Self {
            selected_prim_path: None,
            show_grid: true,
            show_axis: true,
            current_display_mode: 0,
            status_message: "Ready".to_string(),
        }
    }
}

impl DreamUsdApp {
    pub fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open USD File")
            .add_filter("USD Files", &["usd", "usda", "usdc", "usdz"])
            .pick_file()
        {
            self.status_message = format!("Opened: {}", path.display());
            tracing::info!("Opened file: {}", path.display());
        }
    }
}

impl eframe::App for DreamUsdApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top panel: Menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open").clicked() {
                        self.open_file();
                        ui.close_menu();
                    }
                    if ui.button("Save").clicked() {
                        self.status_message = "Save not yet implemented".to_string();
                        ui.close_menu();
                    }
                    if ui.button("Save As").clicked() {
                        self.status_message = "Save As not yet implemented".to_string();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo").clicked() {
                        self.status_message = "Undo not yet implemented".to_string();
                        ui.close_menu();
                    }
                    if ui.button("Redo").clicked() {
                        self.status_message = "Redo not yet implemented".to_string();
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_grid, "Grid");
                    ui.checkbox(&mut self.show_axis, "Axis");
                });
            });
        });

        // Bottom panel: Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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

        // Left panel: Scene Hierarchy
        egui::SidePanel::left("scene_hierarchy")
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.heading("Scene Hierarchy");
                ui.separator();
                ui.label("No stage loaded");
            });

        // Right panel: Properties
        egui::SidePanel::right("properties")
            .default_width(250.0)
            .show(ctx, |ui| {
                ui.heading("Properties");
                ui.separator();
                if let Some(ref path) = self.selected_prim_path {
                    ui.label(format!("Selected: {}", path));
                } else {
                    ui.label("No prim selected");
                }
            });

        // Central panel: 3D Viewport
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                ui.label("3D Viewport");
            });
        });
    }
}
