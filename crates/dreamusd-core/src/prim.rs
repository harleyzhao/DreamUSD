use std::ffi::{CStr, CString};
use std::ptr;

use dreamusd_sys::*;

use crate::error::{check, DuError};

/// Safe wrapper around an opaque `DuPrim` pointer.
pub struct Prim {
    pub(crate) raw: *mut DuPrim,
}

pub struct MaterialParam {
    pub name: String,
    pub type_name: String,
    pub value: String,
    pub is_texture: bool,
}

unsafe impl Send for Prim {}

impl Prim {
    /// Return the name of this prim.
    pub fn name(&self) -> Result<String, DuError> {
        let mut out: *const std::os::raw::c_char = ptr::null();
        unsafe {
            check(du_prim_get_name(self.raw, &mut out))?;
            let s = CStr::from_ptr(out).to_string_lossy().into_owned();
            Ok(s)
        }
    }

    /// Return the full path of this prim.
    pub fn path(&self) -> Result<String, DuError> {
        let mut out: *const std::os::raw::c_char = ptr::null();
        unsafe {
            check(du_prim_get_path(self.raw, &mut out))?;
            let s = CStr::from_ptr(out).to_string_lossy().into_owned();
            Ok(s)
        }
    }

    /// Return the type name of this prim.
    pub fn type_name(&self) -> Result<String, DuError> {
        let mut out: *const std::os::raw::c_char = ptr::null();
        unsafe {
            check(du_prim_get_type_name(self.raw, &mut out))?;
            let s = CStr::from_ptr(out).to_string_lossy().into_owned();
            Ok(s)
        }
    }

    /// Return the children of this prim.
    pub fn children(&self) -> Result<Vec<Prim>, DuError> {
        let mut out: *mut *mut DuPrim = ptr::null_mut();
        let mut count: u32 = 0;
        unsafe {
            check(du_prim_get_children(self.raw, &mut out, &mut count))?;
            let mut result = Vec::with_capacity(count as usize);
            for i in 0..count as usize {
                result.push(Prim { raw: *out.add(i) });
            }
            // Only free the outer array pointer, NOT the individual DuPrim*
            // — Rust now owns them via the Prim structs in `result`.
            if !out.is_null() {
                // Pass count=0 to free only the array, not the elements
                du_free_prim_array(out, 0);
            }
            Ok(result)
        }
    }

    /// Reparent this prim under a new parent path.
    pub fn reparent(&self, new_parent_path: &str) -> Result<(), DuError> {
        let c_path = CString::new(new_parent_path)
            .map_err(|_| DuError::Invalid("path contains null byte".into()))?;
        unsafe { check(du_prim_reparent(self.raw, c_path.as_ptr())) }
    }

    /// Get the local 4x4 transformation matrix (column-major, 16 doubles).
    pub fn get_local_matrix(&self) -> Result<[f64; 16], DuError> {
        let mut matrix = [0.0f64; 16];
        unsafe {
            check(du_xform_get_local(self.raw, matrix.as_mut_ptr()))?;
        }
        Ok(matrix)
    }

    /// Get the world 4x4 transformation matrix (column-major, 16 doubles).
    pub fn get_world_matrix(&self) -> Result<[f64; 16], DuError> {
        let mut matrix = [0.0f64; 16];
        unsafe {
            check(du_xform_get_world(self.raw, matrix.as_mut_ptr()))?;
        }
        Ok(matrix)
    }

    /// Get the translate vector for this prim.
    pub fn get_translate(&self) -> Result<[f64; 3], DuError> {
        let mut xyz = [0.0f64; 3];
        unsafe {
            check(du_xform_get_translate(self.raw, xyz.as_mut_ptr()))?;
        }
        Ok(xyz)
    }

    /// Get the rotate vector for this prim in degrees.
    pub fn get_rotate(&self) -> Result<[f64; 3], DuError> {
        let mut xyz = [0.0f64; 3];
        unsafe {
            check(du_xform_get_rotate(self.raw, xyz.as_mut_ptr()))?;
        }
        Ok(xyz)
    }

