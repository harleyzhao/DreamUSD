// bridge/src/hydra.cpp
// Hydra rendering engine and render delegate operations for DreamUSD bridge.
// When compiled with OpenUSD (HAS_USD defined via CMake), uses real USD API.
// Otherwise provides stub implementations.

#include "dreamusd_bridge.h"
#include "error_internal.h"

#include <algorithm>
#include <array>
#include <cctype>
#include <cstdlib>
#include <iostream>
#include <cstring>
#include <fstream>
#include <limits>
#include <map>
#include <memory>
#include <mutex>
#include <set>
#include <string>
#include <unordered_map>
#include <vector>

#ifdef HAS_USD

#include <pxr/usd/usd/stage.h>
#include <pxr/usd/usd/editContext.h>
#include <pxr/usd/sdf/types.h>
#include <pxr/usd/usdGeom/bboxCache.h>
#include <pxr/usd/usdGeom/imageable.h>
#include <pxr/usd/usdGeom/xformCache.h>
#include <pxr/imaging/hgi/hgi.h>
#include <pxr/imaging/hgi/texture.h>
#include <pxr/imaging/hd/engine.h>
#include <pxr/imaging/hd/renderIndex.h>
#include <pxr/imaging/hd/rendererPlugin.h>
#include <pxr/imaging/hd/rendererPluginRegistry.h>
#include <pxr/imaging/hd/pluginRenderDelegateUniqueHandle.h>
#include <pxr/imaging/hd/filteringSceneIndex.h>
#include <pxr/imaging/hdx/taskController.h>
#include <pxr/imaging/hd/aov.h>
#include <pxr/imaging/hd/renderBuffer.h>
#include <pxr/imaging/hd/selection.h>
#include <pxr/imaging/hd/tokens.h>
#include <pxr/imaging/hd/xformSchema.h>
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
#include <pxr/imaging/hdx/pickTask.h>
#include <pxr/imaging/hdx/renderSetupTask.h>
#include <pxr/imaging/hdx/selectionTracker.h>
#include <pxr/imaging/hdx/shadowMatrixComputation.h>
#include <pxr/imaging/hdx/shadowTask.h>
#define private public
#define protected public
#include <pxr/imaging/hdx/taskControllerSceneIndex.h>
#include <pxr/imaging/hd/retainedDataSource.h>
#include <pxr/imaging/hd/retainedSceneIndex.h>
#include <pxr/imaging/hd/sceneIndexPluginRegistry.h>
#undef protected
#undef private
#include <pxr/imaging/hdx/tokens.h>
#define protected public
#define private public
#include <pxr/usdImaging/usdImagingGL/engine.h>
#undef private
#undef protected
#include <pxr/imaging/hd/lightSchema.h>
#include <pxr/usdImaging/usdImagingGL/renderParams.h>
#include <pxr/usdImaging/usdImagingGL/rendererSettings.h>
#include <pxr/usd/usdLux/lightAPI.h>
#include <pxr/usd/usdLux/distantLight.h>
#include <pxr/usd/usdLux/sphereLight.h>
#include <pxr/usd/usdLux/shadowAPI.h>
#include <pxr/usd/usdGeom/xformable.h>
#include <pxr/imaging/hd/light.h>
#include <pxr/imaging/hd/repr.h>
#include <pxr/imaging/hdx/simpleLightTask.h>
#include <pxr/imaging/glf/simpleShadowArray.h>
// For HgiTokens
#include <pxr/imaging/hgi/tokens.h>

PXR_NAMESPACE_USING_DIRECTIVE

// Forward declaration — DuStage defined in stage.cpp
struct DuStage;
extern UsdStageRefPtr du_stage_get_ptr(DuStage* stage);

static GfRange3d _DuComputeSceneBounds(UsdStageRefPtr const& stage);
static bool _DuGetShadowEnabled(UsdPrim const& prim);

static thread_local UsdStageRefPtr g_duSceneIndexAugmentStage;

struct DuViewerLightState {
    TfToken lightType = HdPrimTypeTokens->simpleLight;
    GfMatrix4d transform{1.0};
    std::unordered_map<TfToken, VtValue, TfToken::HashFunctor> params;
    HdxShadowParams shadowParams;
    HdRprimCollection shadowCollection;
    bool visible = true;
};

struct DuSceneIndexLightOverrideEntry {
    SdfPath primPath;
    TfToken primType;
    HdContainerDataSourceHandle originalDataSource;
};

class DuAugmentedLightSchemaDataSource final : public HdContainerDataSource {
public:
    DuAugmentedLightSchemaDataSource(
        HdContainerDataSourceHandle const& base,
        HdxShadowParams const& shadowParams,
        HdRprimCollection const& shadowCollection)
        : _base(base)
        , _shadowParams(shadowParams)
        , _shadowCollection(shadowCollection) {}

    HdDataSourceBaseHandle Get(TfToken const& name) override {
        if (name == HdLightTokens->shadowParams) {
            return HdRetainedSampledDataSource::New(VtValue(_shadowParams));
        }
        if (name == HdLightTokens->shadowCollection) {
            return HdRetainedSampledDataSource::New(VtValue(_shadowCollection));
        }
        return _base ? _base->Get(name) : nullptr;
    }

    TfTokenVector GetNames() override {
        TfTokenVector names = _base ? _base->GetNames() : TfTokenVector{};
        auto appendIfMissing = [&names](TfToken const& token) {
            if (std::find(names.begin(), names.end(), token) == names.end()) {
                names.push_back(token);
            }
        };
        appendIfMissing(HdLightTokens->shadowParams);
        appendIfMissing(HdLightTokens->shadowCollection);
        return names;
    }

private:
    HdContainerDataSourceHandle _base;
    HdxShadowParams _shadowParams;
    HdRprimCollection _shadowCollection;
};

class DuAugmentedLightPrimDataSource final : public HdContainerDataSource {
public:
    DuAugmentedLightPrimDataSource(
        HdContainerDataSourceHandle const& base,
        HdxShadowParams const& shadowParams,
        HdRprimCollection const& shadowCollection)
        : _base(base)
        , _lightSchema(HdContainerDataSource::Cast(
              base ? base->Get(HdLightSchemaTokens->light) : nullptr))
        , _shadowParams(shadowParams)
        , _shadowCollection(shadowCollection) {}

    HdDataSourceBaseHandle Get(TfToken const& name) override {
        if (name == HdLightSchemaTokens->light) {
            return std::make_shared<DuAugmentedLightSchemaDataSource>(
                _lightSchema, _shadowParams, _shadowCollection);
        }
        return _base ? _base->Get(name) : nullptr;
    }

    TfTokenVector GetNames() override {
        return _base ? _base->GetNames() : TfTokenVector{};
    }

private:
    HdContainerDataSourceHandle _base;
    HdContainerDataSourceHandle _lightSchema;
    HdxShadowParams _shadowParams;
    HdRprimCollection _shadowCollection;
};

static HdRprimCollection _DuMakeShadowCollection() {
    HdRprimCollection collection(
        HdTokens->geometry,
        HdReprSelector(HdReprTokens->smoothHull));
    collection.SetRootPath(SdfPath::AbsoluteRootPath());
    return collection;
}

static bool _DuIsShadowAugmentedSceneLight(UsdPrim const& prim) {
    return prim && prim.IsA<UsdLuxDistantLight>();
}

static HdxShadowParams _DuBuildSceneLightShadowParams(
    UsdStageRefPtr const& stage,
    UsdPrim const& prim);

class DuSceneLightShadowSchemaDataSource final : public HdContainerDataSource {
public:
    DuSceneLightShadowSchemaDataSource(
        HdContainerDataSourceHandle const& base,
        UsdStageRefPtr stage,
        SdfPath primPath)
        : _base(base)
        , _stage(std::move(stage))
        , _primPath(std::move(primPath)) {}

    HdDataSourceBaseHandle Get(TfToken const& name) override {
        HdDataSourceBaseHandle const baseValue = _base ? _base->Get(name) : nullptr;
        if (baseValue) {
            return baseValue;
        }

        const UsdPrim prim = _stage ? _stage->GetPrimAtPath(_primPath) : UsdPrim();
        if (!_DuIsShadowAugmentedSceneLight(prim)) {
            return nullptr;
        }

        const bool shadowEnabled = _DuGetShadowEnabled(prim);
        if (name == HdLightTokens->hasShadow) {
            return HdRetainedTypedSampledDataSource<bool>::New(shadowEnabled);
        }
        if (name == HdLightTokens->shadowCollection) {
            return HdRetainedSampledDataSource::New(
                VtValue(_DuMakeShadowCollection()));
        }
        if (name == HdLightTokens->shadowParams) {
            std::ofstream log("/tmp/dreamusd-shadow-debug.log", std::ios::app);
            if (log) {
                log << "[DreamUSD][SceneLightShadowAugment] path="
                    << _primPath.GetString()
                    << " enabled=" << (shadowEnabled ? "true" : "false")
                    << std::endl;
            }
            return HdRetainedSampledDataSource::New(
                VtValue(_DuBuildSceneLightShadowParams(_stage, prim)));
        }

        return nullptr;
    }

    TfTokenVector GetNames() override {
        TfTokenVector names = _base ? _base->GetNames() : TfTokenVector{};
        const UsdPrim prim = _stage ? _stage->GetPrimAtPath(_primPath) : UsdPrim();
        if (!_DuIsShadowAugmentedSceneLight(prim)) {
            return names;
        }

        auto appendIfMissing = [&names](TfToken const& token) {
            if (std::find(names.begin(), names.end(), token) == names.end()) {
                names.push_back(token);
            }
        };
        appendIfMissing(HdLightTokens->hasShadow);
        appendIfMissing(HdLightTokens->shadowParams);
        appendIfMissing(HdLightTokens->shadowCollection);
        return names;
    }

private:
    HdContainerDataSourceHandle _base;
    UsdStageRefPtr _stage;
    SdfPath _primPath;
};

class DuSceneLightShadowPrimDataSource final : public HdContainerDataSource {
public:
    DuSceneLightShadowPrimDataSource(
        HdContainerDataSourceHandle const& base,
        UsdStageRefPtr stage,
        SdfPath primPath)
        : _base(base)
        , _stage(std::move(stage))
        , _primPath(std::move(primPath))
        , _lightSchema(HdContainerDataSource::Cast(
              base ? base->Get(HdLightSchemaTokens->light) : nullptr)) {}

    HdDataSourceBaseHandle Get(TfToken const& name) override {
        if (name == HdLightSchemaTokens->light) {
            return std::make_shared<DuSceneLightShadowSchemaDataSource>(
                _lightSchema, _stage, _primPath);
        }
        return _base ? _base->Get(name) : nullptr;
    }

    TfTokenVector GetNames() override {
        TfTokenVector names = _base ? _base->GetNames() : TfTokenVector{};
        if (std::find(names.begin(), names.end(), HdLightSchemaTokens->light)
            == names.end()) {
            names.push_back(HdLightSchemaTokens->light);
        }
        return names;
    }

private:
    HdContainerDataSourceHandle _base;
    UsdStageRefPtr _stage;
    SdfPath _primPath;
    HdContainerDataSourceHandle _lightSchema;
};

TF_DECLARE_REF_PTRS(DuSceneLightShadowSceneIndex);

