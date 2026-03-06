// bridge/src/stage.cpp
// Stage operations for DreamUSD bridge.
// When compiled with OpenUSD (HAS_USD defined via CMake), uses real USD API.
// Otherwise provides stub implementations.

#include "dreamusd_bridge.h"
#include "error_internal.h"

#include <string>
#include <vector>
#include <memory>

#ifdef HAS_USD

#include <pxr/usd/usd/stage.h>
#include <pxr/usd/usd/prim.h>
#include <pxr/usd/sdf/layer.h>
#include <pxr/usd/usdGeom/tokens.h>

PXR_NAMESPACE_USING_DIRECTIVE

struct DuStage {
    UsdStageRefPtr stage;
    // Undo stack (simplified: store layer snapshots)
    std::vector<std::string> undo_stack;
    std::vector<std::string> redo_stack;
    std::string snapshot_before;
};

// Accessor for other translation units (prim.cpp, hydra.cpp, etc.)
UsdStageRefPtr du_stage_get_ptr(DuStage* stage) {
    return stage ? stage->stage : UsdStageRefPtr();
}

extern "C" {

DuStatus du_stage_open(const char* path, DuStage** out) {
    DU_CHECK_NULL(path);
    DU_CHECK_NULL(out);

    auto stage = UsdStage::Open(path);
    if (!stage) {
        du_set_last_error(std::string("Failed to open stage: ") + path);
        return DU_ERR_IO;
    }

    *out = new DuStage{stage, {}, {}, {}};
    return DU_OK;
}

DuStatus du_stage_create_new(const char* path, DuStage** out) {
    DU_CHECK_NULL(path);
    DU_CHECK_NULL(out);

    auto stage = UsdStage::CreateNew(path);
    if (!stage) {
        du_set_last_error(std::string("Failed to create stage: ") + path);
        return DU_ERR_IO;
    }

    *out = new DuStage{stage, {}, {}, {}};
    return DU_OK;
}

DuStatus du_stage_save(DuStage* stage) {
    DU_CHECK_NULL(stage);
    stage->stage->GetRootLayer()->Save();
    return DU_OK;
}

DuStatus du_stage_export(DuStage* stage, const char* path) {
    DU_CHECK_NULL(stage);
    DU_CHECK_NULL(path);
    if (!stage->stage->Export(path)) {
        du_set_last_error(std::string("Failed to export to: ") + path);
        return DU_ERR_IO;
    }
    return DU_OK;
}

DuStatus du_stage_get_up_axis(DuStage* stage, const char** out) {
    DU_CHECK_NULL(stage);
    DU_CHECK_NULL(out);

    // Read upAxis metadata from the stage's root layer
    TfToken upAxis;
    if (stage->stage->HasAuthoredMetadata(UsdGeomTokens->upAxis)) {
        VtValue val;
        stage->stage->GetMetadata(UsdGeomTokens->upAxis, &val);
        if (val.IsHolding<TfToken>()) {
            upAxis = val.UncheckedGet<TfToken>();
        }
    }

    static thread_local std::string s_axis;
    if (upAxis == UsdGeomTokens->z) {
        s_axis = "Z";
    } else {
        s_axis = "Y"; // Default
    }
    *out = s_axis.c_str();
    return DU_OK;
}

void du_stage_destroy(DuStage* stage) {
    delete stage;
}

// --- Undo/Redo (simplified via layer string serialization) ---

DuStatus du_undo_begin(DuStage* stage) {
    DU_CHECK_NULL(stage);
    std::string layer_str;
    stage->stage->GetRootLayer()->ExportToString(&layer_str);
    stage->snapshot_before = layer_str;
    return DU_OK;
}

DuStatus du_undo_end(DuStage* stage) {
    DU_CHECK_NULL(stage);
    stage->undo_stack.push_back(stage->snapshot_before);
    stage->redo_stack.clear();
    stage->snapshot_before.clear();
    return DU_OK;
}

DuStatus du_undo(DuStage* stage) {
    DU_CHECK_NULL(stage);
    if (stage->undo_stack.empty()) {
        du_set_last_error("Nothing to undo");
        return DU_ERR_INVALID;
    }

    // Save current state for redo
    std::string current;
    stage->stage->GetRootLayer()->ExportToString(&current);
    stage->redo_stack.push_back(current);

    // Restore previous state
    auto& prev = stage->undo_stack.back();
    stage->stage->GetRootLayer()->ImportFromString(prev);
    stage->undo_stack.pop_back();
    return DU_OK;
}

DuStatus du_redo(DuStage* stage) {
    DU_CHECK_NULL(stage);
    if (stage->redo_stack.empty()) {
        du_set_last_error("Nothing to redo");
        return DU_ERR_INVALID;
    }

    std::string current;
    stage->stage->GetRootLayer()->ExportToString(&current);
    stage->undo_stack.push_back(current);

    auto& next = stage->redo_stack.back();
    stage->stage->GetRootLayer()->ImportFromString(next);
    stage->redo_stack.pop_back();
    return DU_OK;
}

} // extern "C"

#else // !HAS_USD — stub implementations

struct DuStage {};

extern "C" {

DuStatus du_stage_open(const char*, DuStage**) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_stage_create_new(const char*, DuStage**) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_stage_save(DuStage*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_stage_export(DuStage*, const char*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

void du_stage_destroy(DuStage* stage) {
    delete stage;
}

DuStatus du_undo_begin(DuStage*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_undo_end(DuStage*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_undo(DuStage*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_redo(DuStage*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

} // extern "C"

#endif // HAS_USD
