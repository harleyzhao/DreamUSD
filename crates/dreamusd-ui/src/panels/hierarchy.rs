use dreamusd_core::{Prim, Stage};

/// Panel that displays the scene hierarchy as a tree.
pub struct HierarchyPanel {
    pub selected_path: Option<String>,
    filter_text: String,
}

impl Default for HierarchyPanel {
    fn default() -> Self {
        Self {
            selected_path: None,
            filter_text: String::new(),
        }
    }
}

impl HierarchyPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the hierarchy panel: a search box followed by the prim tree.
    pub fn show(&mut self, ui: &mut egui::Ui, stage: Option<&Stage>) {
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.filter_text);
        });

        ui.separator();

        match stage {
            Some(stage) => match stage.root_prim() {
                Ok(root) => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.show_prim_tree(ui, &root);
                    });
                }
                Err(e) => {
                    ui.label(format!("Error getting root prim: {e}"));
                }
            },
            None => {
                ui.label("No stage loaded");
            }
        }
    }

    /// Return an icon for the prim based on its type and name.
    fn prim_icon(type_name: &str, name: &str) -> &'static str {
        // Check name first for default lights (which are Xform type)
        if name == "KeyLight" { return "☀"; }
        if name == "FillLight" { return "◑"; }
        if name == "AmbientLight" { return "○"; }

        match type_name {
            "Mesh" => "△",
            "Xform" => "⊞",
            "Scope" => "▣",
            "Camera" => "⎚",
            "Material" | "Shader" => "◆",
            "DistantLight" => "☀",
            "DomeLight" | "DomeLight_1" => "◎",
            "SphereLight" => "●",
            "RectLight" => "▬",
            "DiskLight" => "◐",
            "CylinderLight" => "▮",
            "Skeleton" | "SkelRoot" => "⚷",
            "GeomSubset" => "◫",
            _ if name.contains("Light") || name.contains("light") => "◈",
            _ if type_name.is_empty() => "·",
            _ => "□",
        }
    }

    /// Recursively render a prim and its children as a tree.
    pub fn show_prim_tree(&mut self, ui: &mut egui::Ui, prim: &Prim) {
        let name = prim.name().unwrap_or_else(|_| "???".to_string());
        let path = prim.path().unwrap_or_else(|_| String::new());
        let type_name = prim.type_name().unwrap_or_else(|_| String::new());

        // Build display label with icon
        let icon = Self::prim_icon(&type_name, &name);
        let label = if type_name.is_empty() {
            format!("{icon} {name}")
        } else {
            format!("{icon} {name} ({type_name})")
        };

        // Apply filter: skip prims whose label doesn't match (unless filter is empty)
        if !self.filter_text.is_empty()
            && !label.to_lowercase().contains(&self.filter_text.to_lowercase())
        {
            // Still check children — a child might match even if the parent doesn't
            if let Ok(children) = prim.children() {
                for child in &children {
                    self.show_prim_tree(ui, child);
                }
            }
            return;
        }

        let is_selected = self.selected_path.as_deref() == Some(path.as_str());
        let children = prim.children().unwrap_or_default();

        if children.is_empty() {
            // Leaf node — use a selectable label
            if ui.selectable_label(is_selected, &label).clicked() {
                self.selected_path = Some(path);
            }
        } else {
            // Branch node — use a collapsing header
            let is_root = path == "/" || name.is_empty();
            let id = ui.make_persistent_id(&path);
            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                is_root,
            )
            .show_header(ui, |ui| {
                if ui.selectable_label(is_selected, &label).clicked() {
                    self.selected_path = Some(path.clone());
                }
            })
            .body(|ui| {
                for child in &children {
                    self.show_prim_tree(ui, child);
                }
            });
        }
    }
}
