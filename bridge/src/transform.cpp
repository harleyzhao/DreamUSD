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
#include <pxr/usd/usdGeom/xformOp.h>
#include <pxr/base/gf/matrix4d.h>
#include <pxr/base/gf/vec3d.h>
#include <pxr/base/gf/vec3f.h>
#include <pxr/base/gf/vec3h.h>

PXR_NAMESPACE_USING_DIRECTIVE

// Forward declaration — DuPrim defined in prim.cpp
struct DuPrim;
extern UsdPrim du_prim_get_usd(DuPrim* prim);

static UsdGeomXformOp _DuFindEditableTranslateOp(const UsdGeomXformable& xformable) {
    const TfToken baseTranslateName =
        UsdGeomXformOp::GetOpName(UsdGeomXformOp::TypeTranslate);
    UsdGeomXformOp firstNonPivotTranslate;

    bool resetsXformStack = false;
    const auto ops = xformable.GetOrderedXformOps(&resetsXformStack);
    for (const UsdGeomXformOp& op : ops) {
        if (!op || op.IsInverseOp()) {
            continue;
        }
        if (op.GetOpType() == UsdGeomXformOp::TypeTranslate) {
            // Prefer the canonical xformOp:translate entry and never treat
            // xformOp:translate:pivot as the editable translation channel.
            if (op.GetOpName() == baseTranslateName) {
                return op;
            }
            if (!op.HasSuffix(UsdGeomTokens->pivot) && !firstNonPivotTranslate) {
                firstNonPivotTranslate = op;
            }
        }
    }
    return firstNonPivotTranslate;
}

static bool _DuSetTranslateOnOp(
    const UsdGeomXformOp& translateOp,
    const GfVec3d& translate)
{
    if (!translateOp) {
        return false;
    }
    switch (translateOp.GetPrecision()) {
        case UsdGeomXformOp::PrecisionFloat:
            return translateOp.Set<GfVec3f>(GfVec3f(translate), UsdTimeCode::Default());
        case UsdGeomXformOp::PrecisionHalf:
            return translateOp.Set<GfVec3h>(GfVec3h(translate), UsdTimeCode::Default());
        case UsdGeomXformOp::PrecisionDouble:
        default:
            return translateOp.Set<GfVec3d>(translate, UsdTimeCode::Default());
    }
}

static bool _DuGetDirectTranslate(
    const UsdGeomXformable& xformable,
    GfVec3d* outTranslate)
{
    const UsdGeomXformOp translateOp = _DuFindEditableTranslateOp(xformable);
    if (!translateOp) {
        return false;
    }
    return translateOp.GetAs<GfVec3d>(outTranslate, UsdTimeCode::Default());
}

static bool _DuSetDirectTranslate(
    const UsdGeomXformable& xformable,
    const GfVec3d& translate)
{
    const UsdGeomXformOp translateOp = _DuFindEditableTranslateOp(xformable);
    if (!translateOp) {
        return false;
    }
    return _DuSetTranslateOnOp(translateOp, translate);
}

static bool _DuCreateAndSetTranslate(
    const UsdGeomXformable& xformable,
    const GfVec3d& translate)
{
    UsdGeomXformOp::Precision precision = UsdGeomXformOp::PrecisionDouble;
    bool resetsXformStack = false;
    const auto ops = xformable.GetOrderedXformOps(&resetsXformStack);
    for (const UsdGeomXformOp& op : ops) {
        if (!op || op.IsInverseOp()) {
            continue;
        }
        if (op.GetOpType() == UsdGeomXformOp::TypeTranslate) {
            precision = op.GetPrecision();
            break;
        }
    }

    const UsdGeomXformOp createdOp = xformable.AddTranslateOp(precision);
    if (!createdOp) {
        return false;
    }
    return _DuSetTranslateOnOp(createdOp, translate);
}

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

