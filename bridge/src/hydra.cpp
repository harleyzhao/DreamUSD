// bridge/src/hydra.cpp
// Hydra rendering engine and render delegate operations for DreamUSD bridge.
// When compiled with OpenUSD (HAS_USD defined via CMake), uses real USD API.
// Otherwise provides stub implementations.

#include "dreamusd_bridge.h"
#include "error_internal.h"

#include <cstring>
#include <string>
#include <vector>

#ifdef HAS_USD

#include <pxr/usd/usd/stage.h>
#include <pxr/imaging/hgi/hgi.h>
#include <pxr/imaging/hd/engine.h>
#include <pxr/imaging/hd/renderIndex.h>
#include <pxr/imaging/hd/rendererPlugin.h>
#include <pxr/imaging/hd/rendererPluginRegistry.h>
#include <pxr/imaging/hd/pluginRenderDelegateUniqueHandle.h>
#include <pxr/imaging/hdx/taskController.h>
#include <pxr/imaging/hd/aov.h>
#include <pxr/imaging/hd/renderBuffer.h>
#include <pxr/imaging/hd/tokens.h>
#include <pxr/usdImaging/usdImaging/delegate.h>
#include <pxr/base/tf/token.h>
#include <pxr/base/gf/camera.h>
#include <pxr/base/gf/vec3d.h>
#include <pxr/base/gf/matrix4d.h>
#include <pxr/base/gf/frustum.h>
#include <pxr/imaging/hf/pluginDesc.h>
#include <pxr/imaging/glf/simpleLight.h>
#include <pxr/imaging/glf/simpleLightingContext.h>
#include <pxr/imaging/glf/simpleMaterial.h>
#include <pxr/imaging/hdx/renderSetupTask.h>
#include <pxr/imaging/hdx/selectionTracker.h>
#include <pxr/imaging/hdx/tokens.h>
#include <pxr/usd/usdLux/lightAPI.h>
#include <pxr/usd/usdLux/domeLight_1.h>
#include <pxr/usd/usdLux/distantLight.h>
#include <pxr/usd/usdLux/shadowAPI.h>
#include <pxr/usd/usdGeom/xformable.h>
#include <pxr/usd/usdGeom/xform.h>
#include <pxr/usd/usdGeom/xformCommonAPI.h>
#include <pxr/imaging/hd/light.h>
#include <pxr/imaging/hd/repr.h>
#include <pxr/imaging/hdx/simpleLightTask.h>
#include <pxr/imaging/hdx/shadowMatrixComputation.h>
#include <pxr/imaging/glf/simpleShadowArray.h>
#include <pxr/base/gf/rotation.h>

// For HgiTokens
#include <pxr/imaging/hgi/tokens.h>

PXR_NAMESPACE_USING_DIRECTIVE

// Forward declaration — DuStage defined in stage.cpp
struct DuStage;
extern UsdStageRefPtr du_stage_get_ptr(DuStage* stage);

// Shadow matrix computation for distant lights (orthographic projection)
class DistantLightShadowMatrix : public HdxShadowMatrixComputation {
public:
    DistantLightShadowMatrix(const GfVec3d& direction) {
        GfFrustum frustum;
        frustum.SetProjectionType(GfFrustum::Orthographic);
        frustum.SetWindow(GfRange2d(GfVec2d(-100, -100), GfVec2d(100, 100)));
        frustum.SetNearFar(GfRange1d(0, 1000));
        GfVec3d dir = direction.GetNormalized();
        frustum.SetPosition(-dir * 500.0);
        frustum.SetRotation(GfRotation(GfVec3d(0, 0, -1), dir));
        _shadowMatrix =
            frustum.ComputeViewMatrix() * frustum.ComputeProjectionMatrix();
    }

    std::vector<GfMatrix4d> Compute(
            const GfVec4f& viewport,
            CameraUtilConformWindowPolicy policy) override {
        return { _shadowMatrix };
    }

    std::vector<GfMatrix4d> Compute(
            const CameraUtilFraming& framing,
            CameraUtilConformWindowPolicy policy) override {
        return { _shadowMatrix };
    }

private:
    GfMatrix4d _shadowMatrix;
};