    /// Get the scale vector for this prim.
    pub fn get_scale(&self) -> Result<[f64; 3], DuError> {
        let mut xyz = [1.0f64; 3];
        unsafe {
            check(du_xform_get_scale(self.raw, xyz.as_mut_ptr()))?;
        }
        Ok(xyz)
    }

    /// Get the local pivot vector for this prim.
    pub fn get_pivot(&self) -> Result<[f64; 3], DuError> {
        let mut xyz = [0.0f64; 3];
        unsafe {
            check(du_xform_get_pivot(self.raw, xyz.as_mut_ptr()))?;
        }
        Ok(xyz)
    }

    /// Get the world-space pivot position for this prim.
    pub fn get_world_pivot(&self) -> Result<[f64; 3], DuError> {
        let mut xyz = [0.0f64; 3];
        unsafe {
            check(du_xform_get_world_pivot(self.raw, xyz.as_mut_ptr()))?;
        }
        Ok(xyz)
    }

    /// Set the translation of this prim.
    pub fn set_translate(&self, x: f64, y: f64, z: f64) -> Result<(), DuError> {
        unsafe { check(du_xform_set_translate(self.raw, x, y, z)) }
    }

    /// Set the translation of this prim in world space.
    pub fn set_translate_world(&self, x: f64, y: f64, z: f64) -> Result<(), DuError> {
        unsafe { check(du_xform_set_translate_world(self.raw, x, y, z)) }
    }

    /// Set the rotation of this prim (Euler angles in degrees).
    pub fn set_rotate(&self, x: f64, y: f64, z: f64) -> Result<(), DuError> {
        unsafe { check(du_xform_set_rotate(self.raw, x, y, z)) }
    }

    /// Set the scale of this prim.
    pub fn set_scale(&self, x: f64, y: f64, z: f64) -> Result<(), DuError> {
        unsafe { check(du_xform_set_scale(self.raw, x, y, z)) }
    }

    /// Return the list of attribute names on this prim.
    pub fn attribute_names(&self) -> Result<Vec<String>, DuError> {
        let mut out: *mut *const std::os::raw::c_char = ptr::null_mut();
        let mut count: u32 = 0;
        unsafe {
            check(du_attr_get_names(self.raw, &mut out, &mut count))?;
            let mut result = Vec::with_capacity(count as usize);
            for i in 0..count as usize {
                let s = CStr::from_ptr(*out.add(i)).to_string_lossy().into_owned();
                result.push(s);
            }
            if !out.is_null() && count > 0 {
                du_free_string_array(out, count);
            }
            Ok(result)
        }
    }

    /// Get the value of an attribute as a string.
    pub fn get_attribute(&self, name: &str) -> Result<String, DuError> {
        let c_name = CString::new(name)
            .map_err(|_| DuError::Invalid("name contains null byte".into()))?;
        let mut out: *mut std::os::raw::c_char = ptr::null_mut();
        unsafe {
            check(du_attr_get_value_as_string(self.raw, c_name.as_ptr(), &mut out))?;
            let s = CStr::from_ptr(out).to_string_lossy().into_owned();
            du_free_string(out);
            Ok(s)
        }
    }

    /// Set the value of an attribute from a string.
    pub fn set_attribute(&self, name: &str, value: &str) -> Result<(), DuError> {
        let c_name = CString::new(name)
            .map_err(|_| DuError::Invalid("name contains null byte".into()))?;
        let c_value = CString::new(value)
            .map_err(|_| DuError::Invalid("value contains null byte".into()))?;
        unsafe { check(du_attr_set_value_from_string(self.raw, c_name.as_ptr(), c_value.as_ptr())) }
    }

    /// Return the list of variant set names on this prim.
    pub fn variant_sets(&self) -> Result<Vec<String>, DuError> {
        let mut out: *mut *const std::os::raw::c_char = ptr::null_mut();
        let mut count: u32 = 0;
        unsafe {
            check(du_variant_get_sets(self.raw, &mut out, &mut count))?;
            let mut result = Vec::with_capacity(count as usize);
            for i in 0..count as usize {
                let s = CStr::from_ptr(*out.add(i)).to_string_lossy().into_owned();
                result.push(s);
            }
            if !out.is_null() && count > 0 {
                du_free_string_array(out, count);
            }
            Ok(result)
        }
    }