DuStatus du_xform_get_world(DuPrim* prim, double matrix[16]) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(matrix);

    UsdGeomXformable xformable(du_prim_get_usd(prim));
    if (!xformable) {
        du_set_last_error("Prim is not Xformable");
        return DU_ERR_INVALID;
    }

    GfMatrix4d worldXform =
        xformable.ComputeLocalToWorldTransform(UsdTimeCode::Default());
    const double* data = worldXform.GetArray();
    memcpy(matrix, data, 16 * sizeof(double));
    return DU_OK;
}

static DuStatus du_xform_get_trs(
    DuPrim* prim,
    double xyz[3],
    int component)
{
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(xyz);

    GfVec3d translation(0.0);
    GfVec3f rotation(0.0f);
    GfVec3f scale(1.0f);
    GfVec3f pivot(0.0f);
    UsdGeomXformCommonAPI::RotationOrder rotationOrder;
    UsdGeomXformCommonAPI api(du_prim_get_usd(prim));

    if (!api.GetXformVectors(
            &translation,
            &rotation,
            &scale,
            &pivot,
            &rotationOrder,
            UsdTimeCode::Default())
        && !api.GetXformVectorsByAccumulation(
            &translation,
            &rotation,
            &scale,
            &pivot,
            &rotationOrder,
            UsdTimeCode::Default())) {
        du_set_last_error("Failed to read transform vectors");
        return DU_ERR_USD;
    }

    if (component == 0) {
        xyz[0] = translation[0];
        xyz[1] = translation[1];
        xyz[2] = translation[2];
    } else if (component == 1) {
        xyz[0] = rotation[0];
        xyz[1] = rotation[1];
        xyz[2] = rotation[2];
    } else {
        xyz[0] = scale[0];
        xyz[1] = scale[1];
        xyz[2] = scale[2];
    }

    return DU_OK;
}

DuStatus du_xform_get_translate(DuPrim* prim, double xyz[3]) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(xyz);

    UsdGeomXformable xformable(du_prim_get_usd(prim));
    if (!xformable) {
        du_set_last_error("Prim is not Xformable");
        return DU_ERR_INVALID;
    }

    GfVec3d translate(0.0);
    if (_DuGetDirectTranslate(xformable, &translate)) {
        xyz[0] = translate[0];
        xyz[1] = translate[1];
        xyz[2] = translate[2];
        return DU_OK;
    }

    return du_xform_get_trs(prim, xyz, 0);
}

DuStatus du_xform_get_rotate(DuPrim* prim, double xyz[3]) {
    return du_xform_get_trs(prim, xyz, 1);
}

DuStatus du_xform_get_rotate_order(DuPrim* prim, int32_t* order) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(order);

    GfVec3d translation(0.0);
    GfVec3f rotation(0.0f);
    GfVec3f scale(1.0f);
    GfVec3f pivot(0.0f);
    UsdGeomXformCommonAPI::RotationOrder rotationOrder;
    UsdGeomXformCommonAPI api(du_prim_get_usd(prim));

    if (!api.GetXformVectors(
            &translation,
            &rotation,
            &scale,
            &pivot,
            &rotationOrder,
            UsdTimeCode::Default())
        && !api.GetXformVectorsByAccumulation(
            &translation,
            &rotation,
            &scale,
            &pivot,
            &rotationOrder,
            UsdTimeCode::Default())) {
        du_set_last_error("Failed to read transform rotation order");
        return DU_ERR_USD;
    }

    *order = static_cast<int32_t>(rotationOrder);
    return DU_OK;
}

DuStatus du_xform_get_scale(DuPrim* prim, double xyz[3]) {
    return du_xform_get_trs(prim, xyz, 2);
}

