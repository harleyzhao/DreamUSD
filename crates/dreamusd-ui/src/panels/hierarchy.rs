use crate::theme;
use dreamusd_core::{Prim, Stage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HierarchyFilter {
    All,
    Xform,
    Geometry,
    Light,
    Camera,
    Material,
}

impl HierarchyFilter {
    fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Xform => "Xform",
            Self::Geometry => "Geom",
            Self::Light => "Light",
            Self::Camera => "Camera",
            Self::Material => "Material",
        }
    }

    fn all() -> &'static [HierarchyFilter] {
        &[
            Self::All,
            Self::Xform,
            Self::Geometry,
            Self::Light,
            Self::Camera,
            Self::Material,
        ]
    }
}

#[derive(Debug, Clone)]
struct HierarchyBadge {
    label: String,
    color: egui::Color32,
}

/// Panel that displays the scene hierarchy as a tree.
pub struct HierarchyPanel {
    pub selected_path: Option<String>,
    pub selected_paths: Vec<String>,
    filter_text: String,
    type_filter: HierarchyFilter,
    row_index: usize,
    dragging_path: Option<String>,
    pending_reparent: Option<(String, String)>,
}

impl Default for HierarchyPanel {
    fn default() -> Self {
        Self {
            selected_path: None,
            selected_paths: Vec::new(),
            filter_text: String::new(),
            type_filter: HierarchyFilter::All,
            row_index: 0,
            dragging_path: None,
            pending_reparent: None,
        }
    }
}

