// bridge/src/prim.cpp
// Prim CRUD, attributes, variants, and memory operations for DreamUSD bridge.
// When compiled with OpenUSD (HAS_USD defined via CMake), uses real USD API.
// Otherwise provides stub implementations.

#include "dreamusd_bridge.h"
#include "error_internal.h"

#include <cstdlib>
#include <cstring>
#include <string>

#ifdef HAS_USD

#include <pxr/usd/usd/stage.h>
#include <pxr/usd/usd/editContext.h>
#include <pxr/usd/usd/prim.h>
#include <pxr/usd/usd/attribute.h>
#include <pxr/usd/usd/variantSets.h>
#include <pxr/usd/sdf/path.h>
#include <pxr/usd/sdf/changeBlock.h>
#include <pxr/usd/sdf/copyUtils.h>
#include <pxr/usd/usdGeom/bboxCache.h>
#include <pxr/usd/usdGeom/imageable.h>
#include <pxr/base/vt/value.h>
#include <pxr/base/tf/token.h>

#include <sstream>
#include <vector>

PXR_NAMESPACE_USING_DIRECTIVE

// Forward declaration — DuStage defined in stage.cpp
struct DuStage;
extern UsdStageRefPtr du_stage_get_ptr(DuStage* stage);

struct DuPrim {
    UsdPrim prim;
};

// Accessor for other translation units (transform.cpp)
UsdPrim du_prim_get_usd(DuPrim* p) {
    return p ? p->prim : UsdPrim();
}

static char* du_strdup(const std::string& s) {
    char* out = (char*)malloc(s.size() + 1);
    if (out) memcpy(out, s.c_str(), s.size() + 1);
    return out;
}

extern "C" {

// --- Prim CRUD ---

DuStatus du_prim_get_root(DuStage* stage, DuPrim** out) {
    DU_CHECK_NULL(stage);
    DU_CHECK_NULL(out);
    auto stagePtr = du_stage_get_ptr(stage);
    if (!stagePtr) {
        du_set_last_error("Stage pointer is invalid");
        return DU_ERR_INVALID;
    }
    *out = new DuPrim{stagePtr->GetPseudoRoot()};
    return DU_OK;
}

DuStatus du_prim_get_children(DuPrim* prim, DuPrim*** out, uint32_t* count) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(out);
    DU_CHECK_NULL(count);

    auto children = prim->prim.GetChildren();
    std::vector<UsdPrim> childVec(children.begin(), children.end());

    *count = (uint32_t)childVec.size();
    if (childVec.empty()) {
        *out = nullptr;
        return DU_OK;
    }
    *out = (DuPrim**)malloc(sizeof(DuPrim*) * childVec.size());
    for (size_t i = 0; i < childVec.size(); i++) {
        (*out)[i] = new DuPrim{childVec[i]};
    }
    return DU_OK;
}

DuStatus du_prim_create(DuStage* stage, const char* path, const char* type_name, DuPrim** out) {
    DU_CHECK_NULL(stage);
    DU_CHECK_NULL(path);
    DU_CHECK_NULL(type_name);
    DU_CHECK_NULL(out);

    auto stagePtr = du_stage_get_ptr(stage);
    auto prim = stagePtr->DefinePrim(SdfPath(path), TfToken(type_name));
    if (!prim.IsValid()) {
        du_set_last_error(std::string("Failed to create prim at: ") + path);
        return DU_ERR_USD;
    }
    *out = new DuPrim{prim};
    return DU_OK;
}

DuStatus du_prim_remove(DuStage* stage, const char* path) {
    DU_CHECK_NULL(stage);
    DU_CHECK_NULL(path);

    auto stagePtr = du_stage_get_ptr(stage);
    const SdfPath primPath(path);

    bool removedAny = false;
    {
        SdfChangeBlock changeBlock;

        if (const SdfLayerHandle sessionLayer = stagePtr->GetSessionLayer()) {
            UsdEditContext sessionContext(
                stagePtr,
                stagePtr->GetEditTargetForLocalLayer(sessionLayer));
            removedAny = stagePtr->RemovePrim(primPath) || removedAny;
        }

        if (const SdfLayerHandle rootLayer = stagePtr->GetRootLayer()) {
            UsdEditContext rootContext(
                stagePtr,
                stagePtr->GetEditTargetForLocalLayer(rootLayer));
            removedAny = stagePtr->RemovePrim(primPath) || removedAny;
        }
    }

    if (!removedAny) {
        UsdPrim prim = stagePtr->GetPrimAtPath(primPath);
        if (prim && prim.SetActive(false)) {
            return DU_OK;
        }

        du_set_last_error(std::string("Failed to remove prim at: ") + path);
        return DU_ERR_USD;
    }
    return DU_OK;
}

