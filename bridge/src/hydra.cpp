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
#include <pxr/usdImaging/usdImaging/delegate.h>
#include <pxr/imaging/glf/drawTarget.h>
#include <pxr/base/tf/token.h>
#include <pxr/base/gf/camera.h>
#include <pxr/base/gf/vec3d.h>
#include <pxr/base/gf/matrix4d.h>
#include <pxr/base/gf/frustum.h>

#include <vulkan/vulkan.h>

// For HgiTokens
#include <pxr/imaging/hgi/tokens.h>

PXR_NAMESPACE_USING_DIRECTIVE

// Forward declaration — DuStage defined in stage.cpp
struct DuStage;
extern UsdStageRefPtr du_stage_get_ptr(DuStage* stage);

struct DuHydraEngine {
    // CPU framebuffer for readback
    std::vector<uint8_t> framebuffer;
    UsdStageRefPtr stage;

    // Hgi and Hydra pipeline
    HgiUniquePtr hgi;
    HdRenderIndex* renderIndex = nullptr;
    HdPluginRenderDelegateUniqueHandle renderDelegateHandle;
    UsdImagingDelegate* sceneDelegate = nullptr;
    HdxTaskController* taskController = nullptr;
    HdEngine engine;

    // Vulkan output
    VkImage outputImage = VK_NULL_HANDLE;
    VkImageView outputImageView = VK_NULL_HANDLE;
    VkSemaphore renderSemaphore = VK_NULL_HANDLE;
    VkDevice vkDevice = VK_NULL_HANDLE;

    // Camera state
    GfVec3d eye{0, 0, 10};
    GfVec3d target{0, 0, 0};
    GfVec3d up{0, 1, 0};

    // Current render delegate
    TfToken currentRdId;

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

        HdDriver hgiDriver;
        hgiDriver.name = HgiTokens->renderDriver;
        hgiDriver.driver = VtValue(eng->hgi.get());
        HdDriverVector drivers = {hgiDriver};

        eng->renderIndex = HdRenderIndex::New(
            eng->renderDelegateHandle.Get(), drivers);
        if (!eng->renderIndex) {
            delete eng;
            du_set_last_error("Failed to create HdRenderIndex");
            return DU_ERR_USD;
        }

        SdfPath delegateId = SdfPath::AbsoluteRootPath();
        eng->sceneDelegate = new UsdImagingDelegate(
            eng->renderIndex, delegateId);
        eng->sceneDelegate->Populate(stagePtr->GetPseudoRoot());

        SdfPath taskControllerId("/taskController");
        eng->taskController = new HdxTaskController(
            eng->renderIndex, taskControllerId);

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

        // Set up camera
        GfCamera cam;
        GfMatrix4d viewMatrix;
        viewMatrix.SetLookAt(engine->eye, engine->target, engine->up);
        cam.SetTransform(viewMatrix.GetInverse());

        GfFrustum frustum;
        frustum.SetPerspective(60.0, (double)width / (double)height, 0.1, 10000.0);
        frustum.SetPosition(engine->eye);
        frustum.SetRotation(
            GfRotation(GfMatrix4d().SetLookAt(
                engine->eye, engine->target, engine->up)));

        engine->taskController->SetFreeCameraMatrices(
            viewMatrix, frustum.ComputeProjectionMatrix());
        engine->taskController->SetRenderViewport(
            GfVec4d(0, 0, width, height));

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

    // TODO: Read back from HdxTaskController AOV output
    // For now, allocate a buffer with a placeholder gradient
    uint32_t w = engine->width;
    uint32_t h = engine->height;
    if (w == 0 || h == 0) {
        du_set_last_error("Render not yet called");
        return DU_ERR_INVALID;
    }

    size_t size = (size_t)w * h * 4;
    engine->framebuffer.resize(size);

    // Placeholder: dark gray with a subtle gradient to show it's working
    for (uint32_t y = 0; y < h; y++) {
        for (uint32_t x = 0; x < w; x++) {
            size_t idx = ((size_t)y * w + x) * 4;
            uint8_t val = (uint8_t)(30 + (x * 20 / w) + (y * 20 / h));
            engine->framebuffer[idx + 0] = val;
            engine->framebuffer[idx + 1] = val;
            engine->framebuffer[idx + 2] = val + 5;
            engine->framebuffer[idx + 3] = 255;
        }
    }

    *rgba = engine->framebuffer.data();
    *width = w;
    *height = h;
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

    // Return current output image info
    if (image) *(VkImage*)image = engine->outputImage;
    if (view) *(VkImageView*)view = engine->outputImageView;
    if (format) *format = 37; // VK_FORMAT_R8G8B8A8_UNORM
    if (width) *width = engine->width;
    if (height) *height = engine->height;

    return DU_OK;
}

DuStatus du_hydra_get_render_semaphore(DuHydraEngine* engine, void* semaphore) {
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(semaphore);

    *(VkSemaphore*)semaphore = engine->renderSemaphore;
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

    DU_TRY({
        TfToken reprName;
        switch (mode) {
            case DU_DISPLAY_SMOOTH_SHADED:
                reprName = TfToken("smoothHull");
                break;
            case DU_DISPLAY_WIREFRAME:
                reprName = TfToken("wire");
                break;
            case DU_DISPLAY_WIREFRAME_ON_SHADED:
                reprName = TfToken("wireOnSurf");
                break;
            case DU_DISPLAY_FLAT_SHADED:
                reprName = TfToken("hull");
                break;
            case DU_DISPLAY_POINTS:
                reprName = TfToken("points");
                break;
            case DU_DISPLAY_TEXTURED:
                reprName = TfToken("smoothHull");
                break;
            default:
                reprName = TfToken("smoothHull");
                break;
        }

        engine->taskController->SetCollection(
            HdRprimCollection(HdTokens->geometry, HdReprSelector(reprName)));

        return DU_OK;
    });

    return DU_ERR_USD;
}

void du_hydra_destroy(DuHydraEngine* engine) {
    if (!engine) return;

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
        TfTokenVector plugins = registry.GetRegisteredRendererPlugins();

        *count = (uint32_t)plugins.size();
        if (plugins.empty()) {
            *names = nullptr;
            return DU_OK;
        }

        *names = (const char**)malloc(sizeof(const char*) * plugins.size());
        for (size_t i = 0; i < plugins.size(); i++) {
            const std::string& s = plugins[i].GetString();
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

        engine->renderIndex = HdRenderIndex::New(
            engine->renderDelegateHandle.Get(), HdDriverVector());

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
