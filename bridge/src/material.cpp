// bridge/src/material.cpp
// Material and texture operations for DreamUSD bridge.
// When compiled with OpenUSD (HAS_USD defined via CMake), uses real USD API.
// Otherwise provides stub implementations.

#include "dreamusd_bridge.h"
#include "error_internal.h"

#include <cstdlib>
#include <cstring>
#include <string>

#ifdef HAS_USD

#include <pxr/usd/usd/stage.h>
#include <pxr/usd/usd/prim.h>
#include <pxr/usd/usdShade/material.h>
#include <pxr/usd/usdShade/materialBindingAPI.h>
#include <pxr/usd/usdShade/shader.h>
#include <pxr/usd/usdShade/input.h>
#include <pxr/usd/sdf/path.h>
#include <pxr/usd/sdf/types.h>
#include <pxr/base/vt/value.h>
#include <pxr/base/tf/token.h>

#include <sstream>
#include <vector>

PXR_NAMESPACE_USING_DIRECTIVE

// Forward declaration — DuPrim defined in prim.cpp
struct DuPrim;
extern UsdPrim du_prim_get_usd(DuPrim* p);

static char* du_mat_strdup(const std::string& s) {
    char* out = (char*)malloc(s.size() + 1);
    if (out) memcpy(out, s.c_str(), s.size() + 1);
    return out;
}

extern "C" {

DuStatus du_material_get_binding(DuPrim* prim, const char** material_path) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(material_path);

    DU_TRY({
        UsdPrim usdPrim = du_prim_get_usd(prim);
        if (!usdPrim.IsValid()) {
            du_set_last_error("Invalid prim");
            return DU_ERR_INVALID;
        }

        UsdShadeMaterialBindingAPI bindingAPI(usdPrim);
        UsdShadeMaterial mat = bindingAPI.ComputeBoundMaterial();
        if (!mat) {
            du_set_last_error("No material bound to prim");
            return DU_ERR_INVALID;
        }

        static thread_local std::string s_path;
        s_path = mat.GetPath().GetString();
        *material_path = s_path.c_str();
        return DU_OK;
    });

    return DU_ERR_USD;
}

DuStatus du_material_get_params(DuPrim* material_prim, DuMaterialParam** params, uint32_t* count) {
    DU_CHECK_NULL(material_prim);
    DU_CHECK_NULL(params);
    DU_CHECK_NULL(count);

    DU_TRY({
        UsdPrim usdPrim = du_prim_get_usd(material_prim);
        if (!usdPrim.IsValid()) {
            du_set_last_error("Invalid material prim");
            return DU_ERR_INVALID;
        }

        UsdShadeMaterial mat(usdPrim);
        if (!mat) {
            du_set_last_error("Prim is not a material");
            return DU_ERR_INVALID;
        }

        // Get the surface shader output
        UsdShadeShader surfaceShader;
        UsdShadeOutput surfaceOutput = mat.GetSurfaceOutput();
        if (surfaceOutput) {
            UsdShadeConnectableAPI source;
            TfToken sourceName;
            UsdShadeAttributeType sourceType;
            if (surfaceOutput.GetConnectedSource(&source, &sourceName, &sourceType)) {
                surfaceShader = UsdShadeShader(source.GetPrim());
            }
        }

        if (!surfaceShader) {
            *params = nullptr;
            *count = 0;
            return DU_OK;
        }

        // Collect inputs from the surface shader
        std::vector<UsdShadeInput> inputs = surfaceShader.GetInputs();
        if (inputs.empty()) {
            *params = nullptr;
            *count = 0;
            return DU_OK;
        }

        *count = (uint32_t)inputs.size();
        *params = (DuMaterialParam*)malloc(sizeof(DuMaterialParam) * inputs.size());

        for (size_t i = 0; i < inputs.size(); i++) {
            const auto& input = inputs[i];

            // Name
            (*params)[i].name = du_mat_strdup(input.GetBaseName().GetString());

            // Type
            SdfValueTypeName typeName = input.GetTypeName();
            (*params)[i].type = du_mat_strdup(typeName.GetAsToken().GetString());

            // Value
            VtValue val;
            input.Get(&val);
            std::ostringstream ss;
            ss << val;
            (*params)[i].value = du_mat_strdup(ss.str());

            // Check if this input is connected to a texture
            UsdShadeConnectableAPI texSource;
            TfToken texSourceName;
            UsdShadeAttributeType texSourceType;
            bool isTexture = false;
            if (input.GetConnectedSource(&texSource, &texSourceName, &texSourceType)) {
                UsdShadeShader texShader(texSource.GetPrim());
                if (texShader) {
                    TfToken shaderId;
                    texShader.GetShaderId(&shaderId);
                    if (shaderId == TfToken("UsdUVTexture")) {
                        isTexture = true;
                    }
                }
            }
            (*params)[i].is_texture = isTexture;
        }

        return DU_OK;
    });

    return DU_ERR_USD;
}

