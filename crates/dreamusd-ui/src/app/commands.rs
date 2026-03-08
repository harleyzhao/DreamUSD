use super::{selection::find_prim_recursive, DreamUsdApp};
use crate::panels::HierarchyPanel;
use dreamusd_core::{HydraEngine, Stage};
use dreamusd_render::glam::Vec3;
use std::path::Path;

impl DreamUsdApp {
    pub(super) fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open USD File")
            .add_filter("USD Files", &["usd", "usda", "usdc", "usdz"])
            .pick_file()
        {
            self.open_path(&path);
        }
    }

    pub(super) fn open_path(&mut self, path: &Path) {
        match Stage::open(path) {
            Ok(stage) => {
                let up_axis = stage.up_axis();
                if up_axis == "Z" {
                    self.camera.set_z_up();
                } else {
                    self.camera.set_y_up();
                }
                self.sync_manual_clip_from_camera();

                match HydraEngine::create(&stage) {
                    Ok(engine) => {
                        self.hydra = Some(engine);
                        self.hydra_error = None;
                        self.status_message = format!("Opened: {}", path.display());
                    }
                    Err(e) => {
                        self.hydra = None;
                        self.hydra_error = Some(format!("{e}"));
                        self.status_message = format!("Opened (no renderer): {}", path.display());
                        tracing::warn!("Hydra init failed: {e}");
                    }
                }
                self.stage = Some(stage);
                self.hierarchy = HierarchyPanel::new();
                self.renderer_setting_text_edits.clear();
                self.invalidate_auto_clip();
                self.clear_viewport_texture();
                tracing::info!("Opened file: {}", path.display());
            }
            Err(e) => {
                self.status_message = format!("Failed to open: {e}");
                tracing::error!("Failed to open {}: {e}", path.display());
            }
        }
    }

    pub(super) fn save_file(&mut self) {
        if let Some(ref stage) = self.stage {
            match stage.save() {
                Ok(()) => self.status_message = "Saved".to_string(),
                Err(e) => self.status_message = format!("Save failed: {e}"),
            }
        }
    }

    pub(super) fn save_file_as(&mut self) {
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

    pub(super) fn undo(&mut self) {
        if let Some(ref stage) = self.stage {
            match stage.undo() {
                Ok(()) => self.status_message = "Undo".to_string(),
                Err(e) => self.status_message = format!("Undo failed: {e}"),
            }
        }
    }

    pub(super) fn redo(&mut self) {
        if let Some(ref stage) = self.stage {
            match stage.redo() {
                Ok(()) => self.status_message = "Redo".to_string(),
                Err(e) => self.status_message = format!("Redo failed: {e}"),
            }
        }
    }

    pub(super) fn focus_selected_prim(&mut self) {
        let Some(stage) = self.stage.as_ref() else {
            return;
        };
        let Some(selected_path) = self.hierarchy.selected_path.clone() else {
            return;
        };
        let Some(prim) = find_prim_recursive(stage, &selected_path) else {
            self.status_message = "Focus failed: selected prim not found".to_string();
            return;
        };
        let Some(position) = self.get_prim_position(&prim) else {
            self.status_message = "Focus failed: prim has no transform".to_string();
            return;
        };

        self.camera.focus_on(position, Self::focus_radius_from_position(position));
        self.invalidate_auto_clip();
        self.status_message = format!("Focused: {selected_path}");
    }

    pub(super) fn focus_stage_contents(&mut self) {
        let Some(stage) = self.stage.as_ref() else {
            return;
        };
        let Ok(root) = stage.root_prim() else {
            self.status_message = "Frame all failed: root prim unavailable".to_string();
            return;
        };

        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        let mut count = 0usize;

        fn accumulate_bounds(app: &DreamUsdApp, prim: &dreamusd_core::Prim, min: &mut Vec3, max: &mut Vec3, count: &mut usize) {
            if let Some(position) = app.get_prim_position(prim) {
                *min = min.min(position);
                *max = max.max(position);
                *count += 1;
            }
            if let Ok(children) = prim.children() {
                for child in children {
                    accumulate_bounds(app, &child, min, max, count);
                }
            }
        }

        accumulate_bounds(self, &root, &mut min, &mut max, &mut count);
        if count == 0 {
            self.status_message = "Frame all failed: scene has no frameable prims".to_string();
            return;
        }

        let center = (min + max) * 0.5;
        let radius = ((max - min) * 0.5).length().max(1.0);
        self.camera.focus_on(center, radius * 1.2);
        self.invalidate_auto_clip();
        self.status_message = "Framed scene".to_string();
    }

    pub(super) fn reset_camera_to_stage_up_axis(&mut self) {
        let z_up = self
            .stage
            .as_ref()
            .is_some_and(|stage| stage.up_axis() == "Z");
        if z_up {
            self.camera.set_z_up();
        } else {
            self.camera.set_y_up();
        }
        self.sync_manual_clip_from_camera();
        self.invalidate_auto_clip();
        self.status_message = "Camera reset".to_string();
    }

    pub(super) fn delete_selected_prim(&mut self) {
        let Some(stage) = self.stage.as_ref() else {
            return;
        };
        let mut selected_paths = self.hierarchy.selected_paths_snapshot();
        if selected_paths.is_empty() {
            return;
        }

        selected_paths.sort();
        selected_paths.dedup();
        selected_paths.retain(|path| path != "/");
        if selected_paths.is_empty() {
            self.status_message = "Cannot delete the pseudo-root".to_string();
            return;
        }

        let mut filtered = Vec::new();
        for path in selected_paths {
            let has_selected_ancestor = filtered.iter().any(|ancestor: &String| {
                path.strip_prefix(ancestor)
                    .is_some_and(|suffix| suffix.starts_with('/'))
            });
            if !has_selected_ancestor {
                filtered.push(path);
            }
        }

        let _ = stage.undo_begin();
        let mut failures = Vec::new();
        for path in &filtered {
            if let Err(err) = stage.remove_prim(path) {
                failures.push(format!("{path}: {err}"));
            }
        }
        let _ = stage.undo_end();

        if failures.is_empty() {
            self.hierarchy.clear_selection();
            self.clear_viewport_texture();
            self.status_message = if filtered.len() == 1 {
                format!("Deleted: {}", filtered[0])
            } else {
                format!("Deleted {} prims", filtered.len())
            };
        } else {
            if filtered.len() != failures.len() {
                self.hierarchy.clear_selection();
                self.clear_viewport_texture();
            }
            self.status_message = if failures.len() == 1 {
                format!("Delete failed: {}", failures[0])
            } else {
                format!("Delete completed with {} failures", failures.len())
            };
            for failure in failures {
                tracing::error!("{failure}");
            }
        }
    }

    fn focus_radius_from_position(position: Vec3) -> f32 {
        position.length().max(1.0)
    }
}