class DuSceneLightShadowSceneIndex final
    : public HdSingleInputFilteringSceneIndexBase {
public:
    static DuSceneLightShadowSceneIndexRefPtr New(
        HdSceneIndexBaseRefPtr const& inputSceneIndex,
        UsdStageRefPtr stage) {
        return TfCreateRefPtr(
            new DuSceneLightShadowSceneIndex(inputSceneIndex, std::move(stage)));
    }

    HdSceneIndexPrim GetPrim(SdfPath const& primPath) const override {
        HdSceneIndexPrim prim = _GetInputSceneIndex()->GetPrim(primPath);
        if (!prim.dataSource || !_stage || !HdPrimTypeIsLight(prim.primType)) {
            return prim;
        }

        const UsdPrim usdPrim = _stage->GetPrimAtPath(primPath);
        if (!usdPrim || !UsdLuxLightAPI(usdPrim)) {
            return prim;
        }

        prim.dataSource = std::make_shared<DuSceneLightShadowPrimDataSource>(
            prim.dataSource,
            _stage,
            primPath);
        return prim;
    }

    SdfPathVector GetChildPrimPaths(SdfPath const& primPath) const override {
        return _GetInputSceneIndex()->GetChildPrimPaths(primPath);
    }

protected:
    void _PrimsAdded(
        const HdSceneIndexBase&,
        const HdSceneIndexObserver::AddedPrimEntries& entries) override {
        for (HdSceneIndexObserver::AddedPrimEntry const& entry : entries) {
            if (HdPrimTypeIsLight(entry.primType)) {
                _lightPaths.insert(entry.primPath);
            }
        }
        _SendPrimsAdded(entries);
        _DirtyAllLights();
    }

    void _PrimsRemoved(
        const HdSceneIndexBase&,
        const HdSceneIndexObserver::RemovedPrimEntries& entries) override {
        for (HdSceneIndexObserver::RemovedPrimEntry const& entry : entries) {
            for (auto it = _lightPaths.begin(); it != _lightPaths.end();) {
                if (it->HasPrefix(entry.primPath)) {
                    it = _lightPaths.erase(it);
                } else {
                    ++it;
                }
            }
        }
        _SendPrimsRemoved(entries);
        _DirtyAllLights();
    }

    void _PrimsDirtied(
        const HdSceneIndexBase&,
        const HdSceneIndexObserver::DirtiedPrimEntries& entries) override {
        HdSceneIndexObserver::DirtiedPrimEntries forwarded = entries;
        bool dirtyAllLights = false;

        for (size_t i = 0; i < forwarded.size(); ++i) {
            const SdfPath& primPath = forwarded[i].primPath;
            if (_lightPaths.find(primPath) != _lightPaths.end()) {
                forwarded[i].dirtyLocators.insert(HdLightSchema::GetDefaultLocator());
            } else {
                dirtyAllLights = true;
            }
        }

        _SendPrimsDirtied(forwarded);
        if (dirtyAllLights) {
            _DirtyAllLights();
        }
    }

private:
    DuSceneLightShadowSceneIndex(
        HdSceneIndexBaseRefPtr const& inputSceneIndex,
        UsdStageRefPtr stage)
        : HdSingleInputFilteringSceneIndexBase(inputSceneIndex)
        , _stage(std::move(stage)) {
        _CollectLightPaths(SdfPath::AbsoluteRootPath());
        SetDisplayName("DreamUSD Scene Light Shadow Scene Index");
    }

    void _CollectLightPaths(SdfPath const& primPath) {
        for (SdfPath const& childPath : _GetInputSceneIndex()->GetChildPrimPaths(primPath)) {
            const HdSceneIndexPrim childPrim = _GetInputSceneIndex()->GetPrim(childPath);
            if (HdPrimTypeIsLight(childPrim.primType)) {
                _lightPaths.insert(childPath);
            }
            _CollectLightPaths(childPath);
        }
    }

    void _DirtyAllLights() {
        if (_lightPaths.empty()) {
            return;
        }

        HdSceneIndexObserver::DirtiedPrimEntries entries;
        entries.reserve(_lightPaths.size());
        static const HdDataSourceLocatorSet lightLocatorSet{
            HdLightSchema::GetDefaultLocator()};
        for (SdfPath const& lightPath : _lightPaths) {
            entries.push_back({lightPath, lightLocatorSet});
        }
        _SendPrimsDirtied(entries);
    }

    UsdStageRefPtr _stage;
    std::set<SdfPath, SdfPath::FastLessThan> _lightPaths;
};

static HdSceneIndexBaseRefPtr _DuAppendSceneLightShadowSceneIndex(
    std::string const&,
    HdSceneIndexBaseRefPtr const& inputScene,
    HdContainerDataSourceHandle const&) {
    if (!g_duSceneIndexAugmentStage) {
        return inputScene;
    }

    std::ofstream log("/tmp/dreamusd-shadow-debug.log", std::ios::app);
    if (log) {
        log << "[DreamUSD][SceneLightShadowSceneIndex] attached=true" << std::endl;
    }

    return DuSceneLightShadowSceneIndex::New(
        inputScene,
        g_duSceneIndexAugmentStage);
}

static void _DuRegisterSceneLightShadowSceneIndex() {
    static std::once_flag once;
    std::call_once(once, []() {
        HdSceneIndexPluginRegistry::GetInstance().RegisterSceneIndexForRenderer(
            std::string(),
            &_DuAppendSceneLightShadowSceneIndex,
            nullptr,
            10,
            HdSceneIndexPluginRegistry::InsertionOrderAtStart);
    });
}

class DuScopedSceneIndexAugmentStage final {
public:
    explicit DuScopedSceneIndexAugmentStage(UsdStageRefPtr stage)
        : _previous(std::move(g_duSceneIndexAugmentStage)) {
        g_duSceneIndexAugmentStage = std::move(stage);
    }

    ~DuScopedSceneIndexAugmentStage() {
        g_duSceneIndexAugmentStage = std::move(_previous);
    }

private:
    UsdStageRefPtr _previous;
};

class DuSceneIndexLightSchemaDataSource final : public HdContainerDataSource {
public:
    explicit DuSceneIndexLightSchemaDataSource(DuViewerLightState state)
        : _state(std::move(state)) {}

    HdDataSourceBaseHandle Get(TfToken const& name) override {
        const auto it = _state.params.find(name);
        if (it == _state.params.end()) {
            return nullptr;
        }
        return HdRetainedSampledDataSource::New(it->second);
    }

    TfTokenVector GetNames() override {
        TfTokenVector names;
        names.reserve(_state.params.size());
        for (auto const& [token, _] : _state.params) {
            names.push_back(token);
        }
        return names;
    }

private:
    DuViewerLightState _state;
};

class DuSceneIndexLightPrimDataSource final : public HdContainerDataSource {
public:
    explicit DuSceneIndexLightPrimDataSource(DuViewerLightState state)
        : _state(std::move(state)) {}

    HdDataSourceBaseHandle Get(TfToken const& name) override {
        if (name == HdLightSchema::GetSchemaToken()) {
            return std::make_shared<DuSceneIndexLightSchemaDataSource>(_state);
        }
        if (name == HdXformSchema::GetSchemaToken()) {
            return HdXformSchema::Builder()
                .SetMatrix(HdRetainedTypedSampledDataSource<GfMatrix4d>::New(
                    _state.transform))
                .Build();
        }
        return nullptr;
    }

    TfTokenVector GetNames() override {
        return {HdLightSchema::GetSchemaToken(), HdXformSchema::GetSchemaToken()};
    }

private:
    DuViewerLightState _state;
};

class DuViewerLightDelegate final : public HdSceneDelegate {
public:
    DuViewerLightDelegate(HdRenderIndex* index, SdfPath const& delegateId)
        : HdSceneDelegate(index, delegateId) {}

    void SyncLights(std::vector<DuViewerLightState> const& lights) {
        HdChangeTracker& tracker = GetRenderIndex().GetChangeTracker();
        std::vector<SdfPath> keepIds;
        keepIds.reserve(lights.size());

        for (size_t i = 0; i < lights.size(); ++i) {
            const SdfPath id = GetDelegateID().AppendChild(
                TfToken(TfStringPrintf("light_%zu", i)));
            keepIds.push_back(id);

            if (_lights.find(id) == _lights.end()) {
                GetRenderIndex().InsertSprim(lights[i].lightType, this, id);
            }

            _lights[id] = lights[i];
            tracker.MarkSprimDirty(id, HdLight::AllDirty);
        }

        for (auto it = _lights.begin(); it != _lights.end();) {
            if (std::find(keepIds.begin(), keepIds.end(), it->first) == keepIds.end()) {
                GetRenderIndex().RemoveSprim(it->second.lightType, it->first);
                it = _lights.erase(it);
            } else {
                ++it;
            }
        }
    }

    GfMatrix4d GetTransform(SdfPath const& id) override {
        auto it = _lights.find(id);
        return it != _lights.end() ? it->second.transform : GfMatrix4d(1.0);
    }

    bool GetVisible(SdfPath const& id) override {
        auto it = _lights.find(id);
        return it != _lights.end() ? it->second.visible : false;
    }

    VtValue Get(SdfPath const& id, TfToken const& key) override {
        auto it = _lights.find(id);
        if (it == _lights.end()) {
            return VtValue();
        }

        if (key == HdTokens->transform) {
            return VtValue(it->second.transform);
        }
        auto paramIt = it->second.params.find(key);
        if (paramIt != it->second.params.end()) {
            return paramIt->second;
        }

        return VtValue();
    }

    VtValue GetLightParamValue(SdfPath const& id, TfToken const& paramName) override {
        auto it = _lights.find(id);
        if (it == _lights.end()) {
            return VtValue();
        }

        if (paramName == HdLightTokens->shadowParams) {
            return VtValue(it->second.shadowParams);
        }
        if (paramName == HdLightTokens->shadowCollection) {
            return VtValue(it->second.shadowCollection);
        }
        if (paramName == HdTokens->transform) {
            return VtValue(it->second.transform);
        }

        auto paramIt = it->second.params.find(paramName);
        if (paramIt != it->second.params.end()) {
            return paramIt->second;
        }

        return VtValue();
    }

private:
    std::map<SdfPath, DuViewerLightState, SdfPath::FastLessThan> _lights;
};

class DuDistantShadowMatrix final : public HdxShadowMatrixComputation {
public:
    DuDistantShadowMatrix(GfVec3d const& lightDirection, GfRange3d const& sceneBounds) {
        GfVec3d direction = lightDirection;
        if (direction.GetLengthSq() < 1e-6) {
            direction = GfVec3d(0.0, 0.0, 1.0);
        } else {
            direction.Normalize();
        }

        GfRange3d bounds = sceneBounds;
        if (bounds.IsEmpty()) {
            bounds = GfRange3d(GfVec3d(-5.0), GfVec3d(5.0));
        }

        const GfVec3d center = bounds.GetMidpoint();
        const GfVec3d size = bounds.GetSize();
        const double radius = std::max({size[0], size[1], size[2], 10.0}) * 0.75;
        const GfVec3d eye = center + direction * (radius * 2.0);

        GfVec3d up(0.0, 1.0, 0.0);
        if (std::abs(GfDot(direction, up)) > 0.98) {
            up = GfVec3d(1.0, 0.0, 0.0);
        }

        GfMatrix4d view;
        view.SetLookAt(eye, center, up);

        GfVec3d minPt( std::numeric_limits<double>::max());
        GfVec3d maxPt(-std::numeric_limits<double>::max());
        for (int x = 0; x < 2; ++x) {
            for (int y = 0; y < 2; ++y) {
                for (int z = 0; z < 2; ++z) {
                    const GfVec3d corner(
                        x ? bounds.GetMax()[0] : bounds.GetMin()[0],
                        y ? bounds.GetMax()[1] : bounds.GetMin()[1],
                        z ? bounds.GetMax()[2] : bounds.GetMin()[2]);
                    const GfVec3d p = view.Transform(corner);
                    minPt[0] = std::min(minPt[0], p[0]);
                    minPt[1] = std::min(minPt[1], p[1]);
                    minPt[2] = std::min(minPt[2], p[2]);
                    maxPt[0] = std::max(maxPt[0], p[0]);
                    maxPt[1] = std::max(maxPt[1], p[1]);
                    maxPt[2] = std::max(maxPt[2], p[2]);
                }
            }
        }

        const double padding = std::max(radius * 0.15, 1.0);
        GfFrustum frustum;
        frustum.SetProjectionType(GfFrustum::Orthographic);
        frustum.SetWindow(GfRange2d(
            GfVec2d(minPt[0] - padding, minPt[1] - padding),
            GfVec2d(maxPt[0] + padding, maxPt[1] + padding)));
        frustum.SetNearFar(GfRange1d(
            std::max(0.1, -(maxPt[2] + padding)),
            std::max(1.0, -(minPt[2] - padding))));
        frustum.SetPositionAndRotationFromMatrix(view.GetInverse());

        _shadowMatrix =
            frustum.ComputeViewMatrix() * frustum.ComputeProjectionMatrix();
    }

    std::vector<GfMatrix4d> Compute(
        const GfVec4f&,
        CameraUtilConformWindowPolicy) override {
        return {_shadowMatrix};
    }

    std::vector<GfMatrix4d> Compute(
        const CameraUtilFraming&,
        CameraUtilConformWindowPolicy) override {
        return {_shadowMatrix};
    }

private:
    GfMatrix4d _shadowMatrix{1.0};
};