DuStatus du_prim_reparent(DuPrim* prim, const char* new_parent_path) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(new_parent_path);

    auto stage = prim->prim.GetStage();
    auto layer = stage->GetEditTarget().GetLayer();
    auto srcPath = prim->prim.GetPath();
    auto dstParent = SdfPath(new_parent_path);
    auto dstPath = dstParent.AppendChild(srcPath.GetNameToken());

    if (!SdfCopySpec(layer, srcPath, layer, dstPath)) {
        du_set_last_error("Failed to copy prim for reparent");
        return DU_ERR_USD;
    }
    stage->RemovePrim(srcPath);
    prim->prim = stage->GetPrimAtPath(dstPath);
    return DU_OK;
}

DuStatus du_prim_get_type_name(DuPrim* prim, const char** out) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(out);
    // Returns pointer to internal TfToken string — stable for lifetime of prim
    *out = prim->prim.GetTypeName().GetText();
    return DU_OK;
}

DuStatus du_prim_get_path(DuPrim* prim, const char** out) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(out);
    // GetPath() returns a temporary SdfPath; store the string to keep it alive
    static thread_local std::string s_path;
    s_path = prim->prim.GetPath().GetString();
    *out = s_path.c_str();
    return DU_OK;
}

DuStatus du_prim_get_name(DuPrim* prim, const char** out) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(out);
    *out = prim->prim.GetName().GetText();
    return DU_OK;
}

DuStatus du_prim_get_world_bounds(DuPrim* prim, double min_xyz[3], double max_xyz[3]) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(min_xyz);
    DU_CHECK_NULL(max_xyz);

    DU_TRY({
        if (!prim->prim.IsValid()) {
            du_set_last_error("Prim is invalid");
            return DU_ERR_INVALID;
        }

        TfTokenVector includedPurposes;
        includedPurposes.push_back(UsdGeomTokens->default_);
        includedPurposes.push_back(UsdGeomTokens->render);
        includedPurposes.push_back(UsdGeomTokens->proxy);
        includedPurposes.push_back(UsdGeomTokens->guide);
        UsdGeomBBoxCache bboxCache(
            UsdTimeCode::Default(),
            includedPurposes,
            /*useExtentsHint=*/true,
            /*ignoreVisibility=*/false
        );
        const GfRange3d bounds = bboxCache.ComputeWorldBound(prim->prim).ComputeAlignedBox();
        if (bounds.IsEmpty()) {
            du_set_last_error("Prim has empty world bounds");
            return DU_ERR_INVALID;
        }

        const GfVec3d min = bounds.GetMin();
        const GfVec3d max = bounds.GetMax();
        for (int i = 0; i < 3; ++i) {
            min_xyz[i] = min[i];
            max_xyz[i] = max[i];
        }
        return DU_OK;
    });

    return DU_ERR_USD;
}

// --- Attributes ---

DuStatus du_attr_get_names(DuPrim* prim, const char*** out, uint32_t* count) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(out);
    DU_CHECK_NULL(count);

    auto attrs = prim->prim.GetAttributes();
    *count = (uint32_t)attrs.size();
    if (attrs.empty()) {
        *out = nullptr;
        return DU_OK;
    }
    *out = (const char**)malloc(sizeof(const char*) * attrs.size());
    for (size_t i = 0; i < attrs.size(); i++) {
        (*out)[i] = du_strdup(attrs[i].GetName().GetString());
    }
    return DU_OK;
}

DuStatus du_attr_get_value_as_string(DuPrim* prim, const char* name, char** out) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(name);
    DU_CHECK_NULL(out);

    auto attr = prim->prim.GetAttribute(TfToken(name));
    if (!attr.IsValid()) {
        du_set_last_error(std::string("Attribute not found: ") + name);
        return DU_ERR_INVALID;
    }

    VtValue val;
    attr.Get(&val);

    std::ostringstream ss;
    ss << val;
    *out = du_strdup(ss.str());
    return DU_OK;
}

DuStatus du_attr_set_value_from_string(DuPrim* prim, const char* name, const char* value) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(name);
    DU_CHECK_NULL(value);

    auto attr = prim->prim.GetAttribute(TfToken(name));
    if (!attr.IsValid()) {
        du_set_last_error(std::string("Attribute not found: ") + name);
        return DU_ERR_INVALID;
    }

    // Type-aware parsing based on current attribute value type
    VtValue current;
    attr.Get(&current);

    if (current.IsHolding<double>()) {
        attr.Set(std::stod(value));
    } else if (current.IsHolding<float>()) {
        attr.Set(std::stof(value));
    } else if (current.IsHolding<int>()) {
        attr.Set(std::stoi(value));
    } else if (current.IsHolding<std::string>()) {
        attr.Set(std::string(value));
    } else if (current.IsHolding<bool>()) {
        attr.Set(std::string(value) == "true" || std::string(value) == "1");
    } else {
        du_set_last_error("Unsupported attribute type for string-based set");
        return DU_ERR_INVALID;
    }

    return DU_OK;
}

// --- Variants ---

DuStatus du_variant_get_sets(DuPrim* prim, const char*** out, uint32_t* count) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(out);
    DU_CHECK_NULL(count);

    auto variantSets = prim->prim.GetVariantSets();
    auto names = variantSets.GetNames();
    *count = (uint32_t)names.size();
    if (names.empty()) {
        *out = nullptr;
        return DU_OK;
    }
    *out = (const char**)malloc(sizeof(const char*) * names.size());
    for (size_t i = 0; i < names.size(); i++) {
        (*out)[i] = du_strdup(names[i]);
    }
    return DU_OK;
}

