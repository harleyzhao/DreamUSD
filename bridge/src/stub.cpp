#include "dreamusd_bridge.h"
#include <cstring>
#include <cstdlib>

static thread_local const char* g_last_error = "not implemented";

// --- Error & Logging ---
DuStatus du_get_last_error(const char** message) {
    if (!message) return DU_ERR_NULL;
    *message = g_last_error;
    return DU_OK;
}

DuStatus du_set_log_callback(DuLogCallback /*cb*/) {
    return DU_OK;
}

// --- Stage ---
DuStatus du_stage_open(const char*, DuStage**) { return DU_ERR_INVALID; }
DuStatus du_stage_create_new(const char*, DuStage**) { return DU_ERR_INVALID; }
DuStatus du_stage_save(DuStage*) { return DU_ERR_INVALID; }
DuStatus du_stage_export(DuStage*, const char*) { return DU_ERR_INVALID; }
DuStatus du_stage_get_up_axis(DuStage*, const char**) { return DU_ERR_INVALID; }
void     du_stage_destroy(DuStage*) {}

// --- Prim ---
DuStatus du_prim_get_root(DuStage*, DuPrim**) { return DU_ERR_INVALID; }
DuStatus du_prim_get_children(DuPrim*, DuPrim***, uint32_t*) { return DU_ERR_INVALID; }
DuStatus du_prim_create(DuStage*, const char*, const char*, DuPrim**) { return DU_ERR_INVALID; }
DuStatus du_prim_remove(DuStage*, const char*) { return DU_ERR_INVALID; }
DuStatus du_prim_reparent(DuPrim*, const char*) { return DU_ERR_INVALID; }
DuStatus du_prim_get_type_name(DuPrim*, const char**) { return DU_ERR_INVALID; }
DuStatus du_prim_get_path(DuPrim*, const char**) { return DU_ERR_INVALID; }
DuStatus du_prim_get_name(DuPrim*, const char**) { return DU_ERR_INVALID; }

// --- Transform ---
DuStatus du_xform_get_local(DuPrim*, double[16]) { return DU_ERR_INVALID; }
DuStatus du_xform_set_translate(DuPrim*, double, double, double) { return DU_ERR_INVALID; }
DuStatus du_xform_set_rotate(DuPrim*, double, double, double) { return DU_ERR_INVALID; }
DuStatus du_xform_set_scale(DuPrim*, double, double, double) { return DU_ERR_INVALID; }

// --- Attributes ---
DuStatus du_attr_get_names(DuPrim*, const char***, uint32_t*) { return DU_ERR_INVALID; }
DuStatus du_attr_get_value_as_string(DuPrim*, const char*, char**) { return DU_ERR_INVALID; }
DuStatus du_attr_set_value_from_string(DuPrim*, const char*, const char*) { return DU_ERR_INVALID; }

// --- Variants ---
DuStatus du_variant_get_sets(DuPrim*, const char***, uint32_t*) { return DU_ERR_INVALID; }
DuStatus du_variant_get_selection(DuPrim*, const char*, const char**) { return DU_ERR_INVALID; }
DuStatus du_variant_set_selection(DuPrim*, const char*, const char*) { return DU_ERR_INVALID; }

// --- Hydra ---
DuStatus du_hydra_create_with_vulkan(DuStage*, void*, void*, void*, uint32_t, DuHydraEngine**) { return DU_ERR_INVALID; }
DuStatus du_hydra_render(DuHydraEngine*, uint32_t, uint32_t) { return DU_ERR_INVALID; }
DuStatus du_hydra_get_vk_image(DuHydraEngine*, void*, void*, uint32_t*, uint32_t*, uint32_t*) { return DU_ERR_INVALID; }
DuStatus du_hydra_get_render_semaphore(DuHydraEngine*, void*) { return DU_ERR_INVALID; }
DuStatus du_hydra_set_camera(DuHydraEngine*, double[3], double[3], double[3]) { return DU_ERR_INVALID; }
DuStatus du_hydra_set_display_mode(DuHydraEngine*, DuDisplayMode) { return DU_ERR_INVALID; }
DuStatus du_hydra_project_point(DuHydraEngine*, double[3], uint32_t, uint32_t, double[2]) { return DU_ERR_INVALID; }
void     du_hydra_destroy(DuHydraEngine*) {}

// --- Render Delegates ---
DuStatus du_rd_list_available(const char***, uint32_t*) { return DU_ERR_INVALID; }
DuStatus du_rd_get_current(DuHydraEngine*, const char**) { return DU_ERR_INVALID; }
DuStatus du_rd_set_current(DuHydraEngine*, const char*) { return DU_ERR_INVALID; }

// --- Material ---
DuStatus du_material_get_binding(DuPrim*, const char**) { return DU_ERR_INVALID; }
DuStatus du_material_get_params(DuPrim*, DuMaterialParam**, uint32_t*) { return DU_ERR_INVALID; }
DuStatus du_material_set_param(DuPrim*, const char*, const char*) { return DU_ERR_INVALID; }
DuStatus du_texture_get_thumbnail(const char*, uint8_t**, uint32_t*, uint32_t*, uint32_t) { return DU_ERR_INVALID; }

// --- Undo/Redo ---
DuStatus du_undo_begin(DuStage*) { return DU_ERR_INVALID; }
DuStatus du_undo_end(DuStage*) { return DU_ERR_INVALID; }
DuStatus du_undo(DuStage*) { return DU_ERR_INVALID; }
DuStatus du_redo(DuStage*) { return DU_ERR_INVALID; }

// --- Memory ---
void du_free_string(char* s) { free(s); }
void du_free_string_array(const char** arr, uint32_t count) {
    for (uint32_t i = 0; i < count; i++) free((void*)arr[i]);
    free((void*)arr);
}
void du_free_prim_array(DuPrim** arr, uint32_t) { free(arr); }
void du_free_material_params(DuMaterialParam* params, uint32_t count) {
    for (uint32_t i = 0; i < count; i++) {
        free((void*)params[i].name);
        free((void*)params[i].type);
        free((void*)params[i].value);
    }
    free(params);
}