class DuPointShadowMatrix final : public HdxShadowMatrixComputation {
public:
    DuPointShadowMatrix(GfVec3d const& lightPosition, GfRange3d const& sceneBounds) {
        GfRange3d bounds = sceneBounds;
        if (bounds.IsEmpty()) {
            bounds = GfRange3d(GfVec3d(-5.0), GfVec3d(5.0));
        }

        const GfVec3d center = bounds.GetMidpoint();
        GfVec3d direction = center - lightPosition;
        if (direction.GetLengthSq() < 1e-6) {
            direction = GfVec3d(0.0, -1.0, 0.0);
        } else {
            direction.Normalize();
        }

        GfVec3d up(0.0, 1.0, 0.0);
        if (std::abs(GfDot(direction, up)) > 0.98) {
            up = GfVec3d(1.0, 0.0, 0.0);
        }

        GfMatrix4d view;
        view.SetLookAt(lightPosition, center, up);

        double nearDist = std::numeric_limits<double>::max();
        double farDist = 1.0;
        double maxTan = std::tan(GfDegreesToRadians(20.0));

        for (int x = 0; x < 2; ++x) {
            for (int y = 0; y < 2; ++y) {
                for (int z = 0; z < 2; ++z) {
                    const GfVec3d corner(
                        x ? bounds.GetMax()[0] : bounds.GetMin()[0],
                        y ? bounds.GetMax()[1] : bounds.GetMin()[1],
                        z ? bounds.GetMax()[2] : bounds.GetMin()[2]);
                    const GfVec3d p = view.Transform(corner);
                    const double depth = -p[2];
                    if (depth <= 0.01) {
                        continue;
                    }
                    nearDist = std::min(nearDist, depth);
                    farDist = std::max(farDist, depth);
                    maxTan = std::max(maxTan, std::abs(p[0]) / depth);
                    maxTan = std::max(maxTan, std::abs(p[1]) / depth);
                }
            }
        }

        const double fallbackDistance = std::max((center - lightPosition).GetLength(), 10.0);
        if (nearDist == std::numeric_limits<double>::max()) {
            nearDist = std::max(0.1, fallbackDistance * 0.1);
            farDist = fallbackDistance * 2.5;
        } else {
            nearDist = std::max(0.1, nearDist * 0.8);
            farDist = std::max(nearDist + 1.0, farDist * 1.25);
        }

        const double fovDegrees =
            GfClamp(GfRadiansToDegrees(2.0 * std::atan(maxTan * 1.15)), 35.0, 140.0);

        GfFrustum frustum;
        frustum.SetPerspective(fovDegrees, 1.0, nearDist, farDist);
        _shadowMatrix = frustum.ComputeProjectionMatrix();
    }

    std::vector<GfMatrix4d> Compute(
        const GfVec4f&,
        CameraUtilConformWindowPolicy) override {
        return {_shadowMatrix};
    }

    std::vector<GfMatrix4d> Compute(
        const CameraUtilFraming&,
        CameraUtilConformWindowPolicy) override {
        return {_shadowMatrix};
    }

private:
    GfMatrix4d _shadowMatrix{1.0};
};

static HdxShadowParams _DuBuildSceneLightShadowParams(
    UsdStageRefPtr const& stage,
    UsdPrim const& prim) {
    HdxShadowParams params;
    params.enabled = _DuGetShadowEnabled(prim);
    params.bias = -0.0005f;
    params.blur = 0.0f;
    params.resolution = 2048;

    if (!params.enabled || !stage || !prim || !_DuIsShadowAugmentedSceneLight(prim)) {
        return params;
    }

    const GfRange3d sceneBounds = _DuComputeSceneBounds(stage);
    const GfMatrix4d authoredTransform =
        UsdGeomXformCache(UsdTimeCode::Default()).GetLocalToWorldTransform(prim);

    GfVec3d direction = authoredTransform.TransformDir(GfVec3d(0.0, 0.0, -1.0));
    if (direction.GetLengthSq() < 1e-6) {
        direction = GfVec3d(0.0, 0.0, -1.0);
    } else {
        direction.Normalize();
    }

    params.shadowMatrix =
        std::make_shared<DuDistantShadowMatrix>(direction, sceneBounds);
    return params;
}

struct DuHydraEngine {
    // CPU framebuffer for readback
    std::vector<uint8_t> framebuffer;
    UsdStageRefPtr stage;
    std::unique_ptr<UsdImagingGLEngine> glEngine;

    // Hgi and Hydra pipeline
    HgiUniquePtr hgi;
    HdDriver hgiDriver;
    HdRenderIndex* renderIndex = nullptr;
    HdPluginRenderDelegateUniqueHandle renderDelegateHandle;
    UsdImagingDelegate* sceneDelegate = nullptr;
    HdxTaskController* taskController = nullptr;
    HdxSelectionTrackerSharedPtr selTracker;
    HdEngine engine;

    // GPU image output (opaque handles)
    uint64_t outputImage = 0;
    uint64_t outputImageView = 0;
    uint64_t renderSemaphore = 0;

    // Camera state
    GfVec3d eye{0, 0, 10};
    GfVec3d target{0, 0, 0};
    GfVec3d up{0, 1, 0};
    double fovYRadians = GfDegreesToRadians(60.0);
    double nearPlane = 0.1;
    double farPlane = 10000.0;

    // Current render delegate
    TfToken currentRdId;
    TfToken currentAov = HdAovTokens->color;
    HdRprimCollection currentCollection;
    DuDisplayMode displayMode = DU_DISPLAY_SMOOTH_SHADED;
    UsdImagingGLRenderParams renderParams;
    SdfPathVector selectedPaths;
    bool msaaEnabled = true;
    float complexity = 1.0f;
    bool showGuides = false;
    bool showProxy = true;
    bool showRender = true;
    bool cullBackfaces = false;
    bool enableSceneMaterials = true;
    bool domeLightCameraVisibility = true;

    // Current lighting mode
    bool enableLighting = true;
    bool enableShadows = false;
    bool hasSceneLights = false;
    bool sceneLightShadowOverridesApplied = false;
    std::unique_ptr<DuViewerLightDelegate> viewerLightDelegate;
    bool shadowDebugLogged = false;
    bool shadowTaskDebugLogged = false;
    std::vector<DuSceneIndexLightOverrideEntry> sceneIndexLightOverrides;
    SdfPathVector sceneIndexMirroredLightPaths;

    // Output dimensions
    uint32_t width = 0;
    uint32_t height = 0;
};

static bool _DuStageHasSceneLights(
    UsdStageRefPtr const& stage,
    bool includeHistoricalDefaultLights = false);
static GfRange3d _DuComputeSceneBounds(UsdStageRefPtr const& stage);
static void _DuEnsureSceneLightShadows(
    UsdStageRefPtr const& stage,
    bool includeHistoricalDefaultLights = false);
static void _DuApplyMirroredSceneLightOverrides(
    DuHydraEngine* engine,
    std::vector<DuViewerLightState> const& viewerLights);
static void _DuRestoreMirroredSceneLightOverrides(DuHydraEngine* engine);
static void _DuSyncSceneIndexMirroredLights(
    DuHydraEngine* engine,
    std::vector<DuViewerLightState> const& viewerLights);

static std::string _DuToLower(std::string value) {
    std::transform(value.begin(), value.end(), value.begin(), [](unsigned char c) {
        return static_cast<char>(std::tolower(c));
    });
    return value;
}

static void _DuBuildCameraMatrices(
    DuHydraEngine const* engine,
    uint32_t width,
    uint32_t height,
    GfMatrix4d* viewMatrix,
    GfMatrix4d* projectionMatrix) {
    if (!engine || !viewMatrix || !projectionMatrix) {
        return;
    }

    viewMatrix->SetLookAt(engine->eye, engine->target, engine->up);

    GfFrustum frustum;
    const double aspectRatio =
        (height > 0) ? static_cast<double>(width) / static_cast<double>(height) : 1.0;
    const double fovYDegrees = GfRadiansToDegrees(engine->fovYRadians);
    frustum.SetPerspective(
        fovYDegrees > 0.0 ? fovYDegrees : 60.0,
        aspectRatio,
        std::max(1e-5, engine->nearPlane),
        std::max(engine->nearPlane + 1e-4, engine->farPlane));
    frustum.SetPositionAndRotationFromMatrix(viewMatrix->GetInverse());
    *projectionMatrix = frustum.ComputeProjectionMatrix();
}

static void _DuComputeAutoClipRange(
    DuHydraEngine const* engine,
    double* nearPlane,
    double* farPlane) {
    if (!engine || !nearPlane || !farPlane) {
        return;
    }

    constexpr double kDefaultNear = 1.0;
    constexpr double kDefaultFar = 2000000.0;
    constexpr double kMaxGoodZResolution = 5e4;

    const GfRange3d bounds = _DuComputeSceneBounds(engine->stage);
    if (bounds.IsEmpty()) {
        *nearPlane = kDefaultNear;
        *farPlane = kDefaultFar;
        return;
    }

    GfMatrix4d viewMatrix(1.0);
    GfMatrix4d projectionMatrix(1.0);
    _DuBuildCameraMatrices(
        engine,
        std::max<uint32_t>(engine->width, 1),
        std::max<uint32_t>(engine->height, 1),
        &viewMatrix,
        &projectionMatrix);

    double computedNear = std::numeric_limits<double>::max();
    double computedFar = 1.0;
    for (int x = 0; x < 2; ++x) {
        for (int y = 0; y < 2; ++y) {
            for (int z = 0; z < 2; ++z) {
                const GfVec3d corner(
                    x ? bounds.GetMax()[0] : bounds.GetMin()[0],
                    y ? bounds.GetMax()[1] : bounds.GetMin()[1],
                    z ? bounds.GetMax()[2] : bounds.GetMin()[2]);
                const GfVec3d point = viewMatrix.Transform(corner);
                const double depth = -point[2];
                if (depth <= 0.0) {
                    continue;
                }
                computedNear = std::min(computedNear, depth);
                computedFar = std::max(computedFar, depth);
            }
        }
    }

    if (computedNear == std::numeric_limits<double>::max()) {
        const double fallbackDistance =
            std::max((engine->target - engine->eye).GetLength(), 10.0);
        *nearPlane = kDefaultNear;
        *farPlane = std::max(kDefaultNear + 1.0, fallbackDistance * 4.0);
        return;
    }

    computedNear = std::max(kDefaultNear, computedNear * 0.99);
    computedFar = std::max(computedNear + 1.0, computedFar * 1.01);

    const double precisionNear = computedFar / kMaxGoodZResolution;
    if (precisionNear > computedNear) {
        computedNear = std::min(
            std::max(kDefaultNear, precisionNear),
            computedFar - 1.0);
    }

    *nearPlane = computedNear;
    *farPlane = computedFar;
}

static UsdImagingGLDrawMode _DuToGlDrawMode(DuDisplayMode mode) {
    switch (mode) {
        case DU_DISPLAY_WIREFRAME:
            return UsdImagingGLDrawMode::DRAW_WIREFRAME;
        case DU_DISPLAY_WIREFRAME_ON_SHADED:
            return UsdImagingGLDrawMode::DRAW_WIREFRAME_ON_SURFACE;
        case DU_DISPLAY_FLAT_SHADED:
            return UsdImagingGLDrawMode::DRAW_SHADED_FLAT;
        case DU_DISPLAY_POINTS:
            return UsdImagingGLDrawMode::DRAW_POINTS;
        case DU_DISPLAY_GEOM_ONLY:
            return UsdImagingGLDrawMode::DRAW_GEOM_ONLY;
        case DU_DISPLAY_GEOM_FLAT:
            return UsdImagingGLDrawMode::DRAW_GEOM_FLAT;
        case DU_DISPLAY_GEOM_SMOOTH:
            return UsdImagingGLDrawMode::DRAW_GEOM_SMOOTH;
        case DU_DISPLAY_TEXTURED:
        case DU_DISPLAY_SMOOTH_SHADED:
        default:
            return UsdImagingGLDrawMode::DRAW_SHADED_SMOOTH;
    }
}