DuStatus du_variant_get_names(DuPrim* prim, const char* set_name, const char*** out, uint32_t* count) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(set_name);
    DU_CHECK_NULL(out);
    DU_CHECK_NULL(count);

    auto variantSet = prim->prim.GetVariantSet(set_name);
    if (!variantSet.IsValid()) {
        du_set_last_error(std::string("Variant set not found: ") + set_name);
        return DU_ERR_INVALID;
    }

    auto names = variantSet.GetVariantNames();
    *count = (uint32_t)names.size();
    if (names.empty()) {
        *out = nullptr;
        return DU_OK;
    }
    *out = (const char**)malloc(sizeof(const char*) * names.size());
    for (size_t i = 0; i < names.size(); i++) {
        (*out)[i] = du_strdup(names[i]);
    }
    return DU_OK;
}

DuStatus du_variant_get_selection(DuPrim* prim, const char* set_name, const char** out) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(set_name);
    DU_CHECK_NULL(out);

    auto sel = prim->prim.GetVariantSets().GetVariantSelection(set_name);
    // Store in a thread_local so the pointer stays valid until next call
    static thread_local std::string s_sel;
    s_sel = sel;
    *out = s_sel.c_str();
    return DU_OK;
}

DuStatus du_variant_set_selection(DuPrim* prim, const char* set_name, const char* variant) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(set_name);
    DU_CHECK_NULL(variant);

    if (!prim->prim.GetVariantSets().SetSelection(set_name, variant)) {
        du_set_last_error(std::string("Failed to set variant: ") + set_name + " = " + variant);
        return DU_ERR_USD;
    }
    return DU_OK;
}

// --- Memory ---

void du_free_string(char* s) { free(s); }

void du_free_string_array(const char** arr, uint32_t count) {
    if (!arr) return;
    for (uint32_t i = 0; i < count; i++) free((void*)arr[i]);
    free((void*)arr);
}

void du_free_prim_array(DuPrim** arr, uint32_t count) {
    if (!arr) return;
    for (uint32_t i = 0; i < count; i++) delete arr[i];
    free(arr);
}

void du_free_material_params(DuMaterialParam* params, uint32_t count) {
    if (!params) return;
    for (uint32_t i = 0; i < count; i++) {
        free((void*)params[i].name);
        free((void*)params[i].type);
        free((void*)params[i].value);
    }
    free(params);
}

void du_free_renderer_settings(DuRendererSetting* settings, uint32_t count) {
    if (!settings) return;
    for (uint32_t i = 0; i < count; i++) {
        free((void*)settings[i].key);
        free((void*)settings[i].name);
        free((void*)settings[i].current_value);
        free((void*)settings[i].default_value);
    }
    free(settings);
}

} // extern "C"

#else // !HAS_USD — stub implementations

struct DuPrim {};

extern "C" {

DuStatus du_prim_get_root(DuStage*, DuPrim**) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_prim_get_children(DuPrim*, DuPrim***, uint32_t*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_prim_create(DuStage*, const char*, const char*, DuPrim**) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_prim_remove(DuStage*, const char*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_prim_reparent(DuPrim*, const char*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_prim_get_type_name(DuPrim*, const char**) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_prim_get_path(DuPrim*, const char**) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_prim_get_name(DuPrim*, const char**) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_attr_get_names(DuPrim*, const char***, uint32_t*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_attr_get_value_as_string(DuPrim*, const char*, char**) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_attr_set_value_from_string(DuPrim*, const char*, const char*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_variant_get_sets(DuPrim*, const char***, uint32_t*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_variant_get_names(DuPrim*, const char*, const char***, uint32_t*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_variant_get_selection(DuPrim*, const char*, const char**) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_variant_set_selection(DuPrim*, const char*, const char*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

// --- Memory ---

void du_free_string(char* s) { free(s); }

void du_free_string_array(const char** arr, uint32_t count) {
    if (!arr) return;
    for (uint32_t i = 0; i < count; i++) free((void*)arr[i]);
    free((void*)arr);
}

void du_free_prim_array(DuPrim** arr, uint32_t count) {
    if (!arr) return;
    for (uint32_t i = 0; i < count; i++) delete arr[i];
    free(arr);
}

void du_free_material_params(DuMaterialParam* params, uint32_t count) {
    if (!params) return;
    for (uint32_t i = 0; i < count; i++) {
        free((void*)params[i].name);
        free((void*)params[i].type);
        free((void*)params[i].value);
    }
    free(params);
}

void du_free_renderer_settings(DuRendererSetting* settings, uint32_t count) {
    if (!settings) return;
    for (uint32_t i = 0; i < count; i++) {
        free((void*)settings[i].key);
        free((void*)settings[i].name);
        free((void*)settings[i].current_value);
        free((void*)settings[i].default_value);
    }
    free(settings);
}

} // extern "C"

#endif // HAS_USD