DuStatus du_material_set_param(DuPrim* material_prim, const char* param_name, const char* value) {
    DU_CHECK_NULL(material_prim);
    DU_CHECK_NULL(param_name);
    DU_CHECK_NULL(value);

    DU_TRY({
        UsdPrim usdPrim = du_prim_get_usd(material_prim);
        if (!usdPrim.IsValid()) {
            du_set_last_error("Invalid material prim");
            return DU_ERR_INVALID;
        }

        UsdShadeMaterial mat(usdPrim);
        if (!mat) {
            du_set_last_error("Prim is not a material");
            return DU_ERR_INVALID;
        }

        // Find surface shader
        UsdShadeShader surfaceShader;
        UsdShadeOutput surfaceOutput = mat.GetSurfaceOutput();
        if (surfaceOutput) {
            UsdShadeConnectableAPI source;
            TfToken sourceName;
            UsdShadeAttributeType sourceType;
            if (surfaceOutput.GetConnectedSource(&source, &sourceName, &sourceType)) {
                surfaceShader = UsdShadeShader(source.GetPrim());
            }
        }

        if (!surfaceShader) {
            du_set_last_error("Material has no surface shader");
            return DU_ERR_INVALID;
        }

        UsdShadeInput input = surfaceShader.GetInput(TfToken(param_name));
        if (!input) {
            du_set_last_error(std::string("Shader input not found: ") + param_name);
            return DU_ERR_INVALID;
        }

        // Type-aware parsing
        VtValue current;
        input.Get(&current);

        if (current.IsHolding<float>()) {
            input.Set(std::stof(value));
        } else if (current.IsHolding<double>()) {
            input.Set(std::stod(value));
        } else if (current.IsHolding<int>()) {
            input.Set(std::stoi(value));
        } else if (current.IsHolding<std::string>()) {
            input.Set(std::string(value));
        } else if (current.IsHolding<bool>()) {
            input.Set(std::string(value) == "true" || std::string(value) == "1");
        } else if (current.IsHolding<GfVec3f>()) {
            // Parse "x y z" format
            float x = 0; float y = 0; float z = 0;
            std::istringstream iss(value);
            iss >> x >> y >> z;
            input.Set(GfVec3f(x, y, z));
        } else {
            // Try setting as string
            input.Set(std::string(value));
        }

        return DU_OK;
    });

    return DU_ERR_USD;
}

DuStatus du_texture_get_thumbnail(const char* asset_path, uint8_t** rgba, uint32_t* w, uint32_t* h, uint32_t max_size) {
    DU_CHECK_NULL(asset_path);
    DU_CHECK_NULL(rgba);
    DU_CHECK_NULL(w);
    DU_CHECK_NULL(h);

    // Placeholder: return a 64x64 gray image
    uint32_t size = (max_size > 0 && max_size < 64) ? max_size : 64;
    uint32_t pixel_count = size * size;
    uint8_t* pixels = (uint8_t*)malloc(pixel_count * 4);
    if (!pixels) {
        du_set_last_error("Failed to allocate thumbnail memory");
        return DU_ERR_IO;
    }

    // Fill with gray (128, 128, 128, 255)
    for (uint32_t i = 0; i < pixel_count; i++) {
        pixels[i * 4 + 0] = 128;
        pixels[i * 4 + 1] = 128;
        pixels[i * 4 + 2] = 128;
        pixels[i * 4 + 3] = 255;
    }

    *rgba = pixels;
    *w = size;
    *h = size;
    return DU_OK;
}

} // extern "C"

#else // !HAS_USD — stub implementations

extern "C" {

DuStatus du_material_get_binding(DuPrim*, const char**) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_material_get_params(DuPrim*, DuMaterialParam**, uint32_t*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_material_set_param(DuPrim*, const char*, const char*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_texture_get_thumbnail(const char*, uint8_t**, uint32_t*, uint32_t*, uint32_t) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

} // extern "C"

#endif // HAS_USD