static void _DuApplyShadowSetting(DuHydraEngine* engine) {
    if (!engine || !engine->glEngine) {
        return;
    }

    HdxShadowTaskParams shadowParams;
    shadowParams.enableLighting = engine->renderParams.enableLighting;
    shadowParams.depthBiasEnable = true;
    shadowParams.depthBiasConstantFactor = 1.0f;
    shadowParams.depthBiasSlopeFactor = 1.0f;
    shadowParams.cullStyle = HdCullStyleBackUnlessDoubleSided;

    if (engine->glEngine->_taskControllerSceneIndex) {
        engine->glEngine->_taskControllerSceneIndex->SetEnableShadows(engine->enableShadows);
        engine->glEngine->_taskControllerSceneIndex->SetShadowParams(shadowParams);
    } else if (engine->glEngine->_taskController) {
        engine->glEngine->_taskController->SetEnableShadows(engine->enableShadows);
        engine->glEngine->_taskController->SetShadowParams(shadowParams);
    }

    const UsdImagingGLRendererSettingsList settings =
        engine->glEngine->GetRendererSettingsList();

    const auto isExactShadowToggle = [](std::string const& text) {
        return text == "enableshadows"
            || text == "enable shadows"
            || text == "shadowenable"
            || text == "shadow enable";
    };

    for (UsdImagingGLRendererSetting const& setting : settings) {
        if (setting.type != UsdImagingGLRendererSetting::TYPE_FLAG) {
            continue;
        }

        const std::string key = _DuToLower(setting.key.GetString());
        const std::string name = _DuToLower(setting.name);
        if (!isExactShadowToggle(key) && !isExactShadowToggle(name)
            && key.find("shadow") == std::string::npos
            && name.find("shadow") == std::string::npos) {
            continue;
        }

        engine->glEngine->SetRendererSetting(
            setting.key, VtValue(engine->enableShadows));
    }
}

static void _DuApplyRendererSettingFlag(
    DuHydraEngine* engine,
    char const* settingName,
    bool value)
{
    if (!engine || !engine->glEngine || !settingName) {
        return;
    }

    engine->glEngine->SetRendererSetting(TfToken(settingName), VtValue(value));
}

static const char* _DuDupString(std::string const& value) {
    char* dup = static_cast<char*>(malloc(value.size() + 1));
    if (!dup) {
        return nullptr;
    }
    memcpy(dup, value.c_str(), value.size() + 1);
    return dup;
}

static std::string _DuRendererSettingValueToString(VtValue const& value) {
    if (value.IsHolding<bool>()) {
        return value.UncheckedGet<bool>() ? "true" : "false";
    }
    if (value.IsHolding<int>()) {
        return std::to_string(value.UncheckedGet<int>());
    }
    if (value.IsHolding<unsigned int>()) {
        return std::to_string(value.UncheckedGet<unsigned int>());
    }
    if (value.IsHolding<float>()) {
        return TfStringPrintf("%.6g", value.UncheckedGet<float>());
    }
    if (value.IsHolding<std::string>()) {
        return value.UncheckedGet<std::string>();
    }
    return value.IsEmpty() ? std::string() : value.GetTypeName();
}

static DuRendererSettingType _DuToRendererSettingType(
    UsdImagingGLRendererSetting::Type type)
{
    switch (type) {
        case UsdImagingGLRendererSetting::TYPE_FLAG:
            return DU_RENDERER_SETTING_FLAG;
        case UsdImagingGLRendererSetting::TYPE_INT:
            return DU_RENDERER_SETTING_INT;
        case UsdImagingGLRendererSetting::TYPE_FLOAT:
            return DU_RENDERER_SETTING_FLOAT;
        case UsdImagingGLRendererSetting::TYPE_STRING:
        default:
            return DU_RENDERER_SETTING_STRING;
    }
}

static void _DuConfigureColorOutput(DuHydraEngine* engine) {
    if (!engine || !engine->glEngine) {
        return;
    }

    if (engine->currentAov != HdAovTokens->color) {
        return;
    }

    auto configureDescriptor = [](HdAovDescriptor desc) {
        if (desc.format == HdFormatInvalid) {
            return desc;
        }
        desc.format = HdFormatUNorm8Vec4;
        return desc;
    };

    if (engine->glEngine->_taskControllerSceneIndex) {
        HdAovDescriptor desc =
            engine->glEngine->_taskControllerSceneIndex->GetRenderOutputSettings(
                HdAovTokens->color);
        desc = configureDescriptor(desc);
        if (desc.format != HdFormatInvalid) {
            engine->glEngine->_taskControllerSceneIndex->SetRenderOutputSettings(
                HdAovTokens->color,
                desc);
        }
    } else if (engine->glEngine->_taskController) {
        HdAovDescriptor desc =
            engine->glEngine->_taskController->GetRenderOutputSettings(
                HdAovTokens->color);
        desc = configureDescriptor(desc);
        if (desc.format != HdFormatInvalid) {
            engine->glEngine->_taskController->SetRenderOutputSettings(
                HdAovTokens->color,
                desc);
        }
    }
}

static void _DuConfigureViewerLighting(
    DuHydraEngine* engine,
    GfMatrix4d const& viewMatrix,
    GfMatrix4d const& projectionMatrix,
    bool useSceneLights) {
    if (!engine || !engine->glEngine) {
        return;
    }

    engine->renderParams.enableSceneLights = false;

    if (!engine->enableLighting) {
        engine->glEngine->SetLightingState(
            GlfSimpleLightVector(),
            GlfSimpleMaterial(),
            GfVec4f(0.0f));
        return;
    }

    if (useSceneLights) {
        // Follow usdview / official GLEngine behavior:
        // viewer lighting state carries only fallback lights while
        // USD scene lights are enabled through render params.
        engine->renderParams.enableSceneLights = true;

        auto lightingContext = GlfSimpleLightingContext::New();
        lightingContext->SetUseLighting(true);
        lightingContext->SetCamera(viewMatrix, projectionMatrix);
        lightingContext->SetSceneAmbient(GfVec4f(0.0f));
        lightingContext->SetLights({});
        engine->glEngine->SetLightingState(lightingContext);
        return;
    }

    GlfSimpleLight key;
    {
        const GfVec3d pos(5.0, 8.0, 4.0);
        const GfVec3d dir = (-pos).GetNormalized();
        key.SetPosition(GfVec4f(dir[0], dir[1], dir[2], 0.0f));
        key.SetDiffuse(GfVec4f(4.0f, 3.8f, 3.6f, 1.0f));
        key.SetSpecular(GfVec4f(4.0f, 3.8f, 3.6f, 1.0f));
        key.SetHasShadow(false);
    }

    GlfSimpleLight fill;
    {
        const GfVec3d pos(-6.0, 4.0, -3.0);
        const GfVec3d dir = (-pos).GetNormalized();
        fill.SetPosition(GfVec4f(dir[0], dir[1], dir[2], 0.0f));
        fill.SetDiffuse(GfVec4f(0.56f, 0.64f, 0.8f, 1.0f));
        fill.SetSpecular(GfVec4f(0.0f));
        fill.SetHasShadow(false);
    }

    auto lightingContext = GlfSimpleLightingContext::New();
    lightingContext->SetUseLighting(true);
    lightingContext->SetCamera(viewMatrix, projectionMatrix);
    lightingContext->SetSceneAmbient(GfVec4f(0.12f, 0.13f, 0.15f, 1.0f));
    lightingContext->SetLights({key, fill});
    engine->glEngine->SetLightingState(lightingContext);
}

static uint8_t _DuTonemapLinearToSrgb8(float value) {
    const float clamped = std::max(0.0f, value);
    const float mapped = clamped / (1.0f + clamped);
    const float srgb = std::pow(mapped, 1.0f / 2.2f);
    const int byte = static_cast<int>(srgb * 255.0f + 0.5f);
    return static_cast<uint8_t>(byte < 0 ? 0 : (byte > 255 ? 255 : byte));
}

static uint8_t _DuLinearToUnorm8(float value) {
    const float clamped = std::clamp(value, 0.0f, 1.0f);
    const int byte = static_cast<int>(clamped * 255.0f + 0.5f);
    return static_cast<uint8_t>(byte < 0 ? 0 : (byte > 255 ? 255 : byte));
}

static void _DuSyncRenderParams(DuHydraEngine* engine) {
    if (!engine) {
        return;
    }

    engine->renderParams.frame = UsdTimeCode::Default();
    engine->renderParams.complexity = engine->complexity;
    engine->renderParams.drawMode = _DuToGlDrawMode(engine->displayMode);
    engine->renderParams.showGuides = engine->showGuides;
    engine->renderParams.showProxy = engine->showProxy;
    engine->renderParams.showRender = engine->showRender;
    engine->renderParams.forceRefresh = false;
    engine->renderParams.flipFrontFacing = false;
    engine->renderParams.cullStyle = engine->cullBackfaces
        ? UsdImagingGLCullStyle::CULL_STYLE_BACK_UNLESS_DOUBLE_SIDED
        : UsdImagingGLCullStyle::CULL_STYLE_NOTHING;
    engine->renderParams.enableLighting =
        engine->enableLighting && engine->displayMode != DU_DISPLAY_WIREFRAME
        && engine->displayMode != DU_DISPLAY_POINTS;
    engine->renderParams.enableSampleAlphaToCoverage = engine->msaaEnabled;
    engine->renderParams.applyRenderState = true;
    engine->renderParams.gammaCorrectColors = false;
    engine->renderParams.highlight = !engine->selectedPaths.empty();
    engine->renderParams.overrideColor = GfVec4f(0.0f);
    engine->renderParams.wireframeColor = GfVec4f(0.0f);
    engine->renderParams.alphaThreshold = -1.0f;
    engine->renderParams.enableSceneMaterials = engine->enableSceneMaterials;
    engine->renderParams.enableUsdDrawModes = true;
    engine->renderParams.colorCorrectionMode = HdxColorCorrectionTokens->sRGB;
    engine->renderParams.ocioDisplay = TfToken();
    engine->renderParams.ocioView = TfToken();
    engine->renderParams.ocioColorSpace = TfToken();
    engine->renderParams.ocioLook = TfToken();
    engine->renderParams.clearColor = GfVec4f(0.02f, 0.02f, 0.025f, 1.0f);

    _DuApplyRendererSettingFlag(
        engine,
        "domeLightCameraVisibility",
        engine->domeLightCameraVisibility);
}

static bool _DuIsHistoricalDefaultLight(UsdPrim const& prim) {
    return prim && prim.GetPath().HasPrefix(SdfPath("/_DefaultLights"));
}

static bool _DuStageHasSceneLights(
    UsdStageRefPtr const& stage,
    bool includeHistoricalDefaultLights) {
    if (!stage) {
        return false;
    }

    for (UsdPrim prim : stage->Traverse()) {
        if (!UsdLuxLightAPI(prim)) {
            continue;
        }
        if (!includeHistoricalDefaultLights && _DuIsHistoricalDefaultLight(prim)) {
            continue;
        }
        if (UsdLuxLightAPI(prim)) {
            return true;
        }
    }

    return false;
}

static GfRange3d _DuComputeSceneBounds(UsdStageRefPtr const& stage) {
    if (!stage) {
        return GfRange3d();
    }

    UsdGeomBBoxCache bboxCache(
        UsdTimeCode::Default(),
        {UsdGeomTokens->default_, UsdGeomTokens->render, UsdGeomTokens->proxy});
    return bboxCache.ComputeWorldBound(stage->GetPseudoRoot()).ComputeAlignedBox();
}

static bool _DuGetShadowEnabled(UsdPrim const& prim) {
    UsdLuxShadowAPI shadow(prim);
    if (!shadow) {
        return false;
    }

    bool enabled = false;
    if (UsdAttribute attr = shadow.GetShadowEnableAttr()) {
        attr.Get(&enabled, UsdTimeCode::Default());
    }
    return enabled;
}

