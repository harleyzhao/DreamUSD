#![allow(non_camel_case_types)]

use std::os::raw::c_char;
use std::ffi::c_void;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuStatus {
    Ok = 0,
    ErrIo = 1,
    ErrInvalid = 2,
    ErrNull = 3,
    ErrUsd = 4,
    ErrVulkan = 5,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuLogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuDisplayMode {
    SmoothShaded = 0,
    Wireframe = 1,
    WireframeOnShaded = 2,
    FlatShaded = 3,
    Points = 4,
    Textured = 5,
}

// ---------------------------------------------------------------------------
// Opaque types
// ---------------------------------------------------------------------------

pub enum DuStage {}
pub enum DuPrim {}
pub enum DuHydraEngine {}

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct DuMaterialParam {
    pub name: *const c_char,
    pub r#type: *const c_char,
    pub value: *const c_char,
    pub is_texture: bool,
}

// ---------------------------------------------------------------------------
// Callback types
// ---------------------------------------------------------------------------

pub type DuLogCallback = Option<extern "C" fn(level: DuLogLevel, message: *const c_char)>;

// ---------------------------------------------------------------------------
// FFI functions
// ---------------------------------------------------------------------------

extern "C" {
    // --- Error & Logging ---
    pub fn du_get_last_error(message: *mut *const c_char) -> DuStatus;
    pub fn du_set_log_callback(cb: DuLogCallback) -> DuStatus;

    // --- Stage ---
    pub fn du_stage_open(path: *const c_char, out: *mut *mut DuStage) -> DuStatus;
    pub fn du_stage_create_new(path: *const c_char, out: *mut *mut DuStage) -> DuStatus;
    pub fn du_stage_save(stage: *mut DuStage) -> DuStatus;
    pub fn du_stage_export(stage: *mut DuStage, path: *const c_char) -> DuStatus;
    pub fn du_stage_get_up_axis(stage: *mut DuStage, out: *mut *const c_char) -> DuStatus;
    pub fn du_stage_destroy(stage: *mut DuStage);

    // --- Prim ---
    pub fn du_prim_get_root(stage: *mut DuStage, out: *mut *mut DuPrim) -> DuStatus;
    pub fn du_prim_get_children(
        prim: *mut DuPrim,
        out: *mut *mut *mut DuPrim,
        count: *mut u32,
    ) -> DuStatus;
    pub fn du_prim_create(
        stage: *mut DuStage,
        path: *const c_char,
        type_name: *const c_char,
        out: *mut *mut DuPrim,
    ) -> DuStatus;
    pub fn du_prim_remove(stage: *mut DuStage, path: *const c_char) -> DuStatus;
    pub fn du_prim_reparent(prim: *mut DuPrim, new_parent_path: *const c_char) -> DuStatus;
    pub fn du_prim_get_type_name(prim: *mut DuPrim, out: *mut *const c_char) -> DuStatus;
    pub fn du_prim_get_path(prim: *mut DuPrim, out: *mut *const c_char) -> DuStatus;
    pub fn du_prim_get_name(prim: *mut DuPrim, out: *mut *const c_char) -> DuStatus;

    // --- Transform ---
    pub fn du_xform_get_local(prim: *mut DuPrim, matrix: *mut f64) -> DuStatus;
    pub fn du_xform_set_translate(prim: *mut DuPrim, x: f64, y: f64, z: f64) -> DuStatus;
    pub fn du_xform_set_rotate(prim: *mut DuPrim, x: f64, y: f64, z: f64) -> DuStatus;
    pub fn du_xform_set_scale(prim: *mut DuPrim, x: f64, y: f64, z: f64) -> DuStatus;

    // --- Attributes ---
    pub fn du_attr_get_names(
        prim: *mut DuPrim,
        out: *mut *mut *const c_char,
        count: *mut u32,
    ) -> DuStatus;
    pub fn du_attr_get_value_as_string(
        prim: *mut DuPrim,
        name: *const c_char,
        out: *mut *mut c_char,
    ) -> DuStatus;
    pub fn du_attr_set_value_from_string(
        prim: *mut DuPrim,
        name: *const c_char,
        value: *const c_char,
    ) -> DuStatus;

    // --- Variants ---
    pub fn du_variant_get_sets(
        prim: *mut DuPrim,
        out: *mut *mut *const c_char,
        count: *mut u32,
    ) -> DuStatus;
    pub fn du_variant_get_selection(
        prim: *mut DuPrim,
        set_name: *const c_char,
        out: *mut *const c_char,
    ) -> DuStatus;
    pub fn du_variant_set_selection(
        prim: *mut DuPrim,
        set_name: *const c_char,
        variant: *const c_char,
    ) -> DuStatus;

    // --- Hydra ---
    pub fn du_hydra_create(
        stage: *mut DuStage,
        out: *mut *mut DuHydraEngine,
    ) -> DuStatus;
    pub fn du_hydra_create_with_vulkan(
        stage: *mut DuStage,
        vk_instance: *mut c_void,
        vk_physical_device: *mut c_void,
        vk_device: *mut c_void,
        queue_family_index: u32,
        out: *mut *mut DuHydraEngine,
    ) -> DuStatus;
    pub fn du_hydra_render(engine: *mut DuHydraEngine, width: u32, height: u32) -> DuStatus;
    pub fn du_hydra_get_framebuffer(
        engine: *mut DuHydraEngine,
        rgba: *mut *mut u8,
        width: *mut u32,
        height: *mut u32,
    ) -> DuStatus;
    pub fn du_hydra_get_vk_image(
        engine: *mut DuHydraEngine,
        image: *mut c_void,
        view: *mut c_void,
        format: *mut u32,
        width: *mut u32,
        height: *mut u32,
    ) -> DuStatus;
    pub fn du_hydra_get_render_semaphore(
        engine: *mut DuHydraEngine,
        semaphore: *mut c_void,
    ) -> DuStatus;
    pub fn du_hydra_set_camera(
        engine: *mut DuHydraEngine,
        eye: *mut f64,
        target: *mut f64,
        up: *mut f64,
    ) -> DuStatus;
    pub fn du_hydra_set_display_mode(
        engine: *mut DuHydraEngine,
        mode: DuDisplayMode,
    ) -> DuStatus;
    pub fn du_hydra_set_enable_lighting(
        engine: *mut DuHydraEngine,
        enable: bool,
    ) -> DuStatus;
    pub fn du_hydra_set_enable_shadows(
        engine: *mut DuHydraEngine,
        enable: bool,
    ) -> DuStatus;
    pub fn du_hydra_set_msaa(
        engine: *mut DuHydraEngine,
        enable: bool,
    ) -> DuStatus;
    pub fn du_hydra_project_point(
        engine: *mut DuHydraEngine,
        world_xyz: *const f64,
        viewport_w: u32,
        viewport_h: u32,
        screen_xy: *mut f64,
    ) -> DuStatus;
    pub fn du_hydra_destroy(engine: *mut DuHydraEngine);

    // --- Render Delegates ---
    pub fn du_rd_list_available(names: *mut *mut *const c_char, count: *mut u32) -> DuStatus;
    pub fn du_rd_get_current(
        engine: *mut DuHydraEngine,
        name: *mut *const c_char,
    ) -> DuStatus;
    pub fn du_rd_set_current(engine: *mut DuHydraEngine, name: *const c_char) -> DuStatus;

    // --- Material ---
    pub fn du_material_get_binding(
        prim: *mut DuPrim,
        material_path: *mut *const c_char,
    ) -> DuStatus;
    pub fn du_material_get_params(
        material_prim: *mut DuPrim,
        params: *mut *mut DuMaterialParam,
        count: *mut u32,
    ) -> DuStatus;
    pub fn du_material_set_param(
        material_prim: *mut DuPrim,
        param_name: *const c_char,
        value: *const c_char,
    ) -> DuStatus;
    pub fn du_texture_get_thumbnail(
        asset_path: *const c_char,
        rgba: *mut *mut u8,
        w: *mut u32,
        h: *mut u32,
        max_size: u32,
    ) -> DuStatus;

    // --- Undo/Redo ---
    pub fn du_undo_begin(stage: *mut DuStage) -> DuStatus;
    pub fn du_undo_end(stage: *mut DuStage) -> DuStatus;
    pub fn du_undo(stage: *mut DuStage) -> DuStatus;
    pub fn du_redo(stage: *mut DuStage) -> DuStatus;

    // --- Memory ---
    pub fn du_free_string(s: *mut c_char);
    pub fn du_free_string_array(arr: *mut *const c_char, count: u32);
    pub fn du_free_prim_array(arr: *mut *mut DuPrim, count: u32);
    pub fn du_free_material_params(params: *mut DuMaterialParam, count: u32);
}
