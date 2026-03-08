use crate::theme;
use dreamusd_core::{Prim, Stage};

/// Panel that displays the scene hierarchy as a tree.
pub struct HierarchyPanel {
    pub selected_path: Option<String>,
    filter_text: String,
    row_index: usize,
}

impl Default for HierarchyPanel {
    fn default() -> Self {
        Self {
            selected_path: None,
            filter_text: String::new(),
            row_index: 0,
        }
    }
}

impl HierarchyPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the hierarchy panel: a search box followed by the prim tree.
    pub fn show(&mut self, ui: &mut egui::Ui, stage: Option<&Stage>) {
        // Search/filter bar
        ui.horizontal(|ui| {
            ui.label(theme::subdued("Filter"));
            ui.add(
                egui::TextEdit::singleline(&mut self.filter_text)
                    .desired_width(ui.available_width())
                    .hint_text("Search prims..."),
            );
        });
        ui.add_space(2.0);

        match stage {
            Some(stage) => match stage.root_prim() {
                Ok(root) => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.row_index = 0;
                        self.show_prim_tree(ui, &root);
                    });
                }
                Err(e) => {
                    ui.label(format!("Error getting root prim: {e}"));
                }
            },
            None => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(theme::subdued("No stage loaded"));
                    ui.label(theme::subdued("Ctrl+O to open a file"));
                });
            }
        }
    }

    /// Return an icon and its color for the prim based on its type and name.
    fn prim_icon(type_name: &str, name: &str) -> (&'static str, egui::Color32) {
        // Check name first for default lights (which are Xform type)
        if name == "KeyLight" || name == "FillLight" || name == "AmbientLight" {
            return ("*", theme::icon_color_light());
        }

        match type_name {
            "Mesh" => ("M", theme::icon_color_mesh()),
            "Xform" => ("X", theme::icon_color_xform()),
            "Scope" => ("S", theme::icon_color_scope()),
            "Camera" => ("C", theme::icon_color_camera()),
            "Material" | "Shader" => ("m", theme::icon_color_material()),
            "DistantLight" | "DomeLight" | "DomeLight_1" | "SphereLight"
            | "RectLight" | "DiskLight" | "CylinderLight" => ("L", theme::icon_color_light()),
            "Skeleton" | "SkelRoot" => ("K", theme::icon_color_default()),
            "GeomSubset" => ("G", theme::icon_color_mesh()),
            _ if name.contains("Light") || name.contains("light") => ("L", theme::icon_color_light()),
            _ if type_name.is_empty() => (".", theme::icon_color_default()),
            _ => ("?", theme::icon_color_default()),
        }
    }

    /// Recursively render a prim and its children as a tree.
    pub fn show_prim_tree(&mut self, ui: &mut egui::Ui, prim: &Prim) {
        let name = prim.name().unwrap_or_else(|_| "???".to_string());
        let path = prim.path().unwrap_or_else(|_| String::new());
        let type_name = prim.type_name().unwrap_or_else(|_| String::new());

        let (icon, icon_color) = Self::prim_icon(&type_name, &name);

        // Apply filter: skip prims whose name/type doesn't match (unless filter is empty)
        let filter_lower = self.filter_text.to_lowercase();
        if !self.filter_text.is_empty()
            && !name.to_lowercase().contains(&filter_lower)
            && !type_name.to_lowercase().contains(&filter_lower)
        {
            if let Ok(children) = prim.children() {
                for child in &children {
                    self.show_prim_tree(ui, child);
                }
            }
            return;
        }

        let is_selected = self.selected_path.as_deref() == Some(path.as_str());
        let children = prim.children().unwrap_or_default();

        let row_even = self.row_index % 2 == 0;
        self.row_index += 1;

        // Paint full-width alternating row background before the widget
        let row_bg = if row_even {
            egui::Color32::from_rgb(24, 24, 30)
        } else {
            egui::Color32::from_rgb(36, 36, 44)
        };
        let row_height = ui.spacing().interact_size.y;
        let full_rect = egui::Rect::from_min_size(
            egui::pos2(ui.clip_rect().left(), ui.cursor().top()),
            egui::vec2(ui.clip_rect().width(), row_height),
        );
        if is_selected {
            ui.painter().rect_filled(full_rect, 0.0, egui::Color32::from_rgb(40, 80, 130));
        } else if !is_selected && !children.is_empty() && self.path_contains_selection(&path) {
            ui.painter().rect_filled(full_rect, 0.0, egui::Color32::from_rgb(30, 50, 40));
        } else {
            ui.painter().rect_filled(full_rect, 0.0, row_bg);
        }

        if children.is_empty() {
            let response = self.show_tree_item(ui, icon, icon_color, &name, is_selected);
            if response.clicked() {
                self.selected_path = Some(path);
            }
        } else {
            let is_root = path == "/" || name.is_empty();
            let id = ui.make_persistent_id(&path);
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                is_root,
            );
            let is_open = state.is_open();
            let is_ancestor_of_selected = !is_selected && self.path_contains_selection(&path);

            let item_resp = ui.horizontal(|ui| {
                let (_aid, arrow_rect) = ui.allocate_space(egui::vec2(
                    ui.spacing().indent,
                    ui.spacing().icon_width,
                ));
                let arrow_resp = ui.interact(arrow_rect, id.with("arr"), egui::Sense::click());
                if arrow_resp.clicked() {
                    state.toggle(ui);
                }
                let col = theme::text_color();
                let c = arrow_rect.center();
                let r = (arrow_rect.width().min(arrow_rect.height()) * 0.35).max(4.0);
                let pts = if is_open {
                    vec![
                        egui::pos2(c.x - r, c.y - r * 0.6),
                        egui::pos2(c.x + r, c.y - r * 0.6),
                        egui::pos2(c.x, c.y + r * 0.7),
                    ]
                } else {
                    vec![
                        egui::pos2(c.x - r * 0.6, c.y - r),
                        egui::pos2(c.x + r * 0.7, c.y),
                        egui::pos2(c.x - r * 0.6, c.y + r),
                    ]
                };
                ui.painter().add(egui::Shape::convex_polygon(pts, col, egui::Stroke::NONE));
                let _ = is_ancestor_of_selected;
                self.show_tree_item(ui, icon, icon_color, &name, is_selected)
            }).inner;

            if item_resp.clicked() {
                self.selected_path = Some(path.clone());
            }

            if is_open {
                ui.indent(id, |ui| {
                    for child in &children {
                        self.show_prim_tree(ui, child);
                    }
                });
            }
            state.store(ui.ctx());
        }
    }

    fn show_tree_item(
        &self,
        ui: &mut egui::Ui,
        icon: &str,
        _icon_color: egui::Color32,
        name: &str,
        is_selected: bool,
    ) -> egui::Response {
        let label = egui::RichText::new(format!("{icon}  {name}"))
            .color(theme::text_color());

        ui.add(
            egui::Button::new(label)
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE)
                .min_size(egui::vec2(ui.available_width(), 0.0))
                .wrap_mode(egui::TextWrapMode::Truncate),
        )
    }

    fn path_contains_selection(&self, path: &str) -> bool {
        let Some(selected_path) = self.selected_path.as_deref() else {
            return false;
        };

        if path == "/" || path.is_empty() {
            return true;
        }

        if selected_path == path {
            return true;
        }

        selected_path
            .strip_prefix(path)
            .is_some_and(|suffix| suffix.starts_with('/'))
    }
}