static std::vector<DuViewerLightState> _DuBuildViewerLights(
    UsdStageRefPtr const& stage,
    HdRprimCollection const& shadowCollection,
    bool includeHistoricalDefaultLights = false) {
    std::vector<DuViewerLightState> result;
    if (!stage) {
        return result;
    }

    const GfRange3d sceneBounds = _DuComputeSceneBounds(stage);
    UsdGeomXformCache xformCache(UsdTimeCode::Default());
    std::ofstream log("/tmp/dreamusd-shadow-debug.log", std::ios::app);

    for (UsdPrim prim : stage->Traverse()) {
        UsdLuxLightAPI lightApi(prim);
        if (!lightApi) {
            continue;
        }

        if (!includeHistoricalDefaultLights && _DuIsHistoricalDefaultLight(prim)) {
            if (log) {
                log << "[DreamUSD][SceneLight] path=" << prim.GetPath().GetString()
                    << " type=" << prim.GetTypeName().GetString()
                    << " visible=true supported=false skipped=historicalDefault"
                    << std::endl;
            }
            continue;
        }

        const std::string typeName = prim.GetTypeName().GetString();
        const bool isDistantLight =
            prim.IsA<UsdLuxDistantLight>() || typeName == "DistantLight";
        const bool isSphereLight =
            prim.IsA<UsdLuxSphereLight>() || typeName == "SphereLight";
        const TfToken visibility =
            UsdGeomImageable(prim).ComputeVisibility(UsdTimeCode::Default());
        const bool visible = visibility != UsdGeomTokens->invisible;
        const bool supported = isDistantLight || isSphereLight;

        if (log) {
            log << "[DreamUSD][SceneLight] path=" << prim.GetPath().GetString()
                << " type=" << typeName
                << " visible=" << (visible ? "true" : "false")
                << " supported=" << (supported ? "true" : "false")
                << std::endl;
        }

        if (!visible || !supported) {
            continue;
        }

        float intensity = 1.0f;
        lightApi.GetIntensityAttr().Get(&intensity, UsdTimeCode::Default());
        float exposure = 0.0f;
        lightApi.GetExposureAttr().Get(&exposure, UsdTimeCode::Default());
        GfVec3f color(1.0f, 1.0f, 1.0f);
        lightApi.GetColorAttr().Get(&color, UsdTimeCode::Default());
        float diffuse = 1.0f;
        lightApi.GetDiffuseAttr().Get(&diffuse, UsdTimeCode::Default());
        float specular = 1.0f;
        lightApi.GetSpecularAttr().Get(&specular, UsdTimeCode::Default());
        const float power = intensity * std::pow(2.0f, exposure);
        const bool shadowEnabled = _DuGetShadowEnabled(prim);
        const GfMatrix4d authoredTransform = xformCache.GetLocalToWorldTransform(prim);

        if (isDistantLight) {
            float angle = 0.53f;
            UsdLuxDistantLight distantLight(prim);
            if (distantLight) {
                distantLight.GetAngleAttr().Get(&angle, UsdTimeCode::Default());
            }

            GfVec3d direction = authoredTransform.TransformDir(GfVec3d(0.0, 0.0, -1.0));
            if (direction.GetLengthSq() < 1e-6) {
                direction = GfVec3d(0.0, 0.0, -1.0);
            } else {
                direction.Normalize();
            }

            auto shadowComputation =
                std::make_shared<DuDistantShadowMatrix>(direction, sceneBounds);

            HdxShadowParams shadowParams;
            shadowParams.enabled = shadowEnabled;
            shadowParams.bias = -0.0005;
            shadowParams.blur = 0.0;
            shadowParams.resolution = 2048;
            shadowParams.shadowMatrix = shadowComputation;

            GlfSimpleLight glfLight;
            glfLight.SetPosition(GfVec4f(direction[0], direction[1], direction[2], 0.0f));
            glfLight.SetAmbient(GfVec4f(0.0f));
            glfLight.SetDiffuse(GfVec4f(
                color[0] * power * diffuse,
                color[1] * power * diffuse,
                color[2] * power * diffuse,
                1.0f));
            glfLight.SetSpecular(GfVec4f(
                color[0] * power * specular,
                color[1] * power * specular,
                color[2] * power * specular,
                1.0f));
            glfLight.SetAttenuation(GfVec3f(0.0f));
            glfLight.SetHasIntensity(power > 0.0f);
            glfLight.SetHasShadow(shadowEnabled);

            std::unordered_map<TfToken, VtValue, TfToken::HashFunctor> params;
            params[HdLightTokens->params] = VtValue(glfLight);
            params[HdLightTokens->color] = VtValue(color);
            params[HdLightTokens->intensity] = VtValue(intensity);
            params[HdLightTokens->exposure] = VtValue(exposure);
            params[HdLightTokens->diffuse] = VtValue(diffuse);
            params[HdLightTokens->specular] = VtValue(specular);
            params[HdLightTokens->angle] = VtValue(angle);
            params[HdLightTokens->ambient] = VtValue(0.0f);
            params[HdLightTokens->normalize] = VtValue(false);
            params[HdLightTokens->hasShadow] = VtValue(shadowEnabled);
            params[HdLightTokens->shadowEnable] = VtValue(shadowEnabled);
            params[HdLightTokens->shadowParams] = VtValue(shadowParams);
            params[HdLightTokens->shadowCollection] = VtValue(shadowCollection);

            if (log) {
                log << "[DreamUSD][SceneLightMirror] path=" << prim.GetPath().GetString()
                    << " dir=(" << direction[0] << ", " << direction[1] << ", " << direction[2] << ")"
                    << " power=" << power
                    << " shadow=" << (shadowEnabled ? "true" : "false")
                    << std::endl;
            }

            result.push_back(DuViewerLightState{
                HdPrimTypeTokens->distantLight,
                authoredTransform,
                std::move(params),
                shadowParams,
                shadowCollection,
                true,
            });
            continue;
        }

        UsdLuxSphereLight sphereLight(prim);
        float radius = 0.5f;
        bool treatAsPoint = false;
        if (sphereLight) {
            sphereLight.GetRadiusAttr().Get(&radius, UsdTimeCode::Default());
            sphereLight.GetTreatAsPointAttr().Get(&treatAsPoint, UsdTimeCode::Default());
        }
        radius = std::max(radius, 0.01f);

        const GfVec3d position = authoredTransform.ExtractTranslation();
        const GfVec3d center = sceneBounds.IsEmpty()
            ? GfVec3d(0.0, 0.0, 0.0)
            : sceneBounds.GetMidpoint();
        GfVec3d direction = center - position;
        if (direction.GetLengthSq() < 1e-6) {
            direction = GfVec3d(0.0, -1.0, 0.0);
        } else {
            direction.Normalize();
        }
        GfVec3d up(0.0, 1.0, 0.0);
        if (std::abs(GfDot(direction, up)) > 0.98) {
            up = GfVec3d(1.0, 0.0, 0.0);
        }
        GfMatrix4d lightView;
        lightView.SetLookAt(position, position + direction, up);
        const GfMatrix4d lightTransform = lightView.GetInverse();

        auto shadowComputation =
            std::make_shared<DuPointShadowMatrix>(position, sceneBounds);

        HdxShadowParams shadowParams;
        shadowParams.enabled = shadowEnabled;
        shadowParams.bias = -0.0005;
        shadowParams.blur = 0.0;
        shadowParams.resolution = 2048;
        shadowParams.shadowMatrix = shadowComputation;

        GlfSimpleLight glfLight;
        glfLight.SetTransform(lightTransform);
        glfLight.SetPosition(GfVec4f(position[0], position[1], position[2], 1.0f));
        glfLight.SetAmbient(GfVec4f(0.0f));
        glfLight.SetDiffuse(GfVec4f(
            color[0] * power * diffuse,
            color[1] * power * diffuse,
            color[2] * power * diffuse,
            1.0f));
        glfLight.SetSpecular(GfVec4f(
            color[0] * power * specular,
            color[1] * power * specular,
            color[2] * power * specular,
            1.0f));
        glfLight.SetAttenuation(GfVec3f(0.0f, 0.0f, 1.0f));
        glfLight.SetHasIntensity(power > 0.0f);
        glfLight.SetHasShadow(shadowEnabled);

        std::unordered_map<TfToken, VtValue, TfToken::HashFunctor> params;
        params[HdLightTokens->params] = VtValue(glfLight);
        params[HdLightTokens->color] = VtValue(color);
        params[HdLightTokens->intensity] = VtValue(intensity);
        params[HdLightTokens->exposure] = VtValue(exposure);
        params[HdLightTokens->diffuse] = VtValue(diffuse);
        params[HdLightTokens->specular] = VtValue(specular);
        params[HdLightTokens->radius] = VtValue(radius);
        params[HdLightTokens->ambient] = VtValue(0.0f);
        params[HdLightTokens->normalize] = VtValue(treatAsPoint);
        params[HdLightTokens->hasShadow] = VtValue(shadowEnabled);
        params[HdLightTokens->shadowEnable] = VtValue(shadowEnabled);
        params[HdLightTokens->shadowParams] = VtValue(shadowParams);
        params[HdLightTokens->shadowCollection] = VtValue(shadowCollection);

        if (log) {
            log << "[DreamUSD][SceneLightMirror] path=" << prim.GetPath().GetString()
                << " pos=(" << position[0] << ", " << position[1] << ", " << position[2] << ")"
                << " power=" << power
                << " radius=" << radius
                << " point=" << (treatAsPoint ? "true" : "false")
                << " shadow=" << (shadowEnabled ? "true" : "false")
                << std::endl;
        }

        result.push_back(DuViewerLightState{
            HdPrimTypeTokens->simpleLight,
            lightTransform,
            std::move(params),
            shadowParams,
            shadowCollection,
            true,
        });
    }

    return result;
}

static GlfSimpleLightVector _DuExtractMirroredLights(
    std::vector<DuViewerLightState> const& viewerLights) {
    GlfSimpleLightVector result;
    result.reserve(viewerLights.size());

    for (DuViewerLightState const& state : viewerLights) {
        const auto paramsIt = state.params.find(HdLightTokens->params);
        if (paramsIt == state.params.end()) {
            continue;
        }

        result.push_back(
            paramsIt->second.GetWithDefault<GlfSimpleLight>(GlfSimpleLight()));
    }

    return result;
}

static void _DuLogShadowDebug(
    UsdStageRefPtr const& stage,
    std::vector<DuViewerLightState> const& viewerLights,
    HdRprimCollection const& shadowCollection) {
    std::ofstream log("/tmp/dreamusd-shadow-debug.log", std::ios::app);
    if (!log) {
        return;
    }

    log << "[DreamUSD][ShadowDebug] sceneLights="
        << (_DuStageHasSceneLights(stage, true) ? "true" : "false")
        << " nonDefaultSceneLights="
        << (_DuStageHasSceneLights(stage, false) ? "true" : "false")
        << " viewerLights=" << viewerLights.size()
        << " shadowName=" << shadowCollection.GetName().GetString()
        << std::endl;

    const GfRange3d bounds = _DuComputeSceneBounds(stage);
    log << "[DreamUSD][ShadowDebug] boundsMin=("
        << bounds.GetMin()[0] << ", "
        << bounds.GetMin()[1] << ", "
        << bounds.GetMin()[2] << ") boundsMax=("
        << bounds.GetMax()[0] << ", "
        << bounds.GetMax()[1] << ", "
        << bounds.GetMax()[2] << ")"
        << std::endl;

    for (size_t i = 0; i < viewerLights.size(); ++i) {
        const DuViewerLightState& state = viewerLights[i];
        const auto paramsIt = state.params.find(HdLightTokens->params);
        const GlfSimpleLight light =
            paramsIt != state.params.end()
                ? paramsIt->second.GetWithDefault<GlfSimpleLight>(GlfSimpleLight())
                : GlfSimpleLight();
        const GfVec4f pos = light.GetPosition();
        log << "[DreamUSD][ShadowDebug] light[" << i << "] "
            << "dir=(" << pos[0] << ", " << pos[1] << ", " << pos[2] << ", " << pos[3] << ") "
            << "hasIntensity=" << (light.HasIntensity() ? "true" : "false") << " "
            << "hasShadow=" << (light.HasShadow() ? "true" : "false") << " "
            << "shadowEnabled=" << (state.shadowParams.enabled ? "true" : "false") << " "
            << "shadowRes=" << state.shadowParams.resolution << " "
            << "lightType=" << state.lightType.GetString()
            << std::endl;
        if (state.shadowParams.shadowMatrix) {
            const std::vector<GfMatrix4d> computed =
                state.shadowParams.shadowMatrix->Compute(
                    GfVec4f(0.0f, 0.0f, 1024.0f, 1024.0f),
                    CameraUtilFit);
            if (!computed.empty()) {
                const GfMatrix4d& m = computed.front();
                log << "[DreamUSD][ShadowDebug] light[" << i << "] shadowMtxRow0=("
                    << m[0][0] << ", " << m[0][1] << ", " << m[0][2] << ", " << m[0][3]
                    << ") row1=("
                    << m[1][0] << ", " << m[1][1] << ", " << m[1][2] << ", " << m[1][3]
                    << ") row2=("
                    << m[2][0] << ", " << m[2][1] << ", " << m[2][2] << ", " << m[2][3]
                    << ")" << std::endl;
            }
        }
    }
}

static void _DuLogLightingMode(
    bool useFallbackLights,
    bool useSceneLights,
    std::vector<DuViewerLightState> const& viewerLights,
    bool hasViewerLightDelegate,
    bool hasRenderIndex) {
    std::ofstream log("/tmp/dreamusd-shadow-debug.log", std::ios::app);
    if (!log) {
        return;
    }

    log << "[DreamUSD][LightingMode] useFallbackLights="
        << (useFallbackLights ? "true" : "false")
        << " useSceneLights=" << (useSceneLights ? "true" : "false")
        << " viewerLights=" << viewerLights.size()
        << " delegate=" << (hasViewerLightDelegate ? "true" : "false")
        << " renderIndex=" << (hasRenderIndex ? "true" : "false")
        << std::endl;
}

