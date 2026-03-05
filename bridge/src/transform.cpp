// bridge/src/transform.cpp
// Transform get/set operations for DreamUSD bridge.
// When compiled with OpenUSD (HAS_USD defined via CMake), uses real USD API.
// Otherwise provides stub implementations.

#include "dreamusd_bridge.h"
#include "error_internal.h"

#include <cstring>

#ifdef HAS_USD

#include <pxr/usd/usd/prim.h>
#include <pxr/usd/usdGeom/xformable.h>
#include <pxr/usd/usdGeom/xformCommonAPI.h>
#include <pxr/base/gf/matrix4d.h>
#include <pxr/base/gf/vec3d.h>
#include <pxr/base/gf/vec3f.h>

PXR_NAMESPACE_USING_DIRECTIVE

// Forward declaration — DuPrim defined in prim.cpp
struct DuPrim;
extern UsdPrim du_prim_get_usd(DuPrim* prim);

extern "C" {

DuStatus du_xform_get_local(DuPrim* prim, double matrix[16]) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(matrix);

    UsdGeomXformable xformable(du_prim_get_usd(prim));
    if (!xformable) {
        du_set_last_error("Prim is not Xformable");
        return DU_ERR_INVALID;
    }

    bool resetsXformStack;
    GfMatrix4d localXform;
    xformable.GetLocalTransformation(&localXform, &resetsXformStack);

    const double* data = localXform.GetArray();
    memcpy(matrix, data, 16 * sizeof(double));
    return DU_OK;
}

DuStatus du_xform_set_translate(DuPrim* prim, double x, double y, double z) {
    DU_CHECK_NULL(prim);

    UsdGeomXformCommonAPI api(du_prim_get_usd(prim));
    if (!api) {
        du_set_last_error("Cannot create XformCommonAPI for prim");
        return DU_ERR_INVALID;
    }
    api.SetTranslate(GfVec3d(x, y, z));
    return DU_OK;
}

DuStatus du_xform_set_rotate(DuPrim* prim, double x, double y, double z) {
    DU_CHECK_NULL(prim);

    UsdGeomXformCommonAPI api(du_prim_get_usd(prim));
    if (!api) {
        du_set_last_error("Cannot create XformCommonAPI for prim");
        return DU_ERR_INVALID;
    }
    api.SetRotate(GfVec3f((float)x, (float)y, (float)z));
    return DU_OK;
}

DuStatus du_xform_set_scale(DuPrim* prim, double x, double y, double z) {
    DU_CHECK_NULL(prim);

    UsdGeomXformCommonAPI api(du_prim_get_usd(prim));
    if (!api) {
        du_set_last_error("Cannot create XformCommonAPI for prim");
        return DU_ERR_INVALID;
    }
    api.SetScale(GfVec3f((float)x, (float)y, (float)z));
    return DU_OK;
}

} // extern "C"

#else // !HAS_USD — stub implementations

extern "C" {

DuStatus du_xform_get_local(DuPrim*, double[16]) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_xform_set_translate(DuPrim*, double, double, double) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_xform_set_rotate(DuPrim*, double, double, double) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_xform_set_scale(DuPrim*, double, double, double) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

} // extern "C"

#endif // HAS_USD
