// bridge/src/hydra_stub.cpp
// Stub implementations for Hydra, Render Delegates, and Material operations.
// These will be replaced with real implementations in later tasks.

#include "dreamusd_bridge.h"

extern "C" {

// --- Hydra ---
DuStatus du_hydra_create_with_vulkan(DuStage*, void*, void*, void*, uint32_t, DuHydraEngine**) { return DU_ERR_INVALID; }
DuStatus du_hydra_render(DuHydraEngine*, uint32_t, uint32_t) { return DU_ERR_INVALID; }
DuStatus du_hydra_get_vk_image(DuHydraEngine*, void*, void*, uint32_t*, uint32_t*, uint32_t*) { return DU_ERR_INVALID; }
DuStatus du_hydra_get_render_semaphore(DuHydraEngine*, void*) { return DU_ERR_INVALID; }
DuStatus du_hydra_set_camera(DuHydraEngine*, double[3], double[3], double[3]) { return DU_ERR_INVALID; }
DuStatus du_hydra_set_display_mode(DuHydraEngine*, DuDisplayMode) { return DU_ERR_INVALID; }
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

} // extern "C"