static void _DuEnsureViewerLightDelegate(DuHydraEngine* engine) {
    if (!engine || engine->viewerLightDelegate || !engine->glEngine) {
        return;
    }

    HdRenderIndex* renderIndex = engine->glEngine->_GetRenderIndex();
    if (renderIndex) {
        engine->viewerLightDelegate = std::make_unique<DuViewerLightDelegate>(
            renderIndex,
            SdfPath("/viewerLights"));
    }

    std::ofstream log("/tmp/dreamusd-shadow-debug.log", std::ios::app);
    if (!log) {
        return;
    }

    log << "[DreamUSD][ViewerLightDelegate] created="
        << (engine->viewerLightDelegate ? "true" : "false")
        << " renderIndex=" << (renderIndex ? "true" : "false")
        << " taskController=" << (engine->glEngine->_GetTaskController() ? "true" : "false")
        << " useSceneIndex=" << (UsdImagingGLEngine::UseUsdImagingSceneIndex() ? "true" : "false")
        << std::endl;
}

static void _DuLogRenderTasks(HdTaskSharedPtrVector const& tasks) {
    std::ofstream log("/tmp/dreamusd-shadow-debug.log", std::ios::app);
    if (!log) {
        return;
    }

    log << "[DreamUSD][ShadowDebug] renderTasks=" << tasks.size();
    for (HdTaskSharedPtr const& task : tasks) {
        if (task) {
            log << " " << task->GetId().GetString();
        }
    }
    log << std::endl;
}

static void _DuLogShadowAovs(DuHydraEngine* engine) {
    std::ofstream log("/tmp/dreamusd-shadow-debug.log", std::ios::app);
    if (!log || !engine || !engine->taskController) {
        return;
    }

    HdRenderBuffer* colorBuffer = engine->taskController->GetRenderOutput(HdAovTokens->color);
    HdRenderBuffer* depthBuffer = engine->taskController->GetRenderOutput(HdAovTokens->depth);
    log << "[DreamUSD][ShadowDebug] colorAov="
        << (colorBuffer ? "present" : "missing")
        << " depthAov=" << (depthBuffer ? "present" : "missing");
    if (depthBuffer) {
        log << " depthSize=" << depthBuffer->GetWidth() << "x" << depthBuffer->GetHeight()
            << " depthFormat=" << depthBuffer->GetFormat();
    }
    log << std::endl;
}

static void _DuLogSceneIndexShadowState(DuHydraEngine* engine) {
    std::ofstream log("/tmp/dreamusd-shadow-debug.log", std::ios::app);
    if (!log || !engine || !engine->glEngine) {
        return;
    }

    HdxTaskControllerSceneIndexRefPtr const& sceneIndex =
        engine->glEngine->_taskControllerSceneIndex;
    log << "[DreamUSD][SceneIndexShadow] present="
        << (sceneIndex ? "true" : "false");

    if (!sceneIndex) {
        log << std::endl;
        return;
    }

    const SdfPathVector taskPaths = sceneIndex->GetRenderingTaskPaths();
    log << " taskCount=" << taskPaths.size();
    for (SdfPath const& path : taskPaths) {
        log << " " << path.GetString();
    }

    const SdfPath colorPath = sceneIndex->GetRenderBufferPath(HdAovTokens->color);
    const SdfPath depthPath = sceneIndex->GetRenderBufferPath(HdAovTokens->depth);
    log << " colorBuffer=" << colorPath.GetString()
        << " depthBuffer=" << depthPath.GetString()
        << std::endl;
}

static void _DuEnsureSceneLightShadows(
    UsdStageRefPtr const& stage,
    bool includeHistoricalDefaultLights) {
    if (!stage) {
        return;
    }

    SdfLayerRefPtr sessionLayer = stage->GetSessionLayer();
    if (!sessionLayer) {
        return;
    }

    UsdEditContext editContext(stage, UsdEditTarget(sessionLayer));

    for (UsdPrim prim : stage->Traverse()) {
        if (!UsdLuxLightAPI(prim)) {
            continue;
        }
        if (!includeHistoricalDefaultLights && _DuIsHistoricalDefaultLight(prim)) {
            continue;
        }

        UsdLuxShadowAPI shadow = prim.HasAPI<UsdLuxShadowAPI>()
            ? UsdLuxShadowAPI(prim)
            : UsdLuxShadowAPI::Apply(prim);
        if (!shadow) {
            continue;
        }

        UsdAttribute attr = shadow.GetShadowEnableAttr();
        if (!attr || !attr.HasAuthoredValueOpinion()) {
            shadow.CreateShadowEnableAttr(VtValue(true));
        }

        // Storm's legacy UsdImagingDelegate path still queries "hasShadow"
        // directly when building GlfSimpleLight state for scene lights.
        UsdAttribute legacyHasShadow = prim.GetAttribute(TfToken("hasShadow"));
        if (!legacyHasShadow) {
            legacyHasShadow = prim.CreateAttribute(
                TfToken("hasShadow"),
                SdfValueTypeNames->Bool,
                /* custom = */ true);
        }
        if (legacyHasShadow && !legacyHasShadow.HasAuthoredValueOpinion()) {
            legacyHasShadow.Set(true);
        }
    }
}

static void _DuApplyMirroredSceneLightOverrides(
    DuHydraEngine* engine,
    std::vector<DuViewerLightState> const& viewerLights) {
    if (!engine || !engine->glEngine || !engine->glEngine->_taskControllerSceneIndex) {
        return;
    }

    _DuRestoreMirroredSceneLightOverrides(engine);

    HdxTaskControllerSceneIndexRefPtr const& sceneIndex =
        engine->glEngine->_taskControllerSceneIndex;
    HdRetainedSceneIndexRefPtr const& retained = sceneIndex->_retainedSceneIndex;
    if (!retained) {
        return;
    }

    const SdfPath lightScope =
        sceneIndex->_params.prefix.AppendChild(TfToken("lights"));

    HdSceneIndexObserver::RemovedPrimEntries removed;
    HdRetainedSceneIndex::AddedPrimEntries added;
    removed.reserve(viewerLights.size());
    added.reserve(viewerLights.size());
    engine->sceneIndexLightOverrides.clear();
    engine->sceneIndexLightOverrides.reserve(viewerLights.size());

    for (size_t i = 0; i < viewerLights.size(); ++i) {
        const SdfPath lightPath = lightScope.AppendChild(
            TfToken(TfStringPrintf("light_%zu", i)));
        const HdSceneIndexPrim prim = retained->GetPrim(lightPath);
        if (!prim.dataSource) {
            continue;
        }

        engine->sceneIndexLightOverrides.push_back({
            lightPath,
            prim.primType,
            prim.dataSource,
        });
        removed.push_back({lightPath});
        added.push_back({
            lightPath,
            prim.primType,
            std::make_shared<DuAugmentedLightPrimDataSource>(
                prim.dataSource,
                viewerLights[i].shadowParams,
                viewerLights[i].shadowCollection),
        });
    }

    if (!removed.empty()) {
        retained->RemovePrims(removed);
    }
    if (!added.empty()) {
        retained->AddPrims(added);
    }
}

static void _DuRestoreMirroredSceneLightOverrides(DuHydraEngine* engine) {
    if (!engine || engine->sceneIndexLightOverrides.empty() ||
        !engine->glEngine || !engine->glEngine->_taskControllerSceneIndex) {
        return;
    }

    HdxTaskControllerSceneIndexRefPtr const& sceneIndex =
        engine->glEngine->_taskControllerSceneIndex;
    HdRetainedSceneIndexRefPtr const& retained = sceneIndex->_retainedSceneIndex;
    if (!retained) {
        engine->sceneIndexLightOverrides.clear();
        return;
    }

    HdSceneIndexObserver::RemovedPrimEntries removed;
    HdRetainedSceneIndex::AddedPrimEntries added;
    removed.reserve(engine->sceneIndexLightOverrides.size());
    added.reserve(engine->sceneIndexLightOverrides.size());

    for (DuSceneIndexLightOverrideEntry const& entry :
         engine->sceneIndexLightOverrides) {
        removed.push_back({entry.primPath});
        added.push_back({entry.primPath, entry.primType, entry.originalDataSource});
    }

    if (!removed.empty()) {
        retained->RemovePrims(removed);
    }
    if (!added.empty()) {
        retained->AddPrims(added);
    }

    engine->sceneIndexLightOverrides.clear();
}

static void _DuSyncSceneIndexMirroredLights(
    DuHydraEngine* engine,
    std::vector<DuViewerLightState> const& viewerLights) {
    if (!engine || !engine->glEngine || !engine->glEngine->_taskControllerSceneIndex) {
        return;
    }

    HdxTaskControllerSceneIndexRefPtr const& sceneIndex =
        engine->glEngine->_taskControllerSceneIndex;
    HdRetainedSceneIndexRefPtr const& retained = sceneIndex->_retainedSceneIndex;
    if (!retained) {
        return;
    }

    if (!engine->sceneIndexMirroredLightPaths.empty()) {
        HdSceneIndexObserver::RemovedPrimEntries removed;
        removed.reserve(engine->sceneIndexMirroredLightPaths.size());
        for (SdfPath const& path : engine->sceneIndexMirroredLightPaths) {
            removed.push_back({path});
        }
        retained->RemovePrims(removed);
        engine->sceneIndexMirroredLightPaths.clear();
    }

    if (viewerLights.empty()) {
        return;
    }

    HdRetainedSceneIndex::AddedPrimEntries added;
    added.reserve(viewerLights.size());
    for (size_t i = 0; i < viewerLights.size(); ++i) {
        const SdfPath path = sceneIndex->_params.prefix.AppendChild(
            TfToken(TfStringPrintf("dreamusdLight_%zu", i)));
        added.push_back({
            path,
            viewerLights[i].lightType,
            std::make_shared<DuSceneIndexLightPrimDataSource>(viewerLights[i]),
        });
        engine->sceneIndexMirroredLightPaths.push_back(path);
    }
    retained->AddPrims(added);
}

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

        eng->hgiDriver.name = HgiTokens->renderDriver;
        eng->hgiDriver.driver = VtValue(eng->hgi.get());
        UsdImagingGLEngine::Parameters params;
        params.rootPath = SdfPath::AbsoluteRootPath();
        params.sceneDelegateID = SdfPath::AbsoluteRootPath();
        params.driver = eng->hgiDriver;
        params.rendererPluginId = TfToken("HdStormRendererPlugin");
        params.gpuEnabled = true;
        params.allowAsynchronousSceneProcessing = true;
        _DuRegisterSceneLightShadowSceneIndex();
        DuScopedSceneIndexAugmentStage sceneIndexStage(stagePtr);
        eng->glEngine = std::make_unique<UsdImagingGLEngine>(params);
        eng->glEngine->SetEnablePresentation(false);
        eng->glEngine->SetRendererAovs({eng->currentAov});
        _DuConfigureColorOutput(eng);
        eng->glEngine->SetSelectionColor(GfVec4f(1.0f, 1.0f, 0.0f, 0.5f));
        _DuEnsureViewerLightDelegate(eng);
        eng->currentRdId = eng->glEngine->GetCurrentRendererId();
        _DuSyncRenderParams(eng);
        _DuApplyShadowSetting(eng);

        *out = eng;
        return DU_OK;
    });

    return DU_ERR_USD;
}

