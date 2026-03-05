use std::ffi::CString;
use std::path::Path;
use std::ptr;

use dreamusd_sys::*;

use crate::error::{check, DuError};
use crate::prim::Prim;

/// Safe wrapper around an opaque `DuStage` pointer.
pub struct Stage {
    pub(crate) raw: *mut DuStage,
}

unsafe impl Send for Stage {}

impl Stage {
    /// Open an existing USD stage from disk.
    pub fn open(path: &Path) -> Result<Self, DuError> {
        let c_path = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| DuError::Invalid("path contains null byte".into()))?;
        let mut raw: *mut DuStage = ptr::null_mut();
        unsafe {
            check(du_stage_open(c_path.as_ptr(), &mut raw))?;
        }
        Ok(Stage { raw })
    }

    /// Create a new USD stage at the given path.
    pub fn create_new(path: &Path) -> Result<Self, DuError> {
        let c_path = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| DuError::Invalid("path contains null byte".into()))?;
        let mut raw: *mut DuStage = ptr::null_mut();
        unsafe {
            check(du_stage_create_new(c_path.as_ptr(), &mut raw))?;
        }
        Ok(Stage { raw })
    }

    /// Save the stage to its current file.
    pub fn save(&self) -> Result<(), DuError> {
        unsafe { check(du_stage_save(self.raw)) }
    }

    /// Export the stage to a different file path.
    pub fn export(&self, path: &Path) -> Result<(), DuError> {
        let c_path = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| DuError::Invalid("path contains null byte".into()))?;
        unsafe { check(du_stage_export(self.raw, c_path.as_ptr())) }
    }

    /// Get the root prim of this stage.
    pub fn root_prim(&self) -> Result<Prim, DuError> {
        let mut raw: *mut DuPrim = ptr::null_mut();
        unsafe {
            check(du_prim_get_root(self.raw, &mut raw))?;
        }
        Ok(Prim { raw })
    }

    /// Create a new prim at the given path with the given type name.
    pub fn create_prim(&self, path: &str, type_name: &str) -> Result<Prim, DuError> {
        let c_path = CString::new(path)
            .map_err(|_| DuError::Invalid("path contains null byte".into()))?;
        let c_type = CString::new(type_name)
            .map_err(|_| DuError::Invalid("type_name contains null byte".into()))?;
        let mut raw: *mut DuPrim = ptr::null_mut();
        unsafe {
            check(du_prim_create(self.raw, c_path.as_ptr(), c_type.as_ptr(), &mut raw))?;
        }
        Ok(Prim { raw })
    }

    /// Remove a prim at the given path.
    pub fn remove_prim(&self, path: &str) -> Result<(), DuError> {
        let c_path = CString::new(path)
            .map_err(|_| DuError::Invalid("path contains null byte".into()))?;
        unsafe { check(du_prim_remove(self.raw, c_path.as_ptr())) }
    }

    /// Begin an undo block.
    pub fn undo_begin(&self) -> Result<(), DuError> {
        unsafe { check(du_undo_begin(self.raw)) }
    }

    /// End an undo block.
    pub fn undo_end(&self) -> Result<(), DuError> {
        unsafe { check(du_undo_end(self.raw)) }
    }

    /// Undo the last operation.
    pub fn undo(&self) -> Result<(), DuError> {
        unsafe { check(du_undo(self.raw)) }
    }

    /// Redo the last undone operation.
    pub fn redo(&self) -> Result<(), DuError> {
        unsafe { check(du_redo(self.raw)) }
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                du_stage_destroy(self.raw);
            }
        }
    }
}
