use crate::theme;
use dreamusd_core::{MaterialParam, Prim, Stage};

/// Panel that displays the properties of a selected prim.
pub struct PropertiesPanel;

impl PropertiesPanel {
    /// Show the properties for the given prim (or a placeholder if none).
    pub fn show(
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        prim: Option<&Prim>,
        transform_prim: Option<&Prim>,
        selected_path: &mut Option<String>,
        status_message: &mut String,
    ) {
        let prim = match prim {
            Some(p) => p,
            None => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(theme::subdued("No prim selected"));
                });
                return;
            }
        };

        // Header: path and type
        let path = prim.path().unwrap_or_else(|_| "???".to_string());
        let type_name = prim.type_name().unwrap_or_else(|_| String::new());

        theme::panel_card_frame().show(ui, |ui: &mut egui::Ui| {
            ui.label(egui::RichText::new(&path).strong().small());
            if !type_name.is_empty() {
                ui.label(theme::subdued(&format!("Type: {type_name}")));
            }
        });
        ui.add_space(4.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            if path.starts_with("/_DefaultLights/") {
                egui::CollapsingHeader::new("Default Light")
                    .default_open(true)
                    .show(ui, |ui| {
                    Self::show_default_light_editor(ui, stage, prim);
                });
            }

            // Transform section
            egui::CollapsingHeader::new("Transform")
                .default_open(true)
                .show(ui, |ui| {
                let transform_prim = transform_prim.unwrap_or(prim);
                match (
                    transform_prim.get_local_matrix(),
                    transform_prim.get_translate(),
                    transform_prim.get_rotate(),
                    transform_prim.get_scale(),
                ) {
                    (Ok(matrix), Ok(translate), Ok(rotate), Ok(scale)) => {
                        if !std::ptr::eq(transform_prim, prim) {
                            let transform_path = transform_prim
                                .path()
                                .unwrap_or_else(|_| "???".to_string());
                            ui.label(theme::subdued(&format!("Editing target: {transform_path}")));
                            ui.add_space(2.0);
                        }
                        let mut row_counter: usize = 0;
                        Self::show_vector_editor(
                            ui,
                            stage,
                            transform_prim,
                            "Translate",
                            translate,
                            Prim::set_translate,
                            &mut row_counter,
                        );
                        Self::show_vector_editor(
                            ui,
                            stage,
                            transform_prim,
                            "Rotate",
                            rotate,
                            Prim::set_rotate,
                            &mut row_counter,
                        );
                        Self::show_vector_editor(
                            ui,
                            stage,
                            transform_prim,
                            "Scale",
                            scale,
                            Prim::set_scale,
                            &mut row_counter,
                        );

                        ui.collapsing("Full Matrix", |ui| {
                            for row in 0..4 {
                                ui.label(format!(
                                    "[{:.4}, {:.4}, {:.4}, {:.4}]",
                                    matrix[row],
                                    matrix[row + 4],
                                    matrix[row + 8],
                                    matrix[row + 12],
                                ));
                            }
                        });
                    }
                    (Err(e), _, _, _) => {
                        ui.label(format!("(no transform: {e})"));
                    }
                    (_, Err(e), _, _) => {
                        ui.label(format!("(translate unavailable: {e})"));
                    }
                    (_, _, Err(e), _) => {
                        ui.label(format!("(rotate unavailable: {e})"));
                    }
                    (_, _, _, Err(e)) => {
                        ui.label(format!("(scale unavailable: {e})"));
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
                        for (row_idx, name) in names.iter().enumerate() {
                            Self::paint_row_bg(ui, row_idx);
                            ui.horizontal(|ui| {
                                ui.label(theme::subdued(name));
                                let val = prim
                                    .get_attribute(name)
                                    .unwrap_or_else(|_| "(error)".to_string());
                                let id = ui.make_persistent_id(format!("attr_{name}"));
                                let mut edit_val =
                                    ui.data(|d| d.get_temp::<String>(id).unwrap_or(val.clone()));
                                let response = ui.text_edit_singleline(&mut edit_val);
                                if response.changed() {
                                    ui.data_mut(|d| d.insert_temp(id, edit_val.clone()));
                                }
                                if response.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    if edit_val != val {
                                        if let Some(stage) = stage {
                                            let _ = stage.undo_begin();
                                            let _ = prim.set_attribute(name, &edit_val);
                                            let _ = stage.undo_end();
                                        } else {
                                            let _ = prim.set_attribute(name, &edit_val);
                                        }
                                    }
                                    ui.data_mut(|d| d.remove_temp::<String>(id));
                                }
                            });
                        }
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
                        for (row_idx, set_name) in sets.iter().enumerate() {
                            Self::paint_row_bg(ui, row_idx);
                            ui.horizontal(|ui| {
                                ui.label(theme::subdued(set_name));
                                Self::show_variant_editor(ui, stage, prim, set_name);
                            });
                        }
                    }
                }
                Err(e) => {
                    ui.label(format!("Error: {e}"));
                }
            });

            // Hierarchy section
            egui::CollapsingHeader::new("Hierarchy")
                .default_open(false)
                .show(ui, |ui| {
                Self::show_reparent_editor(ui, stage, prim, &path, selected_path, status_message);
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
                        if let Some(stage) = stage {
                            Self::show_material_params(
                                ui,
                                stage,
                                &binding,
                                selected_path,
                                status_message,
                            );
                        }
                    }
                }
                Err(e) => {
                    ui.label(format!("(no material binding: {e})"));
                }
            });
        });
    }

    /// Paint a full-width alternating row background.
    fn paint_row_bg(ui: &mut egui::Ui, row_index: usize) {
        let row_bg = if row_index % 2 == 0 {
            egui::Color32::from_rgb(24, 24, 30)
        } else {
            egui::Color32::from_rgb(36, 36, 44)
        };
        let row_height = ui.spacing().interact_size.y;
        let full_rect = egui::Rect::from_min_size(
            egui::pos2(ui.clip_rect().left(), ui.cursor().top()),
            egui::vec2(ui.clip_rect().width(), row_height),
        );
        ui.painter().rect_filled(full_rect, 0.0, row_bg);
    }

    fn show_vector_editor(
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        prim: &Prim,
        prefix: &str,
        values: [f64; 3],
        setter: fn(&Prim, f64, f64, f64) -> Result<(), dreamusd_core::DuError>,
        row_counter: &mut usize,
    ) {
        let labels = ["X", "Y", "Z"];
        let mut edited_values = values;
        let mut changed = false;

        for (index, axis) in labels.iter().enumerate() {
            Self::paint_row_bg(ui, *row_counter);
            *row_counter += 1;
            ui.horizontal(|ui| {
                ui.label(format!("{prefix} {axis}"));
                let id = ui.make_persistent_id(format!("{prefix}_{index}"));
                let mut text = ui
                    .data(|d| d.get_temp::<String>(id))
                    .unwrap_or_else(|| format!("{:.4}", values[index]));
                let response = ui.text_edit_singleline(&mut text);
                if response.changed() {
                    ui.data_mut(|d| d.insert_temp(id, text.clone()));
                }
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Ok(parsed) = text.parse::<f64>() {
                        edited_values[index] = parsed;
                        changed = true;
                    }
                    ui.data_mut(|d| d.remove_temp::<String>(id));
                }
            });
        }

        if changed {
            if let Some(stage) = stage {
                let _ = stage.undo_begin();
            }
            let _ = setter(prim, edited_values[0], edited_values[1], edited_values[2]);
            if let Some(stage) = stage {
                let _ = stage.undo_end();
            }
        }
    }

    fn show_variant_editor(
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        prim: &Prim,
        set_name: &str,
    ) {
        let current = prim
            .get_variant_selection(set_name)
            .unwrap_or_else(|_| "(none)".to_string());
        match prim.variant_names(set_name) {
            Ok(variants) if !variants.is_empty() => {
                egui::ComboBox::from_id_salt(("variant_combo", set_name))
                    .selected_text(&current)
                    .show_ui(ui, |ui| {
                        for variant in variants {
                            let selected = current == variant;
                            if ui.selectable_label(selected, &variant).clicked() && !selected {
                                if let Some(stage) = stage {
                                    let _ = stage.undo_begin();
                                }
                                let _ = prim.set_variant_selection(set_name, &variant);
                                if let Some(stage) = stage {
                                    let _ = stage.undo_end();
                                }
                                ui.close_menu();
                            }
                        }
                    });
            }
            Ok(_) => {
                ui.label(&current);
            }
            Err(e) => {
                ui.label(format!("Error: {e}"));
            }
        }
    }

    fn show_reparent_editor(
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        prim: &Prim,
        current_path: &str,
        selected_path: &mut Option<String>,
        status_message: &mut String,
    ) {
        let parent_path = current_path
            .rsplit_once('/')
            .map(|(parent, _)| if parent.is_empty() { "/" } else { parent })
            .unwrap_or("/");
        let id = ui.make_persistent_id("reparent_target");
        let mut target_parent = ui
            .data(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| parent_path.to_string());

        ui.label("New Parent Path");
        let response = ui.text_edit_singleline(&mut target_parent);
        if response.changed() {
            ui.data_mut(|d| d.insert_temp(id, target_parent.clone()));
        }

        if ui.button("Move Prim").clicked() {
            let prim_name = current_path
                .rsplit('/')
                .next()
                .filter(|name| !name.is_empty())
                .unwrap_or("Prim");
            let new_path = if target_parent == "/" {
                format!("/{prim_name}")
            } else {
                format!("{target_parent}/{prim_name}")
            };
            if let Some(stage) = stage {
                let _ = stage.undo_begin();
            }
            match prim.reparent(&target_parent) {
                Ok(()) => {
                    *selected_path = Some(new_path.clone());
                    *status_message = format!("Moved: {new_path}");
                    if let Some(stage) = stage {
                        let _ = stage.undo_end();
                    }
                }
                Err(e) => {
                    *status_message = format!("Move failed: {e}");
                    if let Some(stage) = stage {
                        let _ = stage.undo_end();
                    }
                }
            }
        }
    }

    fn show_material_params(
        ui: &mut egui::Ui,
        stage: &Stage,
        binding_path: &str,
        selected_path: &mut Option<String>,
        status_message: &mut String,
    ) {
        let Some(material_prim) = find_prim_recursive(stage, binding_path) else {
            ui.label("Bound material prim not found in stage");
            return;
        };

        if ui.button("Select Material").clicked() {
            *selected_path = Some(binding_path.to_string());
            *status_message = format!("Selected material: {binding_path}");
        }

        match material_prim.material_params() {
            Ok(params) if params.is_empty() => {
                ui.label("(no editable shader inputs)");
            }
            Ok(params) => {
                for (row_idx, param) in params.iter().enumerate() {
                    Self::paint_row_bg(ui, row_idx);
                    Self::show_material_param_row(
                        ui,
                        stage,
                        &material_prim,
                        param,
                        status_message,
                    );
                }
            }
            Err(e) => {
                ui.label(format!("Material params unavailable: {e}"));
            }
        }
    }

    fn show_default_light_editor(ui: &mut egui::Ui, stage: Option<&Stage>, prim: &Prim) {
        let Ok(raw_intensity) = prim.get_attribute("intensity") else {
            ui.label("(default light settings unavailable)");
            return;
        };
        let intensity = raw_intensity.parse::<f64>().unwrap_or(1.0);
        let mut edited = intensity;

        ui.horizontal(|ui| {
            ui.label("Intensity");
            let response = ui.add(
                egui::DragValue::new(&mut edited)
                    .speed(0.05)
                    .range(0.0..=100.0),
            );
            if response.changed() {
                if let Some(stage) = stage {
                    let _ = stage.undo_begin();
                }
                let _ = prim.set_attribute("intensity", &edited.to_string());
                if let Some(stage) = stage {
                    let _ = stage.undo_end();
                }
            }
        });
    }

    fn show_material_param_row(
        ui: &mut egui::Ui,
        stage: &Stage,
        material_prim: &Prim,
        param: &MaterialParam,
        status_message: &mut String,
    ) {
        let label = if param.is_texture {
            format!("{} ({}, texture)", param.name, param.type_name)
        } else {
            format!("{} ({})", param.name, param.type_name)
        };

        ui.horizontal(|ui| {
            ui.label(theme::subdued(&label));

            let id = ui.make_persistent_id(format!("material_param_{}", param.name));
            let mut edit_value = ui
                .data(|d| d.get_temp::<String>(id))
                .unwrap_or_else(|| param.value.clone());
            let response = ui.text_edit_singleline(&mut edit_value);
            if response.changed() {
                ui.data_mut(|d| d.insert_temp(id, edit_value.clone()));
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if edit_value != param.value {
                    let _ = stage.undo_begin();
                    match material_prim.set_material_param(&param.name, &edit_value) {
                        Ok(()) => {
                            *status_message = format!("Updated material param: {}", param.name);
                        }
                        Err(e) => {
                            *status_message = format!("Material update failed: {e}");
                        }
                    }
                    let _ = stage.undo_end();
                }
                ui.data_mut(|d| d.remove_temp::<String>(id));
            }
        });
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