    /// Return the list of variant names for a given variant set.
    pub fn variant_names(&self, set_name: &str) -> Result<Vec<String>, DuError> {
        let c_set = CString::new(set_name)
            .map_err(|_| DuError::Invalid("set_name contains null byte".into()))?;
        let mut out: *mut *const std::os::raw::c_char = ptr::null_mut();
        let mut count: u32 = 0;
        unsafe {
            check(du_variant_get_names(
                self.raw,
                c_set.as_ptr(),
                &mut out,
                &mut count,
            ))?;
            let mut result = Vec::with_capacity(count as usize);
            for i in 0..count as usize {
                let s = CStr::from_ptr(*out.add(i)).to_string_lossy().into_owned();
                result.push(s);
            }
            if !out.is_null() && count > 0 {
                du_free_string_array(out, count);
            }
            Ok(result)
        }
    }

    /// Get the current variant selection for a given variant set.
    pub fn get_variant_selection(&self, set_name: &str) -> Result<String, DuError> {
        let c_set = CString::new(set_name)
            .map_err(|_| DuError::Invalid("set_name contains null byte".into()))?;
        let mut out: *const std::os::raw::c_char = ptr::null();
        unsafe {
            check(du_variant_get_selection(self.raw, c_set.as_ptr(), &mut out))?;
            let s = CStr::from_ptr(out).to_string_lossy().into_owned();
            Ok(s)
        }
    }

    /// Set the variant selection for a given variant set.
    pub fn set_variant_selection(&self, set_name: &str, variant: &str) -> Result<(), DuError> {
        let c_set = CString::new(set_name)
            .map_err(|_| DuError::Invalid("set_name contains null byte".into()))?;
        let c_variant = CString::new(variant)
            .map_err(|_| DuError::Invalid("variant contains null byte".into()))?;
        unsafe { check(du_variant_set_selection(self.raw, c_set.as_ptr(), c_variant.as_ptr())) }
    }

    /// Get the material binding path for this prim.
    pub fn material_binding(&self) -> Result<String, DuError> {
        let mut out: *const std::os::raw::c_char = ptr::null();
        unsafe {
            check(du_material_get_binding(self.raw, &mut out))?;
            let s = CStr::from_ptr(out).to_string_lossy().into_owned();
            Ok(s)
        }
    }

    /// Get the editable surface shader parameters for this material prim.
    pub fn material_params(&self) -> Result<Vec<MaterialParam>, DuError> {
        let mut out: *mut DuMaterialParam = ptr::null_mut();
        let mut count: u32 = 0;
        unsafe {
            check(du_material_get_params(self.raw, &mut out, &mut count))?;
            let mut result = Vec::with_capacity(count as usize);
            for i in 0..count as usize {
                let param = &*out.add(i);
                result.push(MaterialParam {
                    name: CStr::from_ptr(param.name).to_string_lossy().into_owned(),
                    type_name: CStr::from_ptr(param.r#type)
                        .to_string_lossy()
                        .into_owned(),
                    value: CStr::from_ptr(param.value).to_string_lossy().into_owned(),
                    is_texture: param.is_texture,
                });
            }
            if !out.is_null() && count > 0 {
                du_free_material_params(out, count);
            }
            Ok(result)
        }
    }

    /// Set a surface shader parameter on this material prim.
    pub fn set_material_param(&self, name: &str, value: &str) -> Result<(), DuError> {
        let c_name = CString::new(name)
            .map_err(|_| DuError::Invalid("name contains null byte".into()))?;
        let c_value = CString::new(value)
            .map_err(|_| DuError::Invalid("value contains null byte".into()))?;
        unsafe { check(du_material_set_param(self.raw, c_name.as_ptr(), c_value.as_ptr())) }
    }
}