// Simple scene delegate for managing default lights with shadow params.
// This is needed because the Scene Index pipeline in USD 0.26+ bypasses
// virtual method overrides on UsdImagingDelegate for light params.
class DefaultLightDelegate : public HdSceneDelegate {
public:
    DefaultLightDelegate(HdRenderIndex* renderIndex, SdfPath const& delegateId)
        : HdSceneDelegate(renderIndex, delegateId) {}

    void AddLight(SdfPath const& id, GlfSimpleLight const& light,
                  HdxShadowParams const& shadowParams) {
        GetRenderIndex().InsertSprim(
            HdPrimTypeTokens->simpleLight, this, id);
        _cache[id][HdLightTokens->params] = light;
        _cache[id][HdLightTokens->shadowParams] = shadowParams;
        _cache[id][HdLightTokens->shadowCollection] =
            HdRprimCollection(HdTokens->geometry,
                HdReprSelector(HdReprTokens->refined));
        _cache[id][HdTokens->transform] = GfMatrix4d(1.0);
        GetRenderIndex().GetChangeTracker().MarkSprimDirty(
            id, HdLight::AllDirty);
    }

    void UpdateLight(SdfPath const& id, GlfSimpleLight const& light,
                     HdxShadowParams const& shadowParams) {
        _cache[id][HdLightTokens->params] = light;
        _cache[id][HdLightTokens->shadowParams] = shadowParams;
        GetRenderIndex().GetChangeTracker().MarkSprimDirty(
            id, HdLight::DirtyParams | HdLight::DirtyShadowParams);
    }

    void RemoveAllLights() {
        for (auto& entry : _cache) {
            GetRenderIndex().RemoveSprim(
                HdPrimTypeTokens->simpleLight, entry.first);
        }
        _cache.clear();
    }

    VtValue Get(SdfPath const& id, TfToken const& key) override {
        auto it = _cache.find(id);
        if (it != _cache.end()) {
            auto valIt = it->second.find(key);
            if (valIt != it->second.end()) return valIt->second;
        }
        return VtValue();
    }

    VtValue GetLightParamValue(SdfPath const& id,
                               TfToken const& paramName) override {
        return Get(id, paramName);
    }

private:
    std::map<SdfPath, std::map<TfToken, VtValue>> _cache;
};

struct DuHydraEngine {
    // CPU framebuffer for readback
    std::vector<uint8_t> framebuffer;
    UsdStageRefPtr stage;

    // Hgi and Hydra pipeline
    HgiUniquePtr hgi;
    HdDriver hgiDriver;
    HdRenderIndex* renderIndex = nullptr;
    HdPluginRenderDelegateUniqueHandle renderDelegateHandle;
    UsdImagingDelegate* sceneDelegate = nullptr;
    HdxTaskController* taskController = nullptr;
    HdxSelectionTrackerSharedPtr selTracker;
    DefaultLightDelegate* lightDelegate = nullptr;
    HdEngine engine;

    // GPU image output (opaque handles)
    uint64_t outputImage = 0;
    uint64_t outputImageView = 0;
    uint64_t renderSemaphore = 0;

    // Camera state
    GfVec3d eye{0, 0, 10};
    GfVec3d target{0, 0, 0};
    GfVec3d up{0, 1, 0};

    // Current render delegate
    TfToken currentRdId;

    // Current lighting mode
    bool enableLighting = true;
    bool enableShadows = false;
    bool hasSceneLights = false;

    // Output dimensions
    uint32_t width = 0;
    uint32_t height = 0;
};

