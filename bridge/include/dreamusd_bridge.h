#ifndef DREAMUSD_BRIDGE_H
#define DREAMUSD_BRIDGE_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// Status codes
typedef enum {
    DU_OK = 0,
    DU_ERR_IO = 1,
    DU_ERR_INVALID = 2,
    DU_ERR_NULL = 3,
    DU_ERR_USD = 4,
    DU_ERR_VULKAN = 5,
} DuStatus;

// Log levels
typedef enum {
    DU_LOG_TRACE = 0,
    DU_LOG_DEBUG = 1,
    DU_LOG_INFO = 2,
    DU_LOG_WARN = 3,
    DU_LOG_ERROR = 4,
} DuLogLevel;

// Display modes
typedef enum {
    DU_DISPLAY_SMOOTH_SHADED = 0,
    DU_DISPLAY_WIREFRAME = 1,
    DU_DISPLAY_WIREFRAME_ON_SHADED = 2,
    DU_DISPLAY_FLAT_SHADED = 3,
    DU_DISPLAY_POINTS = 4,
    DU_DISPLAY_TEXTURED = 5,
} DuDisplayMode;

// Opaque handles
typedef struct DuStage DuStage;
typedef struct DuPrim DuPrim;
typedef struct DuHydraEngine DuHydraEngine;

// Material param struct
typedef struct {
    const char* name;
    const char* type;
    const char* value;
    bool is_texture;
} DuMaterialParam;

// Log callback
typedef void (*DuLogCallback)(DuLogLevel level, const char* message);

// --- Error & Logging ---
DuStatus du_get_last_error(const char** message);
DuStatus du_set_log_callback(DuLogCallback cb);

// --- Stage ---
DuStatus du_stage_open(const char* path, DuStage** out);
DuStatus du_stage_create_new(const char* path, DuStage** out);
DuStatus du_stage_save(DuStage* stage);
DuStatus du_stage_export(DuStage* stage, const char* path);
// Returns "Y" or "Z" for the stage's up axis. Defaults to "Y" if not set.
DuStatus du_stage_get_up_axis(DuStage* stage, const char** out);
void     du_stage_destroy(DuStage* stage);

// --- Prim ---
DuStatus du_prim_get_root(DuStage* stage, DuPrim** out);
DuStatus du_prim_get_children(DuPrim* prim, DuPrim*** out, uint32_t* count);
DuStatus du_prim_create(DuStage* stage, const char* path, const char* type_name, DuPrim** out);
DuStatus du_prim_remove(DuStage* stage, const char* path);
DuStatus du_prim_reparent(DuPrim* prim, const char* new_parent_path);
DuStatus du_prim_get_type_name(DuPrim* prim, const char** out);
DuStatus du_prim_get_path(DuPrim* prim, const char** out);
DuStatus du_prim_get_name(DuPrim* prim, const char** out);

// --- Transform ---
DuStatus du_xform_get_local(DuPrim* prim, double matrix[16]);
DuStatus du_xform_set_translate(DuPrim* prim, double x, double y, double z);
DuStatus du_xform_set_rotate(DuPrim* prim, double x, double y, double z);
DuStatus du_xform_set_scale(DuPrim* prim, double x, double y, double z);

// --- Attributes ---
DuStatus du_attr_get_names(DuPrim* prim, const char*** out, uint32_t* count);
DuStatus du_attr_get_value_as_string(DuPrim* prim, const char* name, char** out);
DuStatus du_attr_set_value_from_string(DuPrim* prim, const char* name, const char* value);

// --- Variants ---
DuStatus du_variant_get_sets(DuPrim* prim, const char*** out, uint32_t* count);
DuStatus du_variant_get_selection(DuPrim* prim, const char* set_name, const char** out);
DuStatus du_variant_set_selection(DuPrim* prim, const char* set_name, const char* variant);

// --- Hydra ---
// Platform-independent creation (uses Metal on macOS, GL on Linux)
DuStatus du_hydra_create(DuStage* stage, DuHydraEngine** out);
// Legacy Vulkan creation (for future GPU texture sharing)
DuStatus du_hydra_create_with_vulkan(
    DuStage* stage,
    void* vk_instance,          // VkInstance
    void* vk_physical_device,   // VkPhysicalDevice
    void* vk_device,            // VkDevice
    uint32_t queue_family_index,
    DuHydraEngine** out
);
DuStatus du_hydra_render(DuHydraEngine* engine, uint32_t width, uint32_t height);
// CPU framebuffer readback (platform-independent)
DuStatus du_hydra_get_framebuffer(DuHydraEngine* engine, uint8_t** rgba, uint32_t* width, uint32_t* height);
// Vulkan image access (when using Vulkan creation path)
DuStatus du_hydra_get_vk_image(
    DuHydraEngine* engine,
    void* image,                // VkImage*
    void* view,                 // VkImageView*
    uint32_t* format,           // VkFormat*
    uint32_t* width,
    uint32_t* height
);
DuStatus du_hydra_get_render_semaphore(DuHydraEngine* engine, void* semaphore); // VkSemaphore*
DuStatus du_hydra_set_camera(DuHydraEngine* engine, double eye[3], double target[3], double up[3]);
DuStatus du_hydra_set_display_mode(DuHydraEngine* engine, DuDisplayMode mode);
DuStatus du_hydra_set_enable_lighting(DuHydraEngine* engine, bool enable);
DuStatus du_hydra_set_enable_shadows(DuHydraEngine* engine, bool enable);
DuStatus du_hydra_set_msaa(DuHydraEngine* engine, bool enable);
// Project a 3D world point to 2D screen coordinates using the same matrices as the render.
// Returns screen_xy[0]=x, screen_xy[1]=y in pixel coordinates within the viewport.
// Returns DU_ERR_INVALID if the point is behind the camera.
DuStatus du_hydra_project_point(
    DuHydraEngine* engine,
    double world_xyz[3],
    uint32_t viewport_w, uint32_t viewport_h,
    double screen_xy[2]
);
void     du_hydra_destroy(DuHydraEngine* engine);

// --- Render Delegates ---
DuStatus du_rd_list_available(const char*** names, uint32_t* count);
DuStatus du_rd_get_current(DuHydraEngine* engine, const char** name);
DuStatus du_rd_set_current(DuHydraEngine* engine, const char* name);

// --- Material ---
DuStatus du_material_get_binding(DuPrim* prim, const char** material_path);
DuStatus du_material_get_params(DuPrim* material_prim, DuMaterialParam** params, uint32_t* count);
DuStatus du_material_set_param(DuPrim* material_prim, const char* param_name, const char* value);
DuStatus du_texture_get_thumbnail(const char* asset_path, uint8_t** rgba, uint32_t* w, uint32_t* h, uint32_t max_size);

// --- Undo/Redo ---
DuStatus du_undo_begin(DuStage* stage);
DuStatus du_undo_end(DuStage* stage);
DuStatus du_undo(DuStage* stage);
DuStatus du_redo(DuStage* stage);

// --- Memory ---
void du_free_string(char* s);
void du_free_string_array(const char** arr, uint32_t count);
void du_free_prim_array(DuPrim** arr, uint32_t count);
void du_free_material_params(DuMaterialParam* params, uint32_t count);

#ifdef __cplusplus
}
#endif

#endif // DREAMUSD_BRIDGE_H