impl HierarchyPanel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_single_selection(&mut self, path: impl Into<String>) {
        let path = path.into();
        self.selected_path = Some(path.clone());
        self.selected_paths.clear();
        self.selected_paths.push(path);
    }

    pub fn clear_selection(&mut self) {
        self.selected_path = None;
        self.selected_paths.clear();
    }

    pub fn sync_selection_model(&mut self) {
        match self.selected_path.clone() {
            Some(path) => {
                if !self.selected_paths.iter().any(|selected| selected == &path) {
                    self.selected_paths.clear();
                    self.selected_paths.push(path.clone());
                }
                self.selected_path = Some(path);
            }
            None => self.selected_paths.clear(),
        }
    }

    pub fn selected_paths_snapshot(&self) -> Vec<String> {
        if self.selected_paths.is_empty() {
            self.selected_path.iter().cloned().collect()
        } else {
            self.selected_paths.clone()
        }
    }

    pub fn selection_contains(&self, path: &str) -> bool {
        self.selected_paths.iter().any(|selected| selected == path)
            || self.selected_path.as_deref() == Some(path)
    }

    pub fn add_to_selection_public(&mut self, path: &str) {
        self.add_to_selection(path);
    }

    pub fn toggle_selection_public(&mut self, path: &str) {
        self.toggle_selection(path);
    }

    pub fn replace_selection(&mut self, paths: Vec<String>) {
        let mut paths = paths;
        paths.sort();
        paths.dedup();
        self.selected_path = paths.last().cloned();
        self.selected_paths = paths;
    }

    /// Show the hierarchy panel: filters followed by the prim tree.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        stage: Option<&Stage>,
        status_message: &mut String,
    ) {
        ui.horizontal(|ui| {
            ui.label(theme::subdued("Filter"));
            ui.add(
                egui::TextEdit::singleline(&mut self.filter_text)
                    .desired_width(ui.available_width())
                    .hint_text("Search prims or paths..."),
            );
        });
        ui.add_space(4.0);

        ui.horizontal_wrapped(|ui| {
            for &filter in HierarchyFilter::all() {
                ui.selectable_value(&mut self.type_filter, filter, filter.label());
            }
        });
        ui.add_space(4.0);

        match stage {
            Some(stage) => match stage.root_prim() {
                Ok(root) => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.row_index = 0;
                        self.pending_reparent = None;
                        self.show_prim_tree(ui, &root);
                    });
                    self.finish_drag(ui, stage, status_message);
                    self.apply_pending_reparent(stage, status_message);
                }
                Err(err) => {
                    ui.label(format!("Error getting root prim: {err}"));
                }
            },
            None => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(theme::subdued("No stage loaded"));
                    ui.label(theme::subdued("Ctrl+O to open a file"));
                });
                self.dragging_path = None;
                self.pending_reparent = None;
            }
        }
    }

    /// Return an icon and its color for the prim based on its type and name.
    fn prim_icon(type_name: &str, name: &str) -> (&'static str, egui::Color32) {
        if name == "KeyLight" || name == "FillLight" || name == "AmbientLight" {
            return ("*", theme::icon_color_light());
        }

        match type_name {
            "Mesh" => ("M", theme::icon_color_mesh()),
            "Xform" => ("X", theme::icon_color_xform()),
            "Scope" => ("S", theme::icon_color_scope()),
            "Camera" => ("C", theme::icon_color_camera()),
            "Material" | "Shader" => ("m", theme::icon_color_material()),
            "DistantLight" | "DomeLight" | "DomeLight_1" | "SphereLight" | "RectLight"
            | "DiskLight" | "CylinderLight" => ("L", theme::icon_color_light()),
            "Skeleton" | "SkelRoot" => ("K", theme::icon_color_default()),
            "GeomSubset" => ("G", theme::icon_color_mesh()),
            _ if name.contains("Light") || name.contains("light") => {
                ("L", theme::icon_color_light())
            }
            _ if type_name.is_empty() => (".", theme::icon_color_default()),
            _ => ("?", theme::icon_color_default()),
        }
    }

    /// Recursively render a prim and its children as a tree.
    pub fn show_prim_tree(&mut self, ui: &mut egui::Ui, prim: &Prim) {
        let name = prim.name().unwrap_or_else(|_| "???".to_string());
        let path = prim.path().unwrap_or_default();
        let type_name = prim.type_name().unwrap_or_default();
        let children = prim.children().unwrap_or_default();

        if !self.should_show_prim(&name, &type_name, &path, &children) {
            return;
        }

        let (icon, icon_color) = Self::prim_icon(&type_name, &name);
        let badges = self.prim_badges(prim);
        let is_selected = self.selection_contains(&path);
        let row_even = self.row_index % 2 == 0;
        self.row_index += 1;

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
        let is_drop_target = self.is_valid_drop_target(&path);

        if is_selected {
            ui.painter()
                .rect_filled(full_rect, 0.0, egui::Color32::from_rgb(40, 80, 130));
        } else if is_drop_target {
            ui.painter()
                .rect_filled(full_rect, 0.0, egui::Color32::from_rgb(48, 74, 52));
        } else if !children.is_empty() && self.path_contains_selection(&path) {
            ui.painter()
                .rect_filled(full_rect, 0.0, egui::Color32::from_rgb(30, 50, 40));
        } else {
            ui.painter().rect_filled(full_rect, 0.0, row_bg);
        }

        if children.is_empty() {
            let content_response =
                self.show_tree_item(ui, icon, icon_color, &name, &type_name, &badges);
            let row_response = ui.interact(
                full_rect,
                ui.make_persistent_id((&path, "row")),
                egui::Sense::click_and_drag(),
            );
            let response = row_response.union(content_response);
            self.handle_tree_item_response(response, &path);
        } else {
            let is_root = path == "/" || name.is_empty();
            let id = ui.make_persistent_id(&path);
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                is_root,
            );
            let is_open = state.is_open();

            let (item_resp, arrow_rect) = ui
                .horizontal(|ui| {
                    let (_aid, arrow_rect) = ui.allocate_space(egui::vec2(
                        ui.spacing().indent,
                        ui.spacing().icon_width,
                    ));
                    let arrow_resp =
                        ui.interact(arrow_rect, id.with("arr"), egui::Sense::click());
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
                    ui.painter()
                        .add(egui::Shape::convex_polygon(pts, col, egui::Stroke::NONE));
                    (
                        self.show_tree_item(ui, icon, icon_color, &name, &type_name, &badges),
                        arrow_rect,
                    )
                })
                .inner;

            let clickable_rect = egui::Rect::from_min_max(
                egui::pos2(arrow_rect.right(), full_rect.top()),
                full_rect.max,
            );
            let row_response = ui.interact(
                clickable_rect,
                ui.make_persistent_id((&path, "row")),
                egui::Sense::click_and_drag(),
            );
            self.handle_tree_item_response(row_response.union(item_resp), &path);

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
        icon_color: egui::Color32,
        name: &str,
        type_name: &str,
        badges: &[HierarchyBadge],
    ) -> egui::Response {
        let response = ui
            .horizontal(|ui| {
                let icon_label = egui::RichText::new(icon).color(icon_color).monospace();
                ui.label(icon_label);

                let name_text = if name.is_empty() {
                    "/"
                } else {
                    name
                };
                ui.label(egui::RichText::new(name_text).color(theme::text_color()));

                if !type_name.is_empty() && type_name != "Xform" {
                    ui.label(theme::subdued(&format!("· {type_name}")));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    for badge in badges.iter().rev() {
                        ui.label(
                            egui::RichText::new(&badge.label)
                                .color(badge.color)
                                .monospace()
                                .small(),
                        );
                    }
                });
            })
            .response;

        ui.interact(response.rect, response.id, egui::Sense::click_and_drag())
    }

    fn handle_tree_item_response(&mut self, response: egui::Response, path: &str) {
        if response.clicked() {
            let modifiers = response.ctx.input(|input| input.modifiers);
            if modifiers.command {
                self.toggle_selection(path);
            } else if modifiers.shift {
                self.add_to_selection(path);
            } else {
                self.set_single_selection(path.to_string());
            }
        }

        if response.drag_started() && path != "/" {
            self.dragging_path = Some(path.to_string());
        }

        if response.hovered()
            && self.dragging_path.is_some()
            && response.ctx.input(|input| input.pointer.any_released())
            && self.is_valid_drop_target(path)
        {
            if let Some(source_path) = self.dragging_path.clone() {
                self.pending_reparent = Some((source_path, path.to_string()));
            }
        }

        if response.hovered() && self.is_valid_drop_target(path) {
            response.ctx.output_mut(|output| {
                output.cursor_icon = egui::CursorIcon::PointingHand;
            });
        }
    }

    fn should_show_prim(
        &self,
        name: &str,
        type_name: &str,
        path: &str,
        children: &[Prim],
    ) -> bool {
        let matches_search = self.matches_search(name, type_name, path);
        let matches_type = self.matches_type_filter(name, type_name);
        if matches_search && matches_type {
            return true;
        }

        children.iter().any(|child| {
            let child_name = child.name().unwrap_or_default();
            let child_path = child.path().unwrap_or_default();
            let child_type = child.type_name().unwrap_or_default();
            let child_children = child.children().unwrap_or_default();
            self.should_show_prim(&child_name, &child_type, &child_path, &child_children)
        })
    }

    fn matches_search(&self, name: &str, type_name: &str, path: &str) -> bool {
        let query = self.filter_text.trim().to_ascii_lowercase();
        query.is_empty()
            || name.to_ascii_lowercase().contains(&query)
            || type_name.to_ascii_lowercase().contains(&query)
            || path.to_ascii_lowercase().contains(&query)
    }

    fn matches_type_filter(&self, name: &str, type_name: &str) -> bool {
        match self.type_filter {
            HierarchyFilter::All => true,
            HierarchyFilter::Xform => type_name == "Xform",
            HierarchyFilter::Geometry => matches!(
                type_name,
                "Mesh" | "Points" | "BasisCurves" | "Curves" | "GeomSubset"
            ),
            HierarchyFilter::Light => matches!(
                type_name,
                "DistantLight"
                    | "DomeLight"
                    | "DomeLight_1"
                    | "SphereLight"
                    | "RectLight"
                    | "DiskLight"
                    | "CylinderLight"
                    | "PortalLight"
            ) || name.contains("Light")
                || name.contains("light"),
            HierarchyFilter::Camera => type_name == "Camera",
            HierarchyFilter::Material => matches!(type_name, "Material" | "Shader"),
        }
    }

    fn prim_badges(&self, prim: &Prim) -> Vec<HierarchyBadge> {
        let mut badges = Vec::new();

        if prim
            .get_attribute("visibility")
            .ok()
            .is_some_and(|value| value.trim() == "invisible")
        {
            badges.push(HierarchyBadge {
                label: "H".to_string(),
                color: egui::Color32::from_rgb(220, 150, 80),
            });
        }

        if let Ok(purpose) = prim.get_attribute("purpose") {
            let trimmed = purpose.trim();
            if matches!(trimmed, "render" | "proxy" | "guide") {
                badges.push(HierarchyBadge {
                    label: trimmed[0..1].to_ascii_uppercase(),
                    color: match trimmed {
                        "render" => egui::Color32::from_rgb(82, 196, 132),
                        "proxy" => egui::Color32::from_rgb(86, 156, 238),
                        _ => egui::Color32::from_rgb(230, 180, 80),
                    },
                });
            }
        }

        badges
    }

    fn is_valid_drop_target(&self, target_path: &str) -> bool {
        let Some(source_path) = self.dragging_path.as_deref() else {
            return false;
        };

        if source_path == target_path {
            return false;
        }

        if source_path == "/" {
            return false;
        }

        !target_path
            .strip_prefix(source_path)
            .is_some_and(|suffix| suffix.starts_with('/'))
    }

    fn finish_drag(&mut self, ui: &egui::Ui, stage: &Stage, status_message: &mut String) {
        if let Some(dragging_path) = self.dragging_path.as_deref() {
            if let Some(pointer) = ui.ctx().pointer_hover_pos() {
                let text = format!("Move {dragging_path}");
                egui::show_tooltip_at_pointer(
                    ui.ctx(),
                    ui.layer_id(),
                    ui.make_persistent_id("hierarchy_drag_hint"),
                    |ui| {
                        ui.label(text);
                    },
                );
                if ui.ctx().input(|input| input.pointer.any_released())
                    && self.pending_reparent.is_none()
                    && !self.is_pointer_over_view(ui, pointer)
                {
                    self.dragging_path = None;
                    *status_message = "Move cancelled".to_string();
                }
            }
        }

        if ui.ctx().input(|input| input.pointer.any_released()) && self.pending_reparent.is_none() {
            self.dragging_path = None;
        }

        if self.dragging_path.is_some() && stage.root_prim().is_err() {
            self.dragging_path = None;
        }
    }

    fn is_pointer_over_view(&self, ui: &egui::Ui, pointer: egui::Pos2) -> bool {
        ui.clip_rect().contains(pointer)
    }

    fn apply_pending_reparent(&mut self, stage: &Stage, status_message: &mut String) {
        let Some((source_path, target_parent_path)) = self.pending_reparent.take() else {
            return;
        };
        self.dragging_path = None;

        let Some(source_prim) = find_prim_recursive(stage, &source_path) else {
            *status_message = "Move failed: source prim not found".to_string();
            return;
        };

        let prim_name = source_path
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or("Prim");
        let new_path = if target_parent_path == "/" {
            format!("/{prim_name}")
        } else {
            format!("{target_parent_path}/{prim_name}")
        };

        let _ = stage.undo_begin();
        match source_prim.reparent(&target_parent_path) {
            Ok(()) => {
                let _ = stage.undo_end();
                self.set_single_selection(new_path.clone());
                *status_message = format!("Moved: {source_path} -> {new_path}");
            }
            Err(err) => {
                let _ = stage.undo_end();
                *status_message = format!("Move failed: {err}");
            }
        }
    }

    fn path_contains_selection(&self, path: &str) -> bool {
        if path == "/" || path.is_empty() {
            return !self.selected_paths.is_empty() || self.selected_path.is_some();
        }

        let selections = if self.selected_paths.is_empty() {
            self.selected_path.iter().collect::<Vec<_>>()
        } else {
            self.selected_paths.iter().collect::<Vec<_>>()
        };
        if selections.is_empty() {
            return false;
        }

        selections.iter().any(|selected_path| {
            *selected_path == path
                || selected_path
                    .strip_prefix(path)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        })
    }

    fn add_to_selection(&mut self, path: &str) {
        if !self.selected_paths.iter().any(|selected| selected == path) {
            self.selected_paths.push(path.to_string());
        }
        self.selected_path = Some(path.to_string());
    }

    fn toggle_selection(&mut self, path: &str) {
        if let Some(index) = self.selected_paths.iter().position(|selected| selected == path) {
            self.selected_paths.remove(index);
            self.selected_path = self.selected_paths.last().cloned();
        } else {
            self.add_to_selection(path);
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
