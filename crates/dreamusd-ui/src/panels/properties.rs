use dreamusd_core::Prim;

/// Panel that displays the properties of a selected prim.
pub struct PropertiesPanel;

impl PropertiesPanel {
    /// Show the properties for the given prim (or a placeholder if none).
    pub fn show(ui: &mut egui::Ui, prim: Option<&Prim>) {
        let prim = match prim {
            Some(p) => p,
            None => {
                ui.label("No prim selected");
                return;
            }
        };

        // Header: path and type
        let path = prim.path().unwrap_or_else(|_| "???".to_string());
        let type_name = prim.type_name().unwrap_or_else(|_| String::new());

        ui.heading(&path);
        if !type_name.is_empty() {
            ui.label(format!("Type: {type_name}"));
        }
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Transform section
            egui::CollapsingHeader::new("Transform")
                .default_open(true)
                .show(ui, |ui| {
                    match prim.get_local_matrix() {
                        Ok(m) => {
                            // Column-major 4x4: translation is in elements [12], [13], [14]
                            ui.label(format!("Translate X: {:.4}", m[12]));
                            ui.label(format!("Translate Y: {:.4}", m[13]));
                            ui.label(format!("Translate Z: {:.4}", m[14]));

                            ui.collapsing("Full Matrix", |ui| {
                                for row in 0..4 {
                                    ui.label(format!(
                                        "[{:.4}, {:.4}, {:.4}, {:.4}]",
                                        m[row],
                                        m[row + 4],
                                        m[row + 8],
                                        m[row + 12],
                                    ));
                                }
                            });
                        }
                        Err(e) => {
                            ui.label(format!("(no transform: {e})"));
                        }
                    }
                });

            // Attributes section
            egui::CollapsingHeader::new("Attributes")
                .default_open(false)
                .show(ui, |ui| match prim.attribute_names() {
                    Ok(names) => {
                        if names.is_empty() {
                            ui.label("(none)");
                        } else {
                            egui::Grid::new("attr_grid")
                                .striped(true)
                                .num_columns(2)
                                .show(ui, |ui| {
                                    for name in &names {
                                        ui.label(name);
                                        let val = prim
                                            .get_attribute(name)
                                            .unwrap_or_else(|_| "(error)".to_string());
                                        let id = ui.make_persistent_id(format!("attr_{name}"));
                                        let mut edit_val = ui.data(|d| d.get_temp::<String>(id).unwrap_or(val.clone()));
                                        let response = ui.text_edit_singleline(&mut edit_val);
                                        if response.changed() {
                                            ui.data_mut(|d| d.insert_temp(id, edit_val.clone()));
                                        }
                                        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                            if edit_val != val {
                                                let _ = prim.set_attribute(name, &edit_val);
                                            }
                                            ui.data_mut(|d| d.remove_temp::<String>(id));
                                        }
                                        ui.end_row();
                                    }
                                });
                        }
                    }
                    Err(e) => {
                        ui.label(format!("Error: {e}"));
                    }
                });

            // Variants section
            egui::CollapsingHeader::new("Variants")
                .default_open(false)
                .show(ui, |ui| match prim.variant_sets() {
                    Ok(sets) => {
                        if sets.is_empty() {
                            ui.label("(none)");
                        } else {
                            egui::Grid::new("variant_grid")
                                .striped(true)
                                .show(ui, |ui| {
                                    for set_name in &sets {
                                        ui.label(set_name);
                                        let sel = prim
                                            .get_variant_selection(set_name)
                                            .unwrap_or_else(|_| "(none)".to_string());
                                        ui.label(&sel);
                                        ui.end_row();
                                    }
                                });
                        }
                    }
                    Err(e) => {
                        ui.label(format!("Error: {e}"));
                    }
                });

            // Material section
            egui::CollapsingHeader::new("Material")
                .default_open(false)
                .show(ui, |ui| match prim.material_binding() {
                    Ok(binding) => {
                        if binding.is_empty() {
                            ui.label("(none)");
                        } else {
                            ui.label(format!("Bound: {binding}"));
                        }
                    }
                    Err(_) => {
                        ui.label("(no material binding)");
                    }
                });
        });
    }
}
