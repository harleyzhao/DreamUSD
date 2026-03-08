use eframe::egui;

use super::{DreamUsdApp, PickFilter};
use dreamusd_core::{Prim, Stage};
use dreamusd_render::glam::Vec3;

impl DreamUsdApp {
    pub(super) fn resolve_gizmo_target_path(&self, selected_path: &str) -> String {
        self.resolve_transform_target_path(selected_path)
            .unwrap_or_else(|| selected_path.to_string())
    }

    pub(super) fn resolve_transform_target_path(&self, selected_path: &str) -> Option<String> {
        let stage = self.stage.as_ref()?;
        let prim = find_prim_recursive(stage, selected_path)?;
        let name = prim.name().unwrap_or_default();
        if prim.type_name().ok().as_deref() == Some("Xform") {
            if is_generic_group_name(&name) {
                return parent_xform_path(stage, selected_path)
                    .or_else(|| Some(selected_path.to_string()));
            }
            return Some(selected_path.to_string());
        }

        let nearest_xform = nearest_xform_path(stage, selected_path)?;
        let nearest_xform_prim = find_prim_recursive(stage, &nearest_xform)?;
        if is_generic_group_name(&nearest_xform_prim.name().unwrap_or_default()) {
            return parent_xform_path(stage, &nearest_xform)
                .or(Some(nearest_xform));
        }
        Some(nearest_xform)
    }

    pub(super) fn pick_prim_in_viewport(
        &self,
        rect: egui::Rect,
        pointer_pos: egui::Pos2,
    ) -> Option<String> {
        let (viewport_w, viewport_h) = self.viewport_render_size(rect);
        let (screen_x, screen_y) = self.viewport_screen_to_render(rect, pointer_pos);

        if let Some(ref hydra) = self.hydra {
            if let Ok(path) = hydra.pick_prim(
                screen_x,
                screen_y,
                viewport_w,
                viewport_h,
            ) {
                if !path.is_empty() && path != "/" {
                    if let Some(prim) = self
                        .stage
                        .as_ref()
                        .and_then(|stage| find_prim_recursive(stage, &path))
                    {
                        if self.matches_pick_filter(&prim) {
                            return Some(path);
                        }
                    }
                }
            }
        }

        self.pick_prim_at_screen(rect, pointer_pos)
    }

    pub(super) fn pick_prim_at_screen(
        &self,
        rect: egui::Rect,
        pointer_pos: egui::Pos2,
    ) -> Option<String> {
        let stage = self.stage.as_ref()?;
        let root = stage.root_prim().ok()?;
        let mut best: Option<(String, f32)> = None;
        self.pick_in_subtree(&root, rect, pointer_pos, &mut best);
        best.map(|(path, _)| path)
    }

    fn pick_in_subtree(
        &self,
        prim: &Prim,
        rect: egui::Rect,
        pointer_pos: egui::Pos2,
        best: &mut Option<(String, f32)>,
    ) {
        if !self.matches_pick_filter(prim) {
            if let Ok(children) = prim.children() {
                for child in children {
                    self.pick_in_subtree(&child, rect, pointer_pos, best);
                }
            }
            return;
        }

        if let (Ok(path), Some(world_pos)) = (prim.path(), self.get_prim_position(prim)) {
            if path != "/" {
                if let Some(screen_pos) = self.hydra_project(world_pos, rect) {
                    let distance = screen_pos.distance(pointer_pos);
                    let within_pick_radius = distance <= 18.0;
                    let is_better = best
                        .as_ref()
                        .map(|(_, best_distance)| distance < *best_distance)
                        .unwrap_or(true);
                    if within_pick_radius && is_better {
                        *best = Some((path, distance));
                    }
                }
            }
        }

        if let Ok(children) = prim.children() {
            for child in children {
                self.pick_in_subtree(&child, rect, pointer_pos, best);
            }
        }
    }

    pub(super) fn matches_pick_filter(&self, prim: &Prim) -> bool {
        matches_pick_filter(prim, self.pick_filter)
    }
}

fn matches_pick_filter(prim: &Prim, filter: PickFilter) -> bool {
    if filter == PickFilter::All {
        return true;
    }

    let type_name = prim.type_name().unwrap_or_default();
    let name = prim.name().unwrap_or_default();

    match filter {
        PickFilter::All => true,
        PickFilter::Geometry => matches!(type_name.as_str(), "Mesh" | "Points" | "BasisCurves" | "Curves" | "GeomSubset"),
        PickFilter::Lights => matches!(
            type_name.as_str(),
            "DistantLight" | "DomeLight" | "SphereLight" | "RectLight" | "DiskLight" | "CylinderLight"
        ) || name.contains("Light") || name.contains("light"),
        PickFilter::Cameras => type_name == "Camera",
    }
}

fn nearest_xform_path(stage: &Stage, path: &str) -> Option<String> {
    let mut current_path = path.to_string();
    loop {
        let prim = find_prim_recursive(stage, &current_path)?;
        if prim.type_name().ok().as_deref() == Some("Xform") {
            return Some(current_path);
        }
        current_path = parent_prim_path(&current_path)?;
    }
}

fn parent_xform_path(stage: &Stage, path: &str) -> Option<String> {
    let mut current_path = parent_prim_path(path)?;
    loop {
        let prim = find_prim_recursive(stage, &current_path)?;
        if prim.type_name().ok().as_deref() == Some("Xform") {
            return Some(current_path);
        }
        current_path = parent_prim_path(&current_path)?;
    }
}

fn is_generic_group_name(name: &str) -> bool {
    matches!(
        name,
        "Geom" | "Geometry" | "geo" | "geometry" | "mesh" | "meshes" | "render" | "Render"
    )
}

fn parent_prim_path(path: &str) -> Option<String> {
    if path.is_empty() || path == "/" {
        return None;
    }
    let trimmed = path.trim_end_matches('/');
    let idx = trimmed.rfind('/')?;
    if idx == 0 {
        Some("/".to_string())
    } else {
        Some(trimmed[..idx].to_string())
    }
}

pub(super) fn find_prim_recursive(stage: &Stage, target_path: &str) -> Option<Prim> {
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

pub(super) fn prim_position(prim: &Prim) -> Option<Vec3> {
    if let Ok(pivot) = prim.get_world_pivot() {
        return Some(Vec3::new(
            pivot[0] as f32,
            pivot[1] as f32,
            pivot[2] as f32,
        ));
    }
    let mat = prim.get_world_matrix().ok()?;
    Some(Vec3::new(mat[12] as f32, mat[13] as f32, mat[14] as f32))
}