DuStatus du_xform_get_pivot(DuPrim* prim, double xyz[3]) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(xyz);

    GfVec3d translation(0.0);
    GfVec3f rotation(0.0f);
    GfVec3f scale(1.0f);
    GfVec3f pivot(0.0f);
    UsdGeomXformCommonAPI::RotationOrder rotationOrder;
    UsdGeomXformCommonAPI api(du_prim_get_usd(prim));

    if (!api.GetXformVectors(
            &translation,
            &rotation,
            &scale,
            &pivot,
            &rotationOrder,
            UsdTimeCode::Default())
        && !api.GetXformVectorsByAccumulation(
            &translation,
            &rotation,
            &scale,
            &pivot,
            &rotationOrder,
            UsdTimeCode::Default())) {
        du_set_last_error("Failed to read transform pivot");
        return DU_ERR_USD;
    }

    xyz[0] = pivot[0];
    xyz[1] = pivot[1];
    xyz[2] = pivot[2];
    return DU_OK;
}

DuStatus du_xform_get_world_pivot(DuPrim* prim, double xyz[3]) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(xyz);

    double pivot[3];
    DuStatus status = du_xform_get_pivot(prim, pivot);
    if (status != DU_OK) {
        return status;
    }

    UsdGeomXformable xformable(du_prim_get_usd(prim));
    if (!xformable) {
        du_set_last_error("Prim is not Xformable");
        return DU_ERR_INVALID;
    }

    const GfMatrix4d worldXform =
        xformable.ComputeLocalToWorldTransform(UsdTimeCode::Default());
    const GfVec3d worldPivot =
        worldXform.Transform(GfVec3d(pivot[0], pivot[1], pivot[2]));
    xyz[0] = worldPivot[0];
    xyz[1] = worldPivot[1];
    xyz[2] = worldPivot[2];
    return DU_OK;
}

DuStatus du_xform_set_translate(DuPrim* prim, double x, double y, double z) {
    DU_CHECK_NULL(prim);

    UsdGeomXformable xformable(du_prim_get_usd(prim));
    if (!xformable) {
        du_set_last_error("Prim is not Xformable");
        return DU_ERR_INVALID;
    }

    if (_DuSetDirectTranslate(xformable, GfVec3d(x, y, z))) {
        return DU_OK;
    }

    if (_DuCreateAndSetTranslate(xformable, GfVec3d(x, y, z))) {
        return DU_OK;
    }

    du_set_last_error("Failed to author translate op");
    return DU_ERR_USD;
}

DuStatus du_xform_set_translate_world(DuPrim* prim, double x, double y, double z) {
    DU_CHECK_NULL(prim);

    UsdPrim usdPrim = du_prim_get_usd(prim);
    UsdGeomXformable xformable(usdPrim);
    if (!xformable) {
        du_set_last_error("Prim is not Xformable");
        return DU_ERR_INVALID;
    }

    GfVec3d localPos(x, y, z);
    const UsdPrim parentPrim = usdPrim.GetParent();
    if (parentPrim && parentPrim.GetPath() != SdfPath::AbsoluteRootPath()) {
        UsdGeomXformable parentXformable(parentPrim);
        if (parentXformable) {
            const GfMatrix4d parentWorld =
                parentXformable.ComputeLocalToWorldTransform(UsdTimeCode::Default());
            localPos = parentWorld.GetInverse().Transform(localPos);
        }
    }

    if (_DuSetDirectTranslate(xformable, localPos)) {
        return DU_OK;
    }

    if (_DuCreateAndSetTranslate(xformable, localPos)) {
        return DU_OK;
    }

    du_set_last_error("Failed to author translate op");
    return DU_ERR_USD;
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

DuStatus du_xform_get_translate(DuPrim*, double[3]) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_xform_get_world(DuPrim*, double[16]) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_xform_get_rotate(DuPrim*, double[3]) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_xform_get_rotate_order(DuPrim*, int32_t*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_xform_get_scale(DuPrim*, double[3]) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_xform_get_pivot(DuPrim*, double[3]) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_xform_get_world_pivot(DuPrim*, double[3]) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_xform_set_translate(DuPrim*, double, double, double) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_xform_set_translate_world(DuPrim*, double, double, double) {
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
