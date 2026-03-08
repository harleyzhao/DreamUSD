use crate::theme;
use dreamusd_core::{MaterialParam, Prim, Stage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PropertiesTab {
    Quick,
    Inspector,
}

impl PropertiesTab {
    fn label(self) -> &'static str {
        match self {
            Self::Quick => "Quick",
            Self::Inspector => "Inspector",
        }
    }
}

/// Panel that displays the properties of an inspected prim.
pub struct PropertiesPanel {
    active_tab: PropertiesTab,
    search_text: String,
    pinned_path: Option<String>,
}

impl Default for PropertiesPanel {
    fn default() -> Self {
        Self {
            active_tab: PropertiesTab::Quick,
            search_text: String::new(),
            pinned_path: None,
        }
    }
}

impl PropertiesPanel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_frame(
        &mut self,
        stage: Option<&Stage>,
        current_selection: Option<&str>,
    ) -> Option<String> {
        if self
            .pinned_path
            .as_deref()
            .is_some_and(|path| stage.and_then(|stage| find_prim_recursive(stage, path)).is_none())
        {
            self.pinned_path = None;
        }

        self.pinned_path
            .clone()
            .or_else(|| current_selection.map(ToOwned::to_owned))
    }

    /// Show the properties for the inspected prim.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        current_selection: Option<&str>,
        selected_paths: &[String],
        selected_transform_paths: &[String],
        inspected_path: Option<&str>,
        prim: Option<&Prim>,
        transform_prim: Option<&Prim>,
        selected_path: &mut Option<String>,
        status_message: &mut String,
    ) {
        self.show_toolbar(ui, current_selection, inspected_path);
        ui.add_space(4.0);

        if self.pinned_path.is_none() && selected_paths.len() > 1 {
            self.show_multi_edit(
                ui,
                stage,
                current_selection,
                selected_paths,
                selected_transform_paths,
                status_message,
            );
            return;
        }

        let prim = match prim {
            Some(prim) => prim,
            None => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    if self.pinned_path.is_some() {
                        ui.label(theme::subdued("Pinned prim no longer exists"));
                    } else {
                        ui.label(theme::subdued("No prim selected"));
                    }
                });
                return;
            }
        };

        let path = prim.path().unwrap_or_else(|_| "???".to_string());
        let type_name = prim.type_name().unwrap_or_default();

        theme::panel_card_frame().show(ui, |ui| {
            ui.label(
                egui::RichText::new(&path)
                    .strong()
                    .small()
                    .color(theme::text_color()),
            );
            if !type_name.is_empty() {
                ui.label(theme::subdued(&format!("Type: {type_name}")));
            }
            if let Some(current_selection) = current_selection.filter(|selected| *selected != path) {
                ui.label(theme::subdued(&format!("Selection: {current_selection}")));
            }
            if self.pinned_path.as_deref() == Some(path.as_str()) {
                ui.label(theme::subdued("Pinned inspector target"));
            }
        });
        ui.add_space(4.0);

        egui::ScrollArea::vertical().show(ui, |ui| match self.active_tab {
            PropertiesTab::Quick => {
                self.show_quick_tab(
                    ui,
                    stage,
                    prim,
                    transform_prim.unwrap_or(prim),
                    &type_name,
                    selected_path,
                    status_message,
                );
            }
            PropertiesTab::Inspector => {
                self.show_inspector_tab(
                    ui,
                    stage,
                    prim,
                    transform_prim.unwrap_or(prim),
                    &path,
                    &type_name,
                    selected_path,
                    status_message,
                );
            }
        });
    }

    fn show_multi_edit(
        &mut self,
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        current_selection: Option<&str>,
        selected_paths: &[String],
        selected_transform_paths: &[String],
        status_message: &mut String,
    ) {
        let Some(stage) = stage else {
            ui.label(theme::subdued("No stage loaded"));
            return;
        };

        let selected_prims = selected_paths
            .iter()
            .filter_map(|path| find_prim_recursive(stage, path))
            .collect::<Vec<_>>();
        let transform_prims = selected_transform_paths
            .iter()
            .filter_map(|path| find_prim_recursive(stage, path))
            .collect::<Vec<_>>();

        theme::panel_card_frame().show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!("{} prims selected", selected_prims.len()))
                    .strong()
                    .small()
                    .color(theme::text_color()),
            );
            if let Some(primary) = current_selection {
                ui.label(theme::subdued(&format!("Primary: {primary}")));
            }
            ui.label(theme::subdued("Changes apply to the full selection"));
        });
        ui.add_space(4.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            theme::collapsing_section(ui, "Transform", true, |ui| {
                self.show_multi_vec3_editor(
                    ui,
                    Some(stage),
                    &transform_prims,
                    "Translate",
                    Prim::get_translate,
                    Prim::set_translate,
                    0,
                );
                self.show_multi_vec3_editor(
                    ui,
                    Some(stage),
                    &transform_prims,
                    "Rotate",
                    Prim::get_rotate,
                    Prim::set_rotate,
                    1,
                );
                self.show_multi_vec3_editor(
                    ui,
                    Some(stage),
                    &transform_prims,
                    "Scale",
                    Prim::get_scale,
                    Prim::set_scale,
                    2,
                );
            });

            theme::collapsing_section(ui, "Common", true, |ui| {
                self.show_multi_common_attributes(ui, stage, &selected_prims, status_message);
            });

            if self.active_tab == PropertiesTab::Inspector {
                ui.add_space(6.0);
                ui.label(theme::subdued(
                    "Full per-prim inspector is not available in multi-edit yet.",
                ));
            }
        });
    }

    fn show_toolbar(
        &mut self,
        ui: &mut egui::Ui,
        current_selection: Option<&str>,
        inspected_path: Option<&str>,
    ) {
        theme::panel_card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                for tab in [PropertiesTab::Quick, PropertiesTab::Inspector] {
                    ui.selectable_value(&mut self.active_tab, tab, tab.label());
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_pinned = inspected_path
                        .zip(self.pinned_path.as_deref())
                        .is_some_and(|(inspected, pinned)| inspected == pinned);
                    if is_pinned {
                        if ui.small_button("Unpin").clicked() {
                            self.pinned_path = None;
                        }
                    } else if let Some(selected) = current_selection {
                        if ui.small_button("Pin").clicked() {
                            self.pinned_path = Some(selected.to_string());
                        }
                    }
                });
            });

            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.search_text)
                    .hint_text("Search attributes, variants, materials...")
                    .desired_width(ui.available_width()),
            );
        });
    }

    fn show_quick_tab(
        &mut self,
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        prim: &Prim,
        transform_prim: &Prim,
        type_name: &str,
        selected_path: &mut Option<String>,
        status_message: &mut String,
    ) {
        if type_name.starts_with("DomeLight")
            || type_name.starts_with("DistantLight")
            || type_name.ends_with("Light")
        {
            theme::collapsing_section(ui, "Light", true, |ui| {
                Self::show_light_editor(ui, stage, prim);
            });
        }

        theme::collapsing_section(ui, "Transform", true, |ui| {
            Self::show_transform_editor(ui, stage, transform_prim, false);
        });

        theme::collapsing_section(ui, "Common", true, |ui| {
            self.show_common_attributes(ui, stage, prim);
        });

        theme::collapsing_section(ui, "Variants", true, |ui| {
            self.show_variants(ui, stage, prim);
        });

        theme::collapsing_section(ui, "Material", true, |ui| {
            self.show_material_summary(ui, stage, prim, selected_path, status_message);
        });
    }

    fn show_inspector_tab(
        &mut self,
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        prim: &Prim,
        transform_prim: &Prim,
        path: &str,
        type_name: &str,
        selected_path: &mut Option<String>,
        status_message: &mut String,
    ) {
        if path.starts_with("/_DefaultLights/") {
            theme::collapsing_section(ui, "Default Light", true, |ui| {
                Self::show_default_light_editor(ui, stage, prim);
            });
        } else if Self::is_light_type(type_name) {
            theme::collapsing_section(ui, "Light", true, |ui| {
                Self::show_light_editor(ui, stage, prim);
            });
        }

        theme::collapsing_section(ui, "Transform", true, |ui| {
            Self::show_transform_editor(ui, stage, transform_prim, true);
        });

        theme::collapsing_section(ui, "Attributes", true, |ui| {
            self.show_all_attributes(ui, stage, prim);
        });

        theme::collapsing_section(ui, "Variants", false, |ui| {
            self.show_variants(ui, stage, prim);
        });

        theme::collapsing_section(ui, "Hierarchy", false, |ui| {
            Self::show_reparent_editor(ui, stage, prim, path, selected_path, status_message);
        });

        theme::collapsing_section(ui, "Material", false, |ui| {
            self.show_material_summary(ui, stage, prim, selected_path, status_message);
        });
    }

    fn show_transform_editor(
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        prim: &Prim,
        show_matrix: bool,
    ) {
        match (
            prim.get_local_matrix(),
            prim.get_translate(),
            prim.get_rotate(),
            prim.get_scale(),
        ) {
            (Ok(matrix), Ok(translate), Ok(rotate), Ok(scale)) => {
                Self::show_vec3_editor(ui, stage, prim, "Translate", translate, Prim::set_translate, 0);
                Self::show_vec3_editor(ui, stage, prim, "Rotate", rotate, Prim::set_rotate, 1);
                Self::show_vec3_editor(ui, stage, prim, "Scale", scale, Prim::set_scale, 2);
                if show_matrix {
                    theme::collapsing_section(ui, "Full Matrix", false, |ui| {
                        for row in 0..4 {
                            ui.label(
                                egui::RichText::new(format!(
                                    "[{:.4}, {:.4}, {:.4}, {:.4}]",
                                    matrix[row],
                                    matrix[row + 4],
                                    matrix[row + 8],
                                    matrix[row + 12],
                                ))
                                .color(theme::text_color()),
                            );
                        }
                    });
                }
            }
            (Err(err), _, _, _) => {
                ui.label(
                    egui::RichText::new(format!("(no transform: {err})"))
                        .color(theme::text_color()),
                );
            }
            (_, Err(err), _, _) => {
                ui.label(
                    egui::RichText::new(format!("(translate unavailable: {err})"))
                        .color(theme::text_color()),
                );
            }
            (_, _, Err(err), _) => {
                ui.label(
                    egui::RichText::new(format!("(rotate unavailable: {err})"))
                        .color(theme::text_color()),
                );
            }
            (_, _, _, Err(err)) => {
                ui.label(
                    egui::RichText::new(format!("(scale unavailable: {err})"))
                        .color(theme::text_color()),
                );
            }
        }
    }

    fn show_vec3_editor(
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        prim: &Prim,
        label: &str,
        values: [f64; 3],
        setter: fn(&Prim, f64, f64, f64) -> Result<(), dreamusd_core::DuError>,
        row_index: usize,
    ) {
        Self::paint_row_bg(ui, row_index);
        let mut edited = values;
        egui::Grid::new(("transform_row", label))
            .num_columns(6)
            .min_col_width(8.0)
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_sized(
                        [78.0, 0.0],
                        egui::Label::new(egui::RichText::new(label).color(theme::text_color())),
                    );
                });

                for (index, axis) in ["x", "y", "z"].iter().enumerate() {
                    ui.label(theme::subdued(axis));
                    if ui
                        .add_sized(
                            [54.0, 0.0],
                            egui::DragValue::new(&mut edited[index])
                                .speed(Self::axis_speed(label))
                                .max_decimals(4),
                        )
                        .changed()
                    {
                        Self::apply_vec3_with_undo(stage, prim, setter, edited);
                    }
                }
                ui.end_row();
            });
        ui.add_space(1.0);
    }

    fn show_multi_vec3_editor(
        &self,
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        prims: &[Prim],
        label: &str,
        getter: fn(&Prim) -> Result<[f64; 3], dreamusd_core::DuError>,
        setter: fn(&Prim, f64, f64, f64) -> Result<(), dreamusd_core::DuError>,
        row_index: usize,
    ) {
        if !self.matches_search(label) {
            return;
        }

        let values = prims
            .iter()
            .filter_map(|prim| getter(prim).ok())
            .collect::<Vec<_>>();
        if values.is_empty() {
            if row_index == 0 {
                ui.label(theme::subdued("Transform is unavailable for the current selection"));
            }
            return;
        }

        let seed = values[0];
        let mixed = [
            values
                .iter()
                .any(|value| (value[0] - seed[0]).abs() > 1e-6),
            values
                .iter()
                .any(|value| (value[1] - seed[1]).abs() > 1e-6),
            values
                .iter()
                .any(|value| (value[2] - seed[2]).abs() > 1e-6),
        ];

        Self::paint_row_bg(ui, row_index);
        let mut edited = seed;
        egui::Grid::new(("multi_transform_row", label))
            .num_columns(6)
            .min_col_width(8.0)
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_sized(
                        [78.0, 0.0],
                        egui::Label::new(
                            egui::RichText::new(label).color(theme::text_color()),
                        ),
                    );
                });

                for (index, axis) in ["x", "y", "z"].iter().enumerate() {
                    let axis_label = if mixed[index] {
                        format!("{axis}*")
                    } else {
                        axis.to_string()
                    };
                    ui.label(theme::subdued(&axis_label));
                    if ui
                        .add_sized(
                            [54.0, 0.0],
                            egui::DragValue::new(&mut edited[index])
                                .speed(Self::axis_speed(label))
                                .max_decimals(4),
                        )
                        .changed()
                    {
                        Self::apply_multi_vec3_component_with_undo(
                            stage,
                            prims,
                            index,
                            edited[index],
                            getter,
                            setter,
                        );
                    }
                }
                ui.end_row();
            });

        if mixed.iter().any(|flag| *flag) {
            ui.label(theme::subdued("* mixed values"));
        }
        ui.add_space(1.0);
    }

    fn show_common_attributes(&mut self, ui: &mut egui::Ui, stage: Option<&Stage>, prim: &Prim) {
        let Ok(names) = prim.attribute_names() else {
            ui.label("(attributes unavailable)");
            return;
        };

        let mut row_index = 0usize;
        let mut shown = false;
        for name in names {
            if !Self::is_quick_attribute(&name) || !self.matches_search(&name) {
                continue;
            }

            let value = prim
                .get_attribute(&name)
                .unwrap_or_else(|_| "(error)".to_string());
            Self::show_generic_attribute_row(
                ui,
                stage,
                prim,
                &name,
                &value,
                &mut row_index,
            );
            shown = true;
        }

        if !shown {
            ui.label(theme::subdued("No common attributes match the current search"));
        }
    }

    fn show_multi_common_attributes(
        &mut self,
        ui: &mut egui::Ui,
        stage: &Stage,
        prims: &[Prim],
        status_message: &mut String,
    ) {
        if prims.is_empty() {
            ui.label(theme::subdued("No common multi-edit attributes available"));
            return;
        }

        let mut row_index = 0usize;
        let mut shown = false;

        if self.matches_search("visibility") {
            if let Some(values) = Self::collect_attr_values(prims, "visibility") {
                Self::show_multi_enum_attribute_row(
                    ui,
                    Some(stage),
                    prims,
                    "visibility",
                    &values,
                    &["inherited", "invisible"],
                    &mut row_index,
                    status_message,
                );
                shown = true;
            }
        }

        if self.matches_search("purpose") {
            if let Some(values) = Self::collect_attr_values(prims, "purpose") {
                Self::show_multi_enum_attribute_row(
                    ui,
                    Some(stage),
                    prims,
                    "purpose",
                    &values,
                    &["default", "render", "proxy", "guide"],
                    &mut row_index,
                    status_message,
                );
                shown = true;
            }
        }

        if !shown {
            ui.label(theme::subdued("No common multi-edit attributes match the current search"));
        }
    }

    fn show_all_attributes(&mut self, ui: &mut egui::Ui, stage: Option<&Stage>, prim: &Prim) {
        match prim.attribute_names() {
            Ok(names) => {
                let mut row_index = 0usize;
                let mut shown = false;
                for name in names {
                    if !self.matches_search(&name) {
                        continue;
                    }
                    let value = prim
                        .get_attribute(&name)
                        .unwrap_or_else(|_| "(error)".to_string());
                    Self::show_generic_attribute_row(
                        ui,
                        stage,
                        prim,
                        &name,
                        &value,
                        &mut row_index,
                    );
                    shown = true;
                }

                if !shown {
                    let label = if self.search_text.trim().is_empty() {
                        "(none)".to_string()
                    } else {
                        format!("No attributes match \"{}\"", self.search_text.trim())
                    };
                    ui.label(egui::RichText::new(label).color(theme::text_color()));
                }
            }
            Err(err) => {
                ui.label(egui::RichText::new(format!("Error: {err}")).color(theme::text_color()));
            }
        }
    }

    fn show_generic_attribute_row(
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        prim: &Prim,
        name: &str,
        current_value: &str,
        row_index: &mut usize,
    ) {
        const NAME_WIDTH: f32 = 92.0;
        const SCALAR_WIDTH: f32 = 88.0;
        const VEC_WIDTH: f32 = 60.0;
        const ENUM_WIDTH: f32 = 132.0;
        const TEXT_WIDTH: f32 = 180.0;

        Self::paint_row_bg(ui, *row_index);
        *row_index += 1;

        ui.horizontal(|ui| {
            ui.add_sized([NAME_WIDTH, 0.0], egui::Label::new(theme::subdued(name)));
            ui.add_space(6.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                match Self::inferred_attr_kind(name, current_value) {
                    InferredAttrKind::Bool(mut value) => {
                        if ui.checkbox(&mut value, "").changed() {
                            Self::set_attr_with_undo(
                                stage,
                                prim,
                                name,
                                if value { "true" } else { "false" },
                            );
                        }
                    }
                    InferredAttrKind::Number(mut value) => {
                        if ui
                            .add_sized(
                                [SCALAR_WIDTH, 0.0],
                                egui::DragValue::new(&mut value)
                                    .speed(Self::number_speed(name))
                                    .max_decimals(6),
                            )
                            .changed()
                        {
                            Self::set_attr_with_undo(stage, prim, name, &value.to_string());
                        }
                    }
                    InferredAttrKind::Vec3(mut value) => {
                        for index in 0..value.len() {
                            if ui
                                .add_sized(
                                    [VEC_WIDTH, 0.0],
                                    egui::DragValue::new(&mut value[index])
                                        .speed(Self::number_speed(name))
                                        .max_decimals(4),
                                )
                                .changed()
                            {
                                let packed = format!("({}, {}, {})", value[0], value[1], value[2]);
                                Self::set_attr_with_undo(stage, prim, name, &packed);
                            }
                        }
                    }
                    InferredAttrKind::Visibility(current) => {
                        let mut chosen = current.to_string();
                        egui::ComboBox::from_id_salt(("visibility", name))
                            .selected_text(&chosen)
                            .width(ENUM_WIDTH - 20.0)
                            .show_ui(ui, |ui| {
                                for option in ["inherited", "invisible"] {
                                    ui.selectable_value(&mut chosen, option.to_string(), option);
                                }
                            });
                        if chosen != current {
                            Self::set_attr_with_undo(stage, prim, name, &chosen);
                        }
                    }
                    InferredAttrKind::Purpose(current) => {
                        let mut chosen = current.to_string();
                        egui::ComboBox::from_id_salt(("purpose", name))
                            .selected_text(&chosen)
                            .width(ENUM_WIDTH - 20.0)
                            .show_ui(ui, |ui| {
                                for option in ["default", "render", "proxy", "guide"] {
                                    ui.selectable_value(&mut chosen, option.to_string(), option);
                                }
                            });
                        if chosen != current {
                            Self::set_attr_with_undo(stage, prim, name, &chosen);
                        }
                    }
                    InferredAttrKind::Text => {
                        let id = ui.make_persistent_id(format!("attr_{name}"));
                        let mut draft = ui
                            .data(|data| data.get_temp::<String>(id))
                            .unwrap_or_else(|| current_value.to_string());
                        let response = ui.add_sized(
                            [TEXT_WIDTH, 0.0],
                            egui::TextEdit::singleline(&mut draft),
                        );
                        if response.changed() {
                            ui.data_mut(|data| data.insert_temp(id, draft.clone()));
                        }
                        if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                            if draft != current_value {
                                Self::set_attr_with_undo(stage, prim, name, &draft);
                            }
                            ui.data_mut(|data| data.remove_temp::<String>(id));
                        }
                    }
                }
            });
        });
    }

    fn show_variants(&mut self, ui: &mut egui::Ui, stage: Option<&Stage>, prim: &Prim) {
        match prim.variant_sets() {
            Ok(sets) => {
                let mut row_index = 0usize;
                let mut shown = false;
                for set_name in sets {
                    if !self.matches_search(&set_name) {
                        continue;
                    }

                    Self::paint_row_bg(ui, row_index);
                    row_index += 1;
                    ui.horizontal(|ui| {
                        ui.label(theme::subdued(&set_name));
                        Self::show_variant_editor(ui, stage, prim, &set_name);
                    });
                    shown = true;
                }

                if !shown {
                    ui.label(theme::subdued("No variants match the current search"));
                }
            }
            Err(err) => {
                ui.label(egui::RichText::new(format!("Error: {err}")).color(theme::text_color()));
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
            Err(err) => {
                ui.label(format!("Error: {err}"));
            }
        }
    }

    fn show_material_summary(
        &mut self,
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        prim: &Prim,
        selected_path: &mut Option<String>,
        status_message: &mut String,
    ) {
        match prim.material_binding() {
            Ok(binding) => {
                if binding.is_empty() {
                    ui.label(egui::RichText::new("(none)").color(theme::text_color()));
                    return;
                }

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Bound: {binding}")).color(theme::text_color()),
                    );
                    if ui.small_button("Select").clicked() {
                        *selected_path = Some(binding.clone());
                        *status_message = format!("Selected material: {binding}");
                    }
                });

                if let Some(stage) = stage {
                    self.show_material_params(ui, stage, &binding, status_message);
                }
            }
            Err(err) => {
                ui.label(
                    egui::RichText::new(format!("(no material binding: {err})"))
                        .color(theme::text_color()),
                );
            }
        }
    }

    fn show_material_params(
        &mut self,
        ui: &mut egui::Ui,
        stage: &Stage,
        binding_path: &str,
        status_message: &mut String,
    ) {
        let Some(material_prim) = find_prim_recursive(stage, binding_path) else {
            ui.label("Bound material prim not found in stage");
            return;
        };

        match material_prim.material_params() {
            Ok(params) if params.is_empty() => {
                ui.label("(no editable shader inputs)");
            }
            Ok(params) => {
                let mut row_index = 0usize;
                let mut shown = false;
                for param in params {
                    if !self.matches_search(&param.name) && !self.matches_search(&param.type_name) {
                        continue;
                    }

                    Self::paint_row_bg(ui, row_index);
                    row_index += 1;
                    Self::show_material_param_row(
                        ui,
                        stage,
                        &material_prim,
                        &param,
                        status_message,
                    );
                    shown = true;
                }

                if !shown {
                    ui.label(theme::subdued("No material inputs match the current search"));
                }
            }
            Err(err) => {
                ui.label(format!("Material params unavailable: {err}"));
            }
        }
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
                .data(|data| data.get_temp::<String>(id))
                .unwrap_or_else(|| param.value.clone());
            let response = ui.text_edit_singleline(&mut edit_value);
            if response.changed() {
                ui.data_mut(|data| data.insert_temp(id, edit_value.clone()));
            }
            if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                if edit_value != param.value {
                    let _ = stage.undo_begin();
                    match material_prim.set_material_param(&param.name, &edit_value) {
                        Ok(()) => {
                            *status_message = format!("Updated material param: {}", param.name);
                        }
                        Err(err) => {
                            *status_message = format!("Material update failed: {err}");
                        }
                    }
                    let _ = stage.undo_end();
                }
                ui.data_mut(|data| data.remove_temp::<String>(id));
            }
        });
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
            .data(|data| data.get_temp::<String>(id))
            .unwrap_or_else(|| parent_path.to_string());

        ui.label(egui::RichText::new("New Parent Path").color(theme::text_color()));
        let response = ui.text_edit_singleline(&mut target_parent);
        if response.changed() {
            ui.data_mut(|data| data.insert_temp(id, target_parent.clone()));
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
                Err(err) => {
                    *status_message = format!("Move failed: {err}");
                    if let Some(stage) = stage {
                        let _ = stage.undo_end();
                    }
                }
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
            if ui
                .add(
                    egui::DragValue::new(&mut edited)
                        .speed(0.05)
                        .range(0.0..=100.0),
                )
                .changed()
            {
                Self::set_attr_with_undo(stage, prim, "intensity", &edited.to_string());
            }
        });
    }

    fn show_light_editor(ui: &mut egui::Ui, stage: Option<&Stage>, prim: &Prim) {
        let mut row_counter = 0usize;
        Self::show_light_float_editor(
            ui,
            stage,
            prim,
            "Intensity",
            &["inputs:intensity", "intensity"],
            0.0..=100_000.0,
            &mut row_counter,
        );
        Self::show_light_float_editor(
            ui,
            stage,
            prim,
            "Exposure",
            &["inputs:exposure", "exposure"],
            -20.0..=20.0,
            &mut row_counter,
        );
        Self::show_light_color_editor(
            ui,
            stage,
            prim,
            "Color",
            &["inputs:color", "color"],
            &mut row_counter,
        );
        Self::show_light_bool_editor(
            ui,
            stage,
            prim,
            "Shadow",
            &["inputs:shadow:enable", "shadow:enable", "hasShadow"],
            &mut row_counter,
        );
        Self::show_light_float_editor(
            ui,
            stage,
            prim,
            "Radius",
            &["inputs:radius", "radius"],
            0.0..=10_000.0,
            &mut row_counter,
        );
        Self::show_light_float_editor(
            ui,
            stage,
            prim,
            "Angle",
            &["inputs:angle", "angle"],
            0.0..=180.0,
            &mut row_counter,
        );
    }

    fn show_light_float_editor(
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        prim: &Prim,
        label: &str,
        attrs: &[&str],
        range: std::ops::RangeInclusive<f64>,
        row_counter: &mut usize,
    ) {
        let Some((attr_name, value)) = Self::read_first_f64_attr(prim, attrs) else {
            return;
        };

        Self::paint_row_bg(ui, *row_counter);
        *row_counter += 1;
        let mut edited = value;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label).color(theme::text_color()));
            if ui
                .add(egui::DragValue::new(&mut edited).speed(0.05).range(range))
                .changed()
            {
                Self::set_attr_with_undo(stage, prim, &attr_name, &edited.to_string());
            }
        });
    }

    fn show_light_bool_editor(
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        prim: &Prim,
        label: &str,
        attrs: &[&str],
        row_counter: &mut usize,
    ) {
        let Some((attr_name, value)) = Self::read_first_bool_attr(prim, attrs) else {
            return;
        };

        Self::paint_row_bg(ui, *row_counter);
        *row_counter += 1;
        let mut edited = value;
        ui.horizontal(|ui| {
            if ui.checkbox(&mut edited, label).changed() {
                Self::set_attr_with_undo(
                    stage,
                    prim,
                    &attr_name,
                    if edited { "true" } else { "false" },
                );
            }
        });
    }

    fn show_light_color_editor(
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        prim: &Prim,
        label: &str,
        attrs: &[&str],
        row_counter: &mut usize,
    ) {
        let Some((attr_name, mut edited)) = Self::read_first_vec3_attr(prim, attrs) else {
            return;
        };

        Self::paint_row_bg(ui, *row_counter);
        *row_counter += 1;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label).color(theme::text_color()));
            for index in 0..edited.len() {
                if ui
                    .add(
                        egui::DragValue::new(&mut edited[index])
                            .speed(0.01)
                            .range(0.0..=1000.0)
                            .max_decimals(4),
                    )
                    .changed()
                {
                    let value = format!("({}, {}, {})", edited[0], edited[1], edited[2]);
                    Self::set_attr_with_undo(stage, prim, &attr_name, &value);
                }
            }
        });
    }

    fn matches_search(&self, text: &str) -> bool {
        let query = self.search_text.trim();
        query.is_empty() || text.to_ascii_lowercase().contains(&query.to_ascii_lowercase())
    }

    fn is_quick_attribute(name: &str) -> bool {
        matches!(
            name,
            "visibility"
                | "purpose"
                | "kind"
                | "doubleSided"
                | "subdivisionScheme"
                | "orientation"
                | "xformOpOrder"
                | "active"
        ) || name.starts_with("primvars:display")
    }

    fn axis_speed(label: &str) -> f64 {
        match label {
            "Rotate" => 0.1,
            "Scale" => 0.01,
            _ => 0.05,
        }
    }

    fn number_speed(name: &str) -> f64 {
        if name.contains("intensity") || name.contains("exposure") {
            0.1
        } else if name.contains("radius") || name.contains("scale") {
            0.01
        } else {
            0.05
        }
    }

    fn is_light_type(type_name: &str) -> bool {
        matches!(
            type_name,
            "DistantLight"
                | "DomeLight"
                | "DomeLight_1"
                | "SphereLight"
                | "RectLight"
                | "DiskLight"
                | "CylinderLight"
                | "PortalLight"
        )
    }

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

    fn inferred_attr_kind<'a>(name: &'a str, value: &'a str) -> InferredAttrKind<'a> {
        if name == "visibility" {
            let trimmed = value.trim();
            if matches!(trimmed, "inherited" | "invisible") {
                return InferredAttrKind::Visibility(trimmed);
            }
        }

        if name == "purpose" {
            let trimmed = value.trim();
            if matches!(trimmed, "default" | "render" | "proxy" | "guide") {
                return InferredAttrKind::Purpose(trimmed);
            }
        }

        if let Some(bool_value) = parse_bool(value) {
            return InferredAttrKind::Bool(bool_value);
        }

        if let Some(vec3_value) = parse_vec3(value) {
            return InferredAttrKind::Vec3(vec3_value);
        }

        if let Ok(number_value) = value.trim().parse::<f64>() {
            return InferredAttrKind::Number(number_value);
        }

        InferredAttrKind::Text
    }

    fn apply_vec3_with_undo(
        stage: Option<&Stage>,
        prim: &Prim,
        setter: fn(&Prim, f64, f64, f64) -> Result<(), dreamusd_core::DuError>,
        value: [f64; 3],
    ) {
        if let Some(stage) = stage {
            let _ = stage.undo_begin();
        }
        let _ = setter(prim, value[0], value[1], value[2]);
        if let Some(stage) = stage {
            let _ = stage.undo_end();
        }
    }

    fn set_attr_with_undo(stage: Option<&Stage>, prim: &Prim, attr_name: &str, value: &str) {
        if let Some(stage) = stage {
            let _ = stage.undo_begin();
            let _ = prim.set_attribute(attr_name, value);
            let _ = stage.undo_end();
        } else {
            let _ = prim.set_attribute(attr_name, value);
        }
    }

    fn set_attr_for_many_with_undo(
        stage: Option<&Stage>,
        prims: &[Prim],
        attr_name: &str,
        value: &str,
    ) {
        if let Some(stage) = stage {
            let _ = stage.undo_begin();
            for prim in prims {
                let _ = prim.set_attribute(attr_name, value);
            }
            let _ = stage.undo_end();
        } else {
            for prim in prims {
                let _ = prim.set_attribute(attr_name, value);
            }
        }
    }

    fn apply_multi_vec3_component_with_undo(
        stage: Option<&Stage>,
        prims: &[Prim],
        axis_index: usize,
        value: f64,
        getter: fn(&Prim) -> Result<[f64; 3], dreamusd_core::DuError>,
        setter: fn(&Prim, f64, f64, f64) -> Result<(), dreamusd_core::DuError>,
    ) {
        if let Some(stage) = stage {
            let _ = stage.undo_begin();
            for prim in prims {
                if let Ok(mut current) = getter(prim) {
                    current[axis_index] = value;
                    let _ = setter(prim, current[0], current[1], current[2]);
                }
            }
            let _ = stage.undo_end();
        } else {
            for prim in prims {
                if let Ok(mut current) = getter(prim) {
                    current[axis_index] = value;
                    let _ = setter(prim, current[0], current[1], current[2]);
                }
            }
        }
    }

    fn collect_attr_values(prims: &[Prim], attr_name: &str) -> Option<Vec<String>> {
        let mut values = Vec::with_capacity(prims.len());
        for prim in prims {
            values.push(prim.get_attribute(attr_name).ok()?);
        }
        Some(values)
    }

    fn show_multi_enum_attribute_row(
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        prims: &[Prim],
        attr_name: &str,
        values: &[String],
        options: &[&str],
        row_index: &mut usize,
        status_message: &mut String,
    ) {
        const NAME_WIDTH: f32 = 92.0;
        const ENUM_WIDTH: f32 = 132.0;

        if values.is_empty() {
            return;
        }

        Self::paint_row_bg(ui, *row_index);
        *row_index += 1;

        let first = values[0].trim().to_string();
        let mixed = values.iter().skip(1).any(|value| value.trim() != first);
        let mut chosen = if mixed {
            "__mixed__".to_string()
        } else {
            first.clone()
        };

        ui.horizontal(|ui| {
            ui.add_sized([NAME_WIDTH, 0.0], egui::Label::new(theme::subdued(attr_name)));
            ui.add_space(6.0);
            egui::ComboBox::from_id_salt(("multi_attr", attr_name))
                .selected_text(if mixed { "Mixed" } else { chosen.as_str() })
                .width(ENUM_WIDTH - 20.0)
                .show_ui(ui, |ui| {
                    for option in options {
                        ui.selectable_value(&mut chosen, (*option).to_string(), *option);
                    }
                });
        });

        if chosen != "__mixed__" && (mixed || chosen != first) {
            Self::set_attr_for_many_with_undo(stage, prims, attr_name, &chosen);
            *status_message = format!("Updated {attr_name} for {} prims", prims.len());
        }
    }

    fn read_first_attr(prim: &Prim, attrs: &[&str]) -> Option<(String, String)> {
        attrs.iter().find_map(|name| {
            prim.get_attribute(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(|value| ((*name).to_string(), value))
        })
    }

    fn read_first_f64_attr(prim: &Prim, attrs: &[&str]) -> Option<(String, f64)> {
        let (name, raw) = Self::read_first_attr(prim, attrs)?;
        raw.trim().parse::<f64>().ok().map(|value| (name, value))
    }

    fn read_first_bool_attr(prim: &Prim, attrs: &[&str]) -> Option<(String, bool)> {
        let (name, raw) = Self::read_first_attr(prim, attrs)?;
        parse_bool(&raw).map(|value| (name, value))
    }

    fn read_first_vec3_attr(prim: &Prim, attrs: &[&str]) -> Option<(String, [f64; 3])> {
        let (name, raw) = Self::read_first_attr(prim, attrs)?;
        parse_vec3(&raw).map(|value| (name, value))
    }
}

enum InferredAttrKind<'a> {
    Bool(bool),
    Number(f64),
    Vec3([f64; 3]),
    Visibility(&'a str),
    Purpose(&'a str),
    Text,
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_vec3(raw: &str) -> Option<[f64; 3]> {
    let mut numbers = Vec::new();
    let mut current = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.' | 'e' | 'E') {
            current.push(ch);
        } else if !current.is_empty() {
            if let Ok(value) = current.parse::<f64>() {
                numbers.push(value);
            }
            current.clear();
        }
    }

    if !current.is_empty() {
        if let Ok(value) = current.parse::<f64>() {
            numbers.push(value);
        }
    }

    if numbers.len() == 3 {
        Some([numbers[0], numbers[1], numbers[2]])
    } else {
        None
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