DuStatus du_hydra_render(DuHydraEngine* engine, uint32_t width, uint32_t height) {
    DU_CHECK_NULL(engine);

    DU_TRY({
        if (!engine->glEngine) {
            du_set_last_error("Hydra engine is not initialized");
            return DU_ERR_INVALID;
        }

        engine->width = width;
        engine->height = height;
        _DuSyncRenderParams(engine);

        GfMatrix4d viewMatrix(1.0);
        GfMatrix4d projectionMatrix(1.0);
        _DuBuildCameraMatrices(engine, width, height, &viewMatrix, &projectionMatrix);
        _DuEnsureViewerLightDelegate(engine);
        _DuRestoreMirroredSceneLightOverrides(engine);

        const bool hasSceneLights = _DuStageHasSceneLights(engine->stage, true);
        const bool useSceneLights = engine->renderParams.enableLighting && hasSceneLights;
        const bool useFallbackLights = engine->renderParams.enableLighting && !hasSceneLights;
        std::vector<DuViewerLightState> viewerLights;

        if (engine->viewerLightDelegate) {
            engine->viewerLightDelegate->SyncLights({});
        }
        _DuSyncSceneIndexMirroredLights(engine, {});
        _DuLogLightingMode(
            useFallbackLights,
            useSceneLights,
            viewerLights,
            static_cast<bool>(engine->viewerLightDelegate),
            engine->glEngine && engine->glEngine->_GetRenderIndex());

        engine->glEngine->SetRenderBufferSize(GfVec2i(width, height));
        engine->glEngine->SetRenderViewport(GfVec4d(0.0, 0.0, width, height));
        engine->glEngine->SetCameraState(viewMatrix, projectionMatrix);
        _DuConfigureColorOutput(engine);
        _DuConfigureViewerLighting(
            engine,
            viewMatrix,
            projectionMatrix,
            useSceneLights);
        _DuApplyShadowSetting(engine);
        _DuLogSceneIndexShadowState(engine);
        engine->glEngine->Render(engine->stage->GetPseudoRoot(), engine->renderParams);
        _DuRestoreMirroredSceneLightOverrides(engine);

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

    // Read back from the active renderer AOV.
    if (!engine->glEngine) {
        du_set_last_error("Hydra engine is not initialized");
        return DU_ERR_INVALID;
    }

    HdRenderBuffer* colorBuffer = engine->glEngine->GetAovRenderBuffer(engine->currentAov);
    if (!colorBuffer) {
        du_set_last_error(
            TfStringPrintf("AOV render buffer not available: %s",
                           engine->currentAov.GetText()).c_str());
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

    const bool isColorAov = engine->currentAov == HdAovTokens->color;
    const auto pack_vector = [&](float x, float y, float z, float a) {
        if (isColorAov) {
            return std::array<uint8_t, 4>{
                _DuTonemapLinearToSrgb8(x),
                _DuTonemapLinearToSrgb8(y),
                _DuTonemapLinearToSrgb8(z),
                _DuLinearToUnorm8(a),
            };
        }
        return std::array<uint8_t, 4>{
            _DuLinearToUnorm8(x * 0.5f + 0.5f),
            _DuLinearToUnorm8(y * 0.5f + 0.5f),
            _DuLinearToUnorm8(z * 0.5f + 0.5f),
            _DuLinearToUnorm8(a),
        };
    };

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
        const uint16_t* src = (const uint16_t*)data;
        for (size_t pixel = 0; pixel < (size_t)bufW * bufH; pixel++) {
            float channels[4];
            for (size_t channel = 0; channel < 4; channel++) {
                uint16_t half = src[pixel * 4 + channel];
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
                channels[channel] = sign ? -f : f;
            }

            const auto packed =
                pack_vector(channels[0], channels[1], channels[2], channels[3]);
            memcpy(engine->framebuffer.data() + pixel * 4, packed.data(), 4);
        }
    } else if (format == HdFormatFloat16Vec3) {
        const uint16_t* src = (const uint16_t*)data;
        for (size_t pixel = 0; pixel < (size_t)bufW * bufH; pixel++) {
            float channels[3];
            for (size_t channel = 0; channel < 3; channel++) {
                uint16_t half = src[pixel * 3 + channel];
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
                channels[channel] = sign ? -f : f;
            }
            const auto packed = pack_vector(channels[0], channels[1], channels[2], 1.0f);
            memcpy(engine->framebuffer.data() + pixel * 4, packed.data(), 4);
        }
    } else if (format == HdFormatFloat32Vec4) {
        const float* src = (const float*)data;
        for (size_t pixel = 0; pixel < (size_t)bufW * bufH; pixel++) {
            const auto packed = pack_vector(
                src[pixel * 4 + 0],
                src[pixel * 4 + 1],
                src[pixel * 4 + 2],
                src[pixel * 4 + 3]);
            memcpy(engine->framebuffer.data() + pixel * 4, packed.data(), 4);
        }
    } else if (format == HdFormatFloat32Vec3) {
        const float* src = (const float*)data;
        for (size_t pixel = 0; pixel < (size_t)bufW * bufH; pixel++) {
            const auto packed = pack_vector(
                src[pixel * 3 + 0],
                src[pixel * 3 + 1],
                src[pixel * 3 + 2],
                1.0f);
            memcpy(engine->framebuffer.data() + pixel * 4, packed.data(), 4);
        }
    } else if (format == HdFormatFloat32) {
        const float* src = static_cast<const float*>(data);
        float minValue = std::numeric_limits<float>::infinity();
        float maxValue = -std::numeric_limits<float>::infinity();
        for (size_t pixel = 0; pixel < (size_t)bufW * bufH; pixel++) {
            const float value = src[pixel];
            if (std::isfinite(value)) {
                minValue = std::min(minValue, value);
                maxValue = std::max(maxValue, value);
            }
        }
        if (!std::isfinite(minValue) || !std::isfinite(maxValue) || minValue == maxValue) {
            minValue = 0.0f;
            maxValue = 1.0f;
        }
        const float scale = 1.0f / (maxValue - minValue);
        for (size_t pixel = 0; pixel < (size_t)bufW * bufH; pixel++) {
            float value = src[pixel];
            value = std::isfinite(value) ? std::clamp((value - minValue) * scale, 0.0f, 1.0f) : 0.0f;
            const uint8_t byte = _DuLinearToUnorm8(1.0f - value);
            engine->framebuffer[pixel * 4 + 0] = byte;
            engine->framebuffer[pixel * 4 + 1] = byte;
            engine->framebuffer[pixel * 4 + 2] = byte;
            engine->framebuffer[pixel * 4 + 3] = 255;
        }
    } else if (format == HdFormatInt32) {
        for (size_t pixel = 0; pixel < (size_t)bufW * bufH; pixel++) {
            uint32_t id =
                static_cast<uint32_t>(static_cast<const int32_t*>(data)[pixel]);
            const uint8_t r = static_cast<uint8_t>((id * 1664525u + 1013904223u) & 0xFFu);
            const uint8_t g = static_cast<uint8_t>(((id >> 8) * 22695477u + 1u) & 0xFFu);
            const uint8_t b = static_cast<uint8_t>(((id >> 16) * 1103515245u + 12345u) & 0xFFu);
            engine->framebuffer[pixel * 4 + 0] = r;
            engine->framebuffer[pixel * 4 + 1] = g;
            engine->framebuffer[pixel * 4 + 2] = b;
            engine->framebuffer[pixel * 4 + 3] = id == 0 ? 0 : 255;
        }
    } else {
        colorBuffer->Unmap();
        du_set_last_error(
            TfStringPrintf(
                "Unsupported AOV format for CPU readback (%s): %d",
                engine->currentAov.GetText(),
                static_cast<int>(format))
                .c_str());
        return DU_ERR_INVALID;
    }

    colorBuffer->Unmap();

    *rgba = engine->framebuffer.data();
    *width = bufW;
    *height = bufH;
    return DU_OK;
}

DuStatus du_hydra_get_native_texture(
    DuHydraEngine* engine,
    void* texture,
    uint32_t* width,
    uint32_t* height)
{
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(texture);

    if (!engine->glEngine) {
        du_set_last_error("Hydra engine is not initialized");
        return DU_ERR_INVALID;
    }

    HgiTextureHandle textureHandle = engine->glEngine->GetAovTexture(engine->currentAov);
    if (!textureHandle) {
        du_set_last_error(
            TfStringPrintf("AOV native texture handle is null: %s",
                           engine->currentAov.GetText()).c_str());
        return DU_ERR_INVALID;
    }

    uint64_t rawTexture = textureHandle->GetRawResource();
    if (rawTexture == 0) {
        du_set_last_error(
            TfStringPrintf("AOV native texture resource is null: %s",
                           engine->currentAov.GetText()).c_str());
        return DU_ERR_INVALID;
    }

    *(uint64_t*)texture = rawTexture;
    if (width) *width = engine->width;
    if (height) *height = engine->height;
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

DuStatus du_hydra_set_camera_lens(
    DuHydraEngine* engine,
    double fov_y_radians,
    double near_plane,
    double far_plane)
{
    DU_CHECK_NULL(engine);

    engine->fovYRadians = std::clamp(fov_y_radians, 0.01, M_PI - 0.01);
    engine->nearPlane = std::max(1e-5, near_plane);
    engine->farPlane = std::max(engine->nearPlane + 1e-4, far_plane);
    return DU_OK;
}

DuStatus du_hydra_compute_auto_clip(
    DuHydraEngine* engine,
    double* near_plane,
    double* far_plane)
{
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(near_plane);
    DU_CHECK_NULL(far_plane);

    DU_TRY({
        _DuComputeAutoClipRange(engine, near_plane, far_plane);
        return DU_OK;
    });

    return DU_ERR_USD;
}

DuStatus du_hydra_set_display_mode(DuHydraEngine* engine, DuDisplayMode mode) {
    DU_CHECK_NULL(engine);

    engine->displayMode = mode;
    engine->enableLighting =
        mode != DU_DISPLAY_WIREFRAME && mode != DU_DISPLAY_POINTS;
    _DuSyncRenderParams(engine);

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
    _DuApplyShadowSetting(engine);
    return DU_OK;
}

DuStatus du_hydra_set_msaa(DuHydraEngine* engine, bool enable) {
    DU_CHECK_NULL(engine);
    engine->msaaEnabled = enable;
    _DuSyncRenderParams(engine);
    return DU_OK;
}

DuStatus du_hydra_set_complexity(DuHydraEngine* engine, float complexity) {
    DU_CHECK_NULL(engine);
    engine->complexity = std::clamp(complexity, 1.0f, 1.3f);
    _DuSyncRenderParams(engine);
    return DU_OK;
}

DuStatus du_hydra_set_show_guides(DuHydraEngine* engine, bool enable) {
    DU_CHECK_NULL(engine);
    engine->showGuides = enable;
    _DuSyncRenderParams(engine);
    return DU_OK;
}

DuStatus du_hydra_set_show_proxy(DuHydraEngine* engine, bool enable) {
    DU_CHECK_NULL(engine);
    engine->showProxy = enable;
    _DuSyncRenderParams(engine);
    return DU_OK;
}

DuStatus du_hydra_set_show_render(DuHydraEngine* engine, bool enable) {
    DU_CHECK_NULL(engine);
    engine->showRender = enable;
    _DuSyncRenderParams(engine);
    return DU_OK;
}

DuStatus du_hydra_set_cull_backfaces(DuHydraEngine* engine, bool enable) {
    DU_CHECK_NULL(engine);
    engine->cullBackfaces = enable;
    _DuSyncRenderParams(engine);
    return DU_OK;
}

DuStatus du_hydra_set_enable_scene_materials(DuHydraEngine* engine, bool enable) {
    DU_CHECK_NULL(engine);
    engine->enableSceneMaterials = enable;
    _DuSyncRenderParams(engine);
    return DU_OK;
}

DuStatus du_hydra_set_dome_light_camera_visibility(
    DuHydraEngine* engine,
    bool enable)
{
    DU_CHECK_NULL(engine);
    engine->domeLightCameraVisibility = enable;
    _DuSyncRenderParams(engine);
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

    GfMatrix4d viewMatrix(1.0);
    GfMatrix4d projMatrix(1.0);
    _DuBuildCameraMatrices(engine, viewport_w, viewport_h, &viewMatrix, &projMatrix);

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

DuStatus du_hydra_pick(
    DuHydraEngine* engine,
    double screen_x,
    double screen_y,
    uint32_t viewport_w,
    uint32_t viewport_h,
    const char** out_path)
{
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(out_path);

    if (viewport_w == 0 || viewport_h == 0) {
        du_set_last_error("Viewport size must be non-zero");
        return DU_ERR_INVALID;
    }

    if (screen_x < 0.0 || screen_y < 0.0
        || screen_x > static_cast<double>(viewport_w)
        || screen_y > static_cast<double>(viewport_h)) {
        du_set_last_error("Pick position is outside the viewport");
        return DU_ERR_INVALID;
    }

    DU_TRY({
        if (!engine->glEngine) {
            du_set_last_error("Hydra engine is not initialized");
            return DU_ERR_INVALID;
        }

        GfMatrix4d viewMatrix(1.0);
        GfMatrix4d projectionMatrix(1.0);
        _DuBuildCameraMatrices(engine, viewport_w, viewport_h, &viewMatrix, &projectionMatrix);

        GfFrustum frustum;
        const double aspectRatio =
            static_cast<double>(viewport_w) / static_cast<double>(viewport_h);
        const double fovYDegrees = GfRadiansToDegrees(engine->fovYRadians);
        frustum.SetPerspective(
            fovYDegrees > 0.0 ? fovYDegrees : 60.0,
            aspectRatio,
            std::max(1e-5, engine->nearPlane),
            std::max(engine->nearPlane + 1e-4, engine->farPlane));
        frustum.SetPositionAndRotationFromMatrix(viewMatrix.GetInverse());

        const GfVec2d pickCenter(
            ((screen_x + 0.5) / static_cast<double>(viewport_w)) * 2.0 - 1.0,
            1.0 - ((screen_y + 0.5) / static_cast<double>(viewport_h)) * 2.0);
        const GfVec2d pickSize(
            std::max(8.0, 1.0) / static_cast<double>(viewport_w),
            std::max(8.0, 1.0) / static_cast<double>(viewport_h));
        const GfFrustum pickFrustum =
            frustum.ComputeNarrowedFrustum(pickCenter, pickSize);

        UsdImagingGLEngine::PickParams pickParams;
        pickParams.resolveMode = HdxPickTokens->resolveNearestToCenter;

        UsdImagingGLEngine::IntersectionResultVector results;
        UsdImagingGLRenderParams params = engine->renderParams;
        params.highlight = false;
        if (!engine->glEngine->TestIntersection(
                pickParams,
                pickFrustum.ComputeViewMatrix(),
                pickFrustum.ComputeProjectionMatrix(),
                engine->stage->GetPseudoRoot(),
                params,
                &results)
            || results.empty()) {
            du_set_last_error("Hydra pick did not hit any prim");
            return DU_ERR_INVALID;
        }

        SdfPath scenePath = results.front().hitPrimPath;
        if (scenePath.IsEmpty()) {
            du_set_last_error("Hydra pick resolved to an empty scene path");
            return DU_ERR_INVALID;
        }

        static thread_local std::string pickedPath;
        pickedPath = scenePath.GetString();
        *out_path = pickedPath.c_str();
        return DU_OK;
    });

    return DU_ERR_USD;
}

DuStatus du_hydra_set_selection(DuHydraEngine* engine, const char* selected_path) {
    const char* selection[] = {selected_path};
    return du_hydra_set_selection_paths(
        engine,
        selected_path ? selection : nullptr,
        selected_path ? 1u : 0u
    );
}

DuStatus du_hydra_set_selection_paths(
    DuHydraEngine* engine,
    const char* const* selected_paths,
    uint32_t count
) {
    DU_CHECK_NULL(engine);

    DU_TRY({
        if (!engine->glEngine) {
            du_set_last_error("Hydra engine is not initialized");
            return DU_ERR_INVALID;
        }

        SdfPathVector nextSelectedPaths;
        nextSelectedPaths.reserve(count);

        for (uint32_t i = 0; i < count; ++i) {
            const char* selected_path = selected_paths ? selected_paths[i] : nullptr;
            if (!selected_path || selected_path[0] == '\0') {
                continue;
            }

            const SdfPath usdPath(selected_path);
            if (!usdPath.IsAbsolutePath() || usdPath.IsPropertyPath()) {
                du_set_last_error("Selection path must be an absolute prim path");
                return DU_ERR_INVALID;
            }

            nextSelectedPaths.push_back(usdPath);
        }

        if (nextSelectedPaths == engine->selectedPaths) {
            return DU_OK;
        }

        engine->selectedPaths = std::move(nextSelectedPaths);

        if (!engine->selectedPaths.empty()) {
            engine->glEngine->SetSelected(engine->selectedPaths);
        } else {
            engine->glEngine->ClearSelected();
        }

        _DuSyncRenderParams(engine);
        return DU_OK;
    });

    return DU_ERR_USD;
}

void du_hydra_destroy(DuHydraEngine* engine) {
    if (!engine) return;

    engine->glEngine.reset();
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
    s_name = engine->glEngine
        ? engine->glEngine->GetCurrentRendererId().GetString()
        : engine->currentRdId.GetString();
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

        if (!engine->glEngine) {
            du_set_last_error("Hydra engine is not initialized");
            return DU_ERR_INVALID;
        }

        _DuRegisterSceneLightShadowSceneIndex();
        DuScopedSceneIndexAugmentStage sceneIndexStage(engine->stage);
        if (!engine->glEngine->SetRendererPlugin(newId)) {
            du_set_last_error(std::string("Failed to create render delegate: ") + name);
            return DU_ERR_INVALID;
        }

        engine->currentRdId = engine->glEngine->GetCurrentRendererId();
        engine->glEngine->SetEnablePresentation(false);
        engine->glEngine->SetRendererAovs({engine->currentAov});
        _DuConfigureColorOutput(engine);
        engine->glEngine->SetSelectionColor(GfVec4f(1.0f, 1.0f, 0.0f, 0.5f));
        engine->viewerLightDelegate.reset();
        _DuEnsureViewerLightDelegate(engine);
        if (!engine->selectedPaths.empty()) {
            engine->glEngine->SetSelected(engine->selectedPaths);
        } else {
            engine->glEngine->ClearSelected();
        }
        _DuApplyShadowSetting(engine);

        return DU_OK;
    });

    return DU_ERR_USD;
}

DuStatus du_rd_get_aovs(DuHydraEngine* engine, const char*** names, uint32_t* count) {
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(names);
    DU_CHECK_NULL(count);

    DU_TRY({
        if (!engine->glEngine) {
            du_set_last_error("Hydra engine is not initialized");
            return DU_ERR_INVALID;
        }

        const TfTokenVector aovs = engine->glEngine->GetRendererAovs();
        *count = static_cast<uint32_t>(aovs.size());
        if (aovs.empty()) {
            *names = nullptr;
            return DU_OK;
        }

        *names = static_cast<const char**>(malloc(sizeof(const char*) * aovs.size()));
        if (!*names) {
            du_set_last_error("Out of memory allocating AOV list");
            return DU_ERR_INVALID;
        }

        for (size_t i = 0; i < aovs.size(); ++i) {
            (*names)[i] = _DuDupString(aovs[i].GetString());
        }
        return DU_OK;
    });

    return DU_ERR_USD;
}

DuStatus du_rd_get_current_aov(DuHydraEngine* engine, const char** name) {
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(name);

    static thread_local std::string s_name;
    s_name = engine->currentAov.GetString();
    *name = s_name.c_str();
    return DU_OK;
}

DuStatus du_rd_set_current_aov(DuHydraEngine* engine, const char* name) {
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(name);

    DU_TRY({
        if (!engine->glEngine) {
            du_set_last_error("Hydra engine is not initialized");
            return DU_ERR_INVALID;
        }

        TfToken const newAov(name);
        if (!engine->glEngine->SetRendererAov(newAov)) {
            du_set_last_error(std::string("Failed to set renderer AOV: ") + name);
            return DU_ERR_INVALID;
        }

        engine->currentAov = newAov;
        _DuConfigureColorOutput(engine);
        return DU_OK;
    });

    return DU_ERR_USD;
}

DuStatus du_rd_get_settings(
    DuHydraEngine* engine,
    DuRendererSetting** settings,
    uint32_t* count)
{
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(settings);
    DU_CHECK_NULL(count);

    DU_TRY({
        if (!engine->glEngine) {
            du_set_last_error("Hydra engine is not initialized");
            return DU_ERR_INVALID;
        }

        const UsdImagingGLRendererSettingsList descriptors =
            engine->glEngine->GetRendererSettingsList();
        *count = static_cast<uint32_t>(descriptors.size());
        if (descriptors.empty()) {
            *settings = nullptr;
            return DU_OK;
        }

        *settings = static_cast<DuRendererSetting*>(
            calloc(descriptors.size(), sizeof(DuRendererSetting)));
        if (!*settings) {
            du_set_last_error("Out of memory allocating renderer settings");
            return DU_ERR_INVALID;
        }

        for (size_t i = 0; i < descriptors.size(); ++i) {
            const auto& desc = descriptors[i];
            const VtValue currentValue = engine->glEngine->GetRendererSetting(desc.key);
            (*settings)[i].key = _DuDupString(desc.key.GetString());
            (*settings)[i].name = _DuDupString(desc.name);
            (*settings)[i].type = _DuToRendererSettingType(desc.type);
            (*settings)[i].current_value =
                _DuDupString(_DuRendererSettingValueToString(currentValue));
            (*settings)[i].default_value =
                _DuDupString(_DuRendererSettingValueToString(desc.defValue));
        }
        return DU_OK;
    });

    return DU_ERR_USD;
}

DuStatus du_rd_set_setting_bool(DuHydraEngine* engine, const char* key, bool value) {
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(key);
    DU_TRY({
        if (!engine->glEngine) {
            du_set_last_error("Hydra engine is not initialized");
            return DU_ERR_INVALID;
        }
        engine->glEngine->SetRendererSetting(TfToken(key), VtValue(value));
        return DU_OK;
    });
    return DU_ERR_USD;
}

DuStatus du_rd_set_setting_int(DuHydraEngine* engine, const char* key, int value) {
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(key);
    DU_TRY({
        if (!engine->glEngine) {
            du_set_last_error("Hydra engine is not initialized");
            return DU_ERR_INVALID;
        }
        engine->glEngine->SetRendererSetting(TfToken(key), VtValue(value));
        return DU_OK;
    });
    return DU_ERR_USD;
}

DuStatus du_rd_set_setting_float(DuHydraEngine* engine, const char* key, float value) {
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(key);
    DU_TRY({
        if (!engine->glEngine) {
            du_set_last_error("Hydra engine is not initialized");
            return DU_ERR_INVALID;
        }
        engine->glEngine->SetRendererSetting(TfToken(key), VtValue(value));
        return DU_OK;
    });
    return DU_ERR_USD;
}

DuStatus du_rd_set_setting_string(
    DuHydraEngine* engine,
    const char* key,
    const char* value)
{
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(key);
    DU_CHECK_NULL(value);
    DU_TRY({
        if (!engine->glEngine) {
            du_set_last_error("Hydra engine is not initialized");
            return DU_ERR_INVALID;
        }
        engine->glEngine->SetRendererSetting(TfToken(key), VtValue(std::string(value)));
        return DU_OK;
    });
    return DU_ERR_USD;
}

DuStatus du_hydra_poll_async_updates(DuHydraEngine* engine, bool* changed) {
    DU_CHECK_NULL(engine);
    DU_CHECK_NULL(changed);

    DU_TRY({
        if (!engine->glEngine) {
            du_set_last_error("Hydra engine is not initialized");
            return DU_ERR_INVALID;
        }

        *changed = engine->glEngine->PollForAsynchronousUpdates();
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

DuStatus du_hydra_get_native_texture(DuHydraEngine*, void*, uint32_t*, uint32_t*) {
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

DuStatus du_hydra_set_camera_lens(DuHydraEngine*, double, double, double) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_compute_auto_clip(DuHydraEngine*, double*, double*) {
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

DuStatus du_hydra_set_msaa(DuHydraEngine*, bool) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_set_complexity(DuHydraEngine*, float) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_set_show_guides(DuHydraEngine*, bool) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_set_show_proxy(DuHydraEngine*, bool) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_set_show_render(DuHydraEngine*, bool) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_set_cull_backfaces(DuHydraEngine*, bool) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_set_enable_scene_materials(DuHydraEngine*, bool) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_set_dome_light_camera_visibility(DuHydraEngine*, bool) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_project_point(DuHydraEngine*, double[3], uint32_t, uint32_t, double[2]) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_pick(DuHydraEngine*, double, double, uint32_t, uint32_t, const char**) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_set_selection(DuHydraEngine*, const char*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_set_selection_paths(DuHydraEngine*, const char* const*, uint32_t) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_hydra_poll_async_updates(DuHydraEngine*, bool*) {
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

DuStatus du_rd_get_aovs(DuHydraEngine*, const char***, uint32_t*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_rd_get_current_aov(DuHydraEngine*, const char**) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_rd_set_current_aov(DuHydraEngine*, const char*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_rd_get_settings(DuHydraEngine*, DuRendererSetting**, uint32_t*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_rd_set_setting_bool(DuHydraEngine*, const char*, bool) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_rd_set_setting_int(DuHydraEngine*, const char*, int) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_rd_set_setting_float(DuHydraEngine*, const char*, float) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

DuStatus du_rd_set_setting_string(DuHydraEngine*, const char*, const char*) {
    du_set_last_error("USD not available (stub build)");
    return DU_ERR_INVALID;
}

} // extern "C"

#endif // HAS_USD