extern "C" {

DuStatus du_hydra_create(DuStage* stage, DuHydraEngine** out) {
    // Delegate to the Vulkan version with NULL Vulkan handles
    // The implementation uses platform-default Hgi anyway
    return du_hydra_create_with_vulkan(stage, nullptr, nullptr, nullptr, 0, out);
}

DuStatus du_hydra_create_with_vulkan(
    DuStage* stage,
    void* /*vk_instance*/,
    void* /*vk_physical_device*/,
    void* /*vk_device*/,
    uint32_t /*queue_family_index*/,
    DuHydraEngine** out)
{
    DU_CHECK_NULL(stage);
    DU_CHECK_NULL(out);

    auto stagePtr = du_stage_get_ptr(stage);
    if (!stagePtr) {
        du_set_last_error("Stage pointer is invalid");
        return DU_ERR_INVALID;
    }

    DU_TRY({
        auto* eng = new DuHydraEngine();
        eng->stage = stagePtr;

        // Create platform-default Hgi (Metal on macOS, GL on Linux)
        eng->hgi = Hgi::CreatePlatformDefaultHgi();
        if (!eng->hgi) {
            delete eng;
            du_set_last_error("Failed to create Hgi");
            return DU_ERR_USD;
        }

        TfToken stormId("HdStormRendererPlugin");
        eng->currentRdId = stormId;

        auto& registry = HdRendererPluginRegistry::GetInstance();
        eng->renderDelegateHandle = registry.CreateRenderDelegate(stormId);
        if (!eng->renderDelegateHandle) {
            delete eng;
            du_set_last_error("Failed to create Storm render delegate");
            return DU_ERR_USD;
        }

        eng->hgiDriver.name = HgiTokens->renderDriver;
        eng->hgiDriver.driver = VtValue(eng->hgi.get());
        HdDriverVector drivers = {&eng->hgiDriver};

        eng->renderIndex = HdRenderIndex::New(
            eng->renderDelegateHandle.Get(), drivers);
        if (!eng->renderIndex) {
            delete eng;
            du_set_last_error("Failed to create HdRenderIndex");
            return DU_ERR_USD;
        }

        // Check if scene has its own lights
        eng->hasSceneLights = false;
        for (auto prim : stagePtr->Traverse()) {
            if (prim.HasAPI<UsdLuxLightAPI>()) {
                eng->hasSceneLights = true;
                break;
            }
        }

        // Add default light Xform prims for hierarchy visibility
        // (actual rendering lights are handled by DefaultLightDelegate)
        if (!eng->hasSceneLights) {
            // Create Xform prims so they show in the hierarchy panel
            // Use XformCommonAPI-compatible ops: translate, rotate, scale
            UsdGeomXform::Define(stagePtr, SdfPath("/_DefaultLights"));

            {
                auto keyXf = UsdGeomXform::Define(
                    stagePtr, SdfPath("/_DefaultLights/KeyLight"));
                UsdGeomXformCommonAPI api(keyXf.GetPrim());
                api.SetTranslate(GfVec3d(5, 8, 4));
            }

            {
                auto fillXf = UsdGeomXform::Define(
                    stagePtr, SdfPath("/_DefaultLights/FillLight"));
                UsdGeomXformCommonAPI api(fillXf.GetPrim());
                api.SetTranslate(GfVec3d(-6, 4, -3));
            }

            {
                UsdGeomXform::Define(
                    stagePtr, SdfPath("/_DefaultLights/AmbientLight"));
            }
        }

        // Populate scene delegate
        SdfPath delegateId = SdfPath::AbsoluteRootPath();
        eng->sceneDelegate = new UsdImagingDelegate(
            eng->renderIndex, delegateId);
        eng->sceneDelegate->Populate(stagePtr->GetPseudoRoot());

        // Set up rendering lights via DefaultLightDelegate (with shadow params)
        if (!eng->hasSceneLights) {
            eng->lightDelegate = new DefaultLightDelegate(
                eng->renderIndex, SdfPath("/_DuLights"));

            // Key light — direction from initial position (5,8,4) toward origin
            {
                GlfSimpleLight key;
                GfVec3d pos(5, 8, 4);
                GfVec3d dir = (-pos).GetNormalized();
                key.SetPosition(GfVec4f(dir[0], dir[1], dir[2], 0.0f));
                float intensity = 4.0f;
                GfVec4f color(1.0f * intensity, 0.95f * intensity,
                              0.9f * intensity, 1.0f);
                key.SetDiffuse(color);
                key.SetSpecular(color);
                key.SetHasShadow(true);

                HdxShadowParams shadowParams;
                shadowParams.enabled = true;
                shadowParams.resolution = 2048;
                shadowParams.bias = -0.001;
                shadowParams.blur = 0.1;
                shadowParams.shadowMatrix =
                    HdxShadowMatrixComputationSharedPtr(
                        new DistantLightShadowMatrix(dir));

                eng->lightDelegate->AddLight(
                    SdfPath("/_DuLights/KeyLight"), key, shadowParams);
            }

            // Fill light — direction from initial position (-6,4,-3) toward origin
            {
                GlfSimpleLight fill;
                GfVec3d pos(-6, 4, -3);
                GfVec3d dir = (-pos).GetNormalized();
                fill.SetPosition(GfVec4f(dir[0], dir[1], dir[2], 0.0f));
                float intensity = 0.8f;
                GfVec4f color(0.7f * intensity, 0.8f * intensity,
                              1.0f * intensity, 1.0f);
                fill.SetDiffuse(color);
                fill.SetSpecular(GfVec4f(0, 0, 0, 0));
                fill.SetHasShadow(false);

                HdxShadowParams noShadow;
                eng->lightDelegate->AddLight(
                    SdfPath("/_DuLights/FillLight"), fill, noShadow);
            }

            // Ambient
            {
                GlfSimpleLight ambient;
                ambient.SetPosition(GfVec4f(0, 1, 0, 0));
                ambient.SetAmbient(GfVec4f(0.12f, 0.13f, 0.15f, 1.0f));
                ambient.SetDiffuse(GfVec4f(0, 0, 0, 0));
                ambient.SetSpecular(GfVec4f(0, 0, 0, 0));
                ambient.SetHasShadow(false);

                HdxShadowParams noShadow;
                eng->lightDelegate->AddLight(
                    SdfPath("/_DuLights/AmbientLight"), ambient, noShadow);
            }
        }

        SdfPath taskControllerId("/taskController");
        eng->taskController = new HdxTaskController(
            eng->renderIndex, taskControllerId);

        // Configure AOV outputs: color for readback, depth for shadow maps
        eng->taskController->SetRenderOutputs(
            {HdAovTokens->color, HdAovTokens->depth});

        // Enable MSAA on color and depth AOVs
        {
            HdAovDescriptor colorDesc =
                eng->taskController->GetRenderOutputSettings(HdAovTokens->color);
            colorDesc.multiSampled = true;
            eng->taskController->SetRenderOutputSettings(
                HdAovTokens->color, colorDesc);

            HdAovDescriptor depthDesc =
                eng->taskController->GetRenderOutputSettings(HdAovTokens->depth);
            depthDesc.multiSampled = true;
            eng->taskController->SetRenderOutputSettings(
                HdAovTokens->depth, depthDesc);
        }

        // Disable presentation — we read back via AOV, no GL/Metal surface needed
        eng->taskController->SetEnablePresentation(false);

        // Set up selection tracker to avoid selectionState errors
        eng->selTracker = std::make_shared<HdxSelectionTracker>();

        *out = eng;
        return DU_OK;
    });

    return DU_ERR_USD;
}

DuStatus du_hydra_render(DuHydraEngine* engine, uint32_t width, uint32_t height) {
    DU_CHECK_NULL(engine);

    DU_TRY({
        engine->width = width;
        engine->height = height;

        // Set up camera via view and projection matrices
        GfMatrix4d viewMatrix;
        viewMatrix.SetLookAt(engine->eye, engine->target, engine->up);

        // Build camera-to-world transform for the frustum
        GfMatrix4d camToWorld = viewMatrix.GetInverse();

        double aspectRatio = (height > 0) ? (double)width / (double)height : 1.0;
        GfFrustum frustum;
        frustum.SetPerspective(60.0, aspectRatio, 0.1, 10000.0);
        frustum.SetPositionAndRotationFromMatrix(camToWorld);

        engine->taskController->SetFreeCameraMatrices(
            viewMatrix, frustum.ComputeProjectionMatrix());
        engine->taskController->SetRenderViewport(
            GfVec4d(0, 0, width, height));
        engine->taskController->SetRenderBufferSize(GfVec2i(width, height));

        // Enable lighting in render params
        {
            HdxRenderTaskParams params;
            params.enableLighting = engine->enableLighting;
            params.enableSceneLights = true;
            engine->taskController->SetRenderParams(params);
        }

        // Set up lighting context for Storm
        {
            auto lightingCtx = GlfSimpleLightingContext::New();
            lightingCtx->SetUseLighting(engine->enableLighting);
            lightingCtx->SetCamera(viewMatrix, frustum.ComputeProjectionMatrix());
            engine->taskController->SetLightingState(lightingCtx);
        }

        // Enable/disable shadow task
        engine->taskController->SetEnableShadows(engine->enableShadows);

        // Provide selectionState to task context
        engine->engine.SetTaskContextData(
            HdxTokens->selectionState,
            VtValue(engine->selTracker));

        // Sync USD stage changes (attribute edits) into Hydra
        engine->sceneDelegate->ApplyPendingUpdates();

        // Sync default light transforms from Xform prims to rendering lights
        if (engine->lightDelegate && engine->stage) {
            auto syncLight = [&](const char* xformPath, const char* lightPath,
                                 bool hasShadow) {
                UsdPrim prim = engine->stage->GetPrimAtPath(SdfPath(xformPath));
                if (!prim) {
                    fprintf(stderr, "[sync] prim not found: %s\n", xformPath);
                    return;
                }

                UsdGeomXformable xf(prim);
                GfMatrix4d xform;
                bool resetStack = false;
                xf.GetLocalTransformation(&xform, &resetStack,
                                           UsdTimeCode::Default());

                // Get translation from the matrix
                GfVec3d pos(xform[3][0], xform[3][1], xform[3][2]);

                GfVec3d dir;
                if (pos.GetLength() > 0.01) {
                    // Position-based: light points from position toward origin
                    dir = (-pos).GetNormalized();
                } else {
                    // Rotation-based fallback: use -Z axis of rotation matrix
                    dir = GfVec3d(-xform[2][0], -xform[2][1], -xform[2][2]);
                    dir.Normalize();
                }

                GlfSimpleLight light;
                light.SetPosition(GfVec4f(dir[0], dir[1], dir[2], 0.0f));
                light.SetHasShadow(hasShadow);

                // Preserve the original light color/intensity
                // (read from current delegate cache via Get)
                VtValue oldParams = engine->lightDelegate->Get(
                    SdfPath(lightPath), HdLightTokens->params);
                if (oldParams.IsHolding<GlfSimpleLight>()) {
                    GlfSimpleLight old = oldParams.UncheckedGet<GlfSimpleLight>();
                    light.SetDiffuse(old.GetDiffuse());
                    light.SetSpecular(old.GetSpecular());
                    light.SetAmbient(old.GetAmbient());
                }

                HdxShadowParams shadowParams;
                if (hasShadow) {
                    shadowParams.enabled = true;
                    shadowParams.resolution = 2048;
                    shadowParams.bias = -0.001;
                    shadowParams.blur = 0.1;
                    shadowParams.shadowMatrix =
                        HdxShadowMatrixComputationSharedPtr(
                            new DistantLightShadowMatrix(dir));
                }

                engine->lightDelegate->UpdateLight(
                    SdfPath(lightPath), light, shadowParams);
            };

            syncLight("/_DefaultLights/KeyLight", "/_DuLights/KeyLight", true);
            syncLight("/_DefaultLights/FillLight", "/_DuLights/FillLight", false);
        }

        HdTaskSharedPtrVector tasks = engine->taskController->GetRenderingTasks();
        engine->engine.Execute(engine->renderIndex, &tasks);

        return DU_OK;
    });

    return DU_ERR_USD;
}

DuStatus du_hydra_get_framebuffer(DuHydraEngine* engine, uint8_t** rgba, uint32_t* width, uint32_t* height) {
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(rgba);
    DU_CHECK_NULL(width);
    DU_CHECK_NULL(height);

    uint32_t w = engine->width;
    uint32_t h = engine->height;
    if (w == 0 || h == 0) {
        du_set_last_error("Render not yet called");
        return DU_ERR_INVALID;
    }

    // Read back from HdxTaskController color AOV
    HdRenderBuffer* colorBuffer = engine->taskController->GetRenderOutput(HdAovTokens->color);
    if (!colorBuffer) {
        du_set_last_error("Color AOV not available");
        return DU_ERR_USD;
    }

    colorBuffer->Resolve();
    void* data = colorBuffer->Map();
    if (!data) {
        du_set_last_error("Failed to map color render buffer");
        return DU_ERR_USD;
    }

    uint32_t bufW = colorBuffer->GetWidth();
    uint32_t bufH = colorBuffer->GetHeight();
    HdFormat format = colorBuffer->GetFormat();

    size_t outSize = (size_t)bufW * bufH * 4;
    engine->framebuffer.resize(outSize);

    if (format == HdFormatUNorm8Vec4) {
        // RGBA8 — direct copy
        memcpy(engine->framebuffer.data(), data, outSize);
    } else if (format == HdFormatUNorm8Vec3) {
        // RGB8 — expand to RGBA
        const uint8_t* src = (const uint8_t*)data;
        for (size_t i = 0; i < (size_t)bufW * bufH; i++) {
            engine->framebuffer[i * 4 + 0] = src[i * 3 + 0];
            engine->framebuffer[i * 4 + 1] = src[i * 3 + 1];
            engine->framebuffer[i * 4 + 2] = src[i * 3 + 2];
            engine->framebuffer[i * 4 + 3] = 255;
        }
    } else if (format == HdFormatFloat16Vec4) {
        // Float16 RGBA — convert to uint8
        const uint16_t* src = (const uint16_t*)data;
        for (size_t i = 0; i < (size_t)bufW * bufH * 4; i++) {
            // Simple half-float to uint8 conversion
            uint16_t half = src[i];
            // Extract components
            uint32_t sign = (half >> 15) & 1;
            uint32_t exp = (half >> 10) & 0x1F;
            uint32_t mant = half & 0x3FF;
            float f;
            if (exp == 0) {
                f = (mant / 1024.0f) * (1.0f / 16384.0f);
            } else if (exp == 31) {
                f = 1.0f;
            } else {
                f = ldexpf((1.0f + mant / 1024.0f), (int)exp - 15);
            }
            if (sign) f = -f;
            int val = (int)(f * 255.0f + 0.5f);
            engine->framebuffer[i] = (uint8_t)(val < 0 ? 0 : (val > 255 ? 255 : val));
        }
    } else if (format == HdFormatFloat32Vec4) {
        // Float32 RGBA — convert to uint8
        const float* src = (const float*)data;
        for (size_t i = 0; i < (size_t)bufW * bufH * 4; i++) {
            int val = (int)(src[i] * 255.0f + 0.5f);
            engine->framebuffer[i] = (uint8_t)(val < 0 ? 0 : (val > 255 ? 255 : val));
        }
    } else {
        // Unknown format — fill with magenta for debugging
        for (size_t i = 0; i < (size_t)bufW * bufH; i++) {
            engine->framebuffer[i * 4 + 0] = 255;
            engine->framebuffer[i * 4 + 1] = 0;
            engine->framebuffer[i * 4 + 2] = 255;
            engine->framebuffer[i * 4 + 3] = 255;
        }
    }

    colorBuffer->Unmap();

    *rgba = engine->framebuffer.data();
    *width = bufW;
    *height = bufH;
    return DU_OK;
}

DuStatus du_hydra_get_vk_image(
    DuHydraEngine* engine,
    void* image,
    void* view,
    uint32_t* format,
    uint32_t* width,
    uint32_t* height)
{
    DU_CHECK_NULL(engine);

    // Return current output image info (opaque handles)
    if (image) *(uint64_t*)image = engine->outputImage;
    if (view) *(uint64_t*)view = engine->outputImageView;
    if (format) *format = 37; // VK_FORMAT_R8G8B8A8_UNORM
    if (width) *width = engine->width;
    if (height) *height = engine->height;

    return DU_OK;
}

DuStatus du_hydra_get_render_semaphore(DuHydraEngine* engine, void* semaphore) {
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(semaphore);

    *(uint64_t*)semaphore = engine->renderSemaphore;
    return DU_OK;
}

DuStatus du_hydra_set_camera(DuHydraEngine* engine, double eye[3], double target[3], double up[3]) {
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(eye);
    DU_CHECK_NULL(target);
    DU_CHECK_NULL(up);

    engine->eye = GfVec3d(eye[0], eye[1], eye[2]);
    engine->target = GfVec3d(target[0], target[1], target[2]);
    engine->up = GfVec3d(up[0], up[1], up[2]);

    return DU_OK;
}

DuStatus du_hydra_set_display_mode(DuHydraEngine* engine, DuDisplayMode mode) {
    DU_CHECK_NULL(engine);

    TfToken reprName;
    bool lighting = true;

    switch (mode) {
        case DU_DISPLAY_SMOOTH_SHADED:
            reprName = TfToken("smoothHull");
            lighting = true;
            break;
        case DU_DISPLAY_WIREFRAME:
            reprName = TfToken("wire");
            lighting = false;
            break;
        case DU_DISPLAY_WIREFRAME_ON_SHADED:
            reprName = TfToken("wireOnSurf");
            lighting = true;
            break;
        case DU_DISPLAY_FLAT_SHADED:
            reprName = TfToken("hull");
            lighting = true;
            break;
        case DU_DISPLAY_POINTS:
            reprName = TfToken("points");
            lighting = false;
            break;
        case DU_DISPLAY_TEXTURED:
            reprName = TfToken("smoothHull");
            lighting = true;
            break;
        default:
            reprName = TfToken("smoothHull");
            lighting = true;
            break;
    }

    engine->taskController->SetCollection(
        HdRprimCollection(HdTokens->geometry, HdReprSelector(reprName)));

    engine->enableLighting = lighting;

    return DU_OK;
}

DuStatus du_hydra_set_enable_lighting(DuHydraEngine* engine, bool enable) {
    DU_CHECK_NULL(engine);
    engine->enableLighting = enable;
    return DU_OK;
}

DuStatus du_hydra_set_enable_shadows(DuHydraEngine* engine, bool enable) {
    DU_CHECK_NULL(engine);
    engine->enableShadows = enable;
    engine->taskController->SetEnableShadows(enable);
    return DU_OK;
}

DuStatus du_hydra_set_msaa(DuHydraEngine* engine, bool enable) {
    DU_CHECK_NULL(engine);

    HdAovDescriptor colorDesc =
        engine->taskController->GetRenderOutputSettings(HdAovTokens->color);
    colorDesc.multiSampled = enable;
    engine->taskController->SetRenderOutputSettings(HdAovTokens->color, colorDesc);

    HdAovDescriptor depthDesc =
        engine->taskController->GetRenderOutputSettings(HdAovTokens->depth);
    depthDesc.multiSampled = enable;
    engine->taskController->SetRenderOutputSettings(HdAovTokens->depth, depthDesc);

    return DU_OK;
}

DuStatus du_hydra_project_point(
    DuHydraEngine* engine,
    double world_xyz[3],
    uint32_t viewport_w, uint32_t viewport_h,
    double screen_xy[2])
{
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(world_xyz);
    DU_CHECK_NULL(screen_xy);

    // Build the exact same view and projection matrices as du_hydra_render
    GfMatrix4d viewMatrix;
    viewMatrix.SetLookAt(engine->eye, engine->target, engine->up);

    GfMatrix4d camToWorld = viewMatrix.GetInverse();
    double aspectRatio = (viewport_h > 0) ? (double)viewport_w / (double)viewport_h : 1.0;
    GfFrustum frustum;
    frustum.SetPerspective(60.0, aspectRatio, 0.1, 10000.0);
    frustum.SetPositionAndRotationFromMatrix(camToWorld);

    GfMatrix4d projMatrix = frustum.ComputeProjectionMatrix();

    // USD uses row-vector convention: clip = point * view * proj
    GfVec4d point(world_xyz[0], world_xyz[1], world_xyz[2], 1.0);

    // Multiply: point * viewMatrix
    GfVec4d eye_space;
    for (int j = 0; j < 4; j++) {
        eye_space[j] = point[0] * viewMatrix[0][j]
                     + point[1] * viewMatrix[1][j]
                     + point[2] * viewMatrix[2][j]
                     + point[3] * viewMatrix[3][j];
    }

    // Multiply: eye_space * projMatrix
    GfVec4d clip;
    for (int j = 0; j < 4; j++) {
        clip[j] = eye_space[0] * projMatrix[0][j]
                + eye_space[1] * projMatrix[1][j]
                + eye_space[2] * projMatrix[2][j]
                + eye_space[3] * projMatrix[3][j];
    }

    if (clip[3] <= 0.0) {
        return DU_ERR_INVALID; // Behind camera
    }

    double ndc_x = clip[0] / clip[3];
    double ndc_y = clip[1] / clip[3];

    // NDC [-1,1] to screen pixels
    // The framebuffer is displayed with flipped UV (to correct OpenGL bottom-to-top),
    // so the standard NDC-to-screen mapping applies: NDC +Y = screen top.
    screen_xy[0] = (ndc_x * 0.5 + 0.5) * viewport_w;
    screen_xy[1] = (1.0 - (ndc_y * 0.5 + 0.5)) * viewport_h;

    return DU_OK;
}

void du_hydra_destroy(DuHydraEngine* engine) {
    if (!engine) return;

    if (engine->lightDelegate) {
        engine->lightDelegate->RemoveAllLights();
        delete engine->lightDelegate;
    }
    delete engine->taskController;
    delete engine->sceneDelegate;
    delete engine->renderIndex;
    // renderDelegateHandle cleaned up by destructor

    delete engine;
}

// --- Render Delegates ---

DuStatus du_rd_list_available(const char*** names, uint32_t* count) {
    DU_CHECK_NULL(names);
    DU_CHECK_NULL(count);

    DU_TRY({
        auto& registry = HdRendererPluginRegistry::GetInstance();
        HfPluginDescVector pluginDescs;
        registry.GetPluginDescs(&pluginDescs);

        *count = (uint32_t)pluginDescs.size();
        if (pluginDescs.empty()) {
            *names = nullptr;
            return DU_OK;
        }

        *names = (const char**)malloc(sizeof(const char*) * pluginDescs.size());
        for (size_t i = 0; i < pluginDescs.size(); i++) {
            const std::string& s = pluginDescs[i].id.GetString();
            char* dup = (char*)malloc(s.size() + 1);
            memcpy(dup, s.c_str(), s.size() + 1);
            (*names)[i] = dup;
        }
        return DU_OK;
    });

    return DU_ERR_USD;
}

DuStatus du_rd_get_current(DuHydraEngine* engine, const char** name) {
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(name);

    static thread_local std::string s_name;
    s_name = engine->currentRdId.GetString();
    *name = s_name.c_str();
    return DU_OK;
}

DuStatus du_rd_set_current(DuHydraEngine* engine, const char* name) {
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(name);

    DU_TRY({
        TfToken newId(name);
        if (newId == engine->currentRdId) {
            return DU_OK; // Already using this delegate
        }

        auto& registry = HdRendererPluginRegistry::GetInstance();
        auto newHandle = registry.CreateRenderDelegate(newId);
        if (!newHandle) {
            du_set_last_error(std::string("Failed to create render delegate: ") + name);
            return DU_ERR_INVALID;
        }

        // Tear down old pipeline
        delete engine->taskController;
        engine->taskController = nullptr;
        delete engine->sceneDelegate;
        engine->sceneDelegate = nullptr;
        delete engine->renderIndex;
        engine->renderIndex = nullptr;

        // Set up new pipeline
        engine->renderDelegateHandle = std::move(newHandle);
        engine->currentRdId = newId;

        HdDriverVector rdDrivers = {&engine->hgiDriver};
        engine->renderIndex = HdRenderIndex::New(
            engine->renderDelegateHandle.Get(), rdDrivers);

        SdfPath delegateId = SdfPath::AbsoluteRootPath();
        engine->sceneDelegate = new UsdImagingDelegate(
            engine->renderIndex, delegateId);
        engine->sceneDelegate->Populate(engine->stage->GetPseudoRoot());

        SdfPath taskControllerId("/taskController");
        engine->taskController = new HdxTaskController(
            engine->renderIndex, taskControllerId);

        return DU_OK;
    });

    return DU_ERR_USD;
}

} // extern "C"

#else // !HAS_USD — stub implementations

struct DuHydraEngine {};

extern "C" {

DuStatus du_hydra_create(DuStage*, DuHydraEngine**) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_create_with_vulkan(DuStage*, void*, void*, void*, uint32_t, DuHydraEngine**) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_render(DuHydraEngine*, uint32_t, uint32_t) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_get_framebuffer(DuHydraEngine*, uint8_t**, uint32_t*, uint32_t*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_get_vk_image(DuHydraEngine*, void*, void*, uint32_t*, uint32_t*, uint32_t*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_get_render_semaphore(DuHydraEngine*, void*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_set_camera(DuHydraEngine*, double[3], double[3], double[3]) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_set_display_mode(DuHydraEngine*, DuDisplayMode) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_set_enable_lighting(DuHydraEngine*, bool) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_set_enable_shadows(DuHydraEngine*, bool) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_project_point(DuHydraEngine*, double[3], uint32_t, uint32_t, double[2]) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

void du_hydra_destroy(DuHydraEngine*) {}

// --- Render Delegates ---

DuStatus du_rd_list_available(const char***, uint32_t*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_rd_get_current(DuHydraEngine*, const char**) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_rd_set_current(DuHydraEngine*, const char*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

} // extern "C"

#endif // HAS_USD
