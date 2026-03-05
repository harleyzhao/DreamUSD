# DreamUSD Viewer - Design Document

## Overview

A high-performance USD file viewer and editor built with Rust, using Hydra for rendering via a C ABI bridge to OpenUSD's C++ libraries. The application uses egui for UI and wgpu (Vulkan) for display, with GPU texture sharing for zero-copy frame transfer from Hydra Storm.

## Goals

- Open, display, edit, and save USD files
- Display textures and shaders (UsdPreviewSurface, MaterialX)
- Pluggable render delegate architecture
- Switch between real-time display modes (smooth shaded, wireframe, textured, etc.)
- Cross-platform: macOS (MoltenVK), Linux, Windows
- Open source product quality

## Architecture: Rust App + C ABI Bridge to OpenUSD/Hydra

Single-process architecture. Rust controls the application lifecycle, UI, and display. C++ bridge layer wraps OpenUSD and Hydra APIs behind a C ABI. All Vulkan resources shared within one VkInstance.

```
Rust (egui + wgpu/Vulkan) <--FFI--> C ABI Bridge <--> OpenUSD/Hydra C++
```

## Project Structure

```
dreamusd/
├── crates/
│   ├── dreamusd-sys/        # OpenUSD/Hydra C ABI bindings (unsafe FFI)
│   ├── dreamusd-bridge/     # C++ bridge source (CMake build)
│   ├── dreamusd-core/       # Safe Rust abstractions (Stage, Prim, Layer, Hydra)
│   ├── dreamusd-render/     # wgpu integration, Hydra VkImage -> wgpu texture
│   ├── dreamusd-ui/         # egui UI components
│   └── dreamusd-app/        # Main application entry point
├── bridge/                  # C++ bridge code
│   ├── CMakeLists.txt
│   ├── include/
│   │   └── dreamusd_bridge.h
│   └── src/
│       ├── stage.cpp
│       ├── prim.cpp
│       ├── hydra.cpp
│       ├── material.cpp
│       └── transform.cpp
├── docs/plans/
├── Cargo.toml               # workspace
└── README.md
```

### Crate Responsibilities

- **dreamusd-sys**: Raw FFI declarations (`extern "C"`), build.rs calls CMake to compile bridge
- **dreamusd-bridge**: C++ source wrapping OpenUSD/Hydra, exposed via C ABI
- **dreamusd-core**: Safe Rust types (Stage, Prim, HydraEngine, etc.), no unsafe leaks
- **dreamusd-render**: wgpu Vulkan setup, VkImage wrapping, frame synchronization
- **dreamusd-ui**: egui panels (scene hierarchy, properties, viewport, material)
- **dreamusd-app**: Window creation, event loop, assembles all crates

## C ABI Bridge Design

All C++ objects represented as opaque pointers. All functions return `DuStatus` error codes.

### Core Types

```c
typedef struct DuStage DuStage;
typedef struct DuPrim DuPrim;
typedef struct DuHydraEngine DuHydraEngine;
typedef enum { DU_OK = 0, DU_ERR_IO, DU_ERR_INVALID, DU_ERR_NULL } DuStatus;
```

### Stage Operations

```c
DuStatus du_stage_open(const char* path, DuStage** out);
DuStatus du_stage_create_new(const char* path, DuStage** out);
DuStatus du_stage_save(DuStage* stage);
DuStatus du_stage_export(DuStage* stage, const char* path);
void     du_stage_destroy(DuStage* stage);
```

### Prim Operations

```c
DuStatus du_prim_get_root(DuStage* stage, DuPrim** out);
DuStatus du_prim_get_children(DuPrim* prim, DuPrim*** out, uint32_t* count);
DuStatus du_prim_create(DuStage* stage, const char* path, const char* type, DuPrim** out);
DuStatus du_prim_remove(DuStage* stage, const char* path);
DuStatus du_prim_reparent(DuPrim* prim, const char* new_parent_path);
DuStatus du_prim_get_type_name(DuPrim* prim, const char** out);
```

### Transform Operations

```c
DuStatus du_xform_get_local(DuPrim* prim, double matrix[16]);
DuStatus du_xform_set_translate(DuPrim* prim, double x, double y, double z);
DuStatus du_xform_set_rotate(DuPrim* prim, double x, double y, double z);
DuStatus du_xform_set_scale(DuPrim* prim, double x, double y, double z);
```

### Attribute Operations

```c
DuStatus du_attr_get_names(DuPrim* prim, const char*** out, uint32_t* count);
DuStatus du_attr_get_value_as_string(DuPrim* prim, const char* name, char** out);
DuStatus du_attr_set_value_from_string(DuPrim* prim, const char* name, const char* value);
```

Attribute values serialized as strings to simplify C ABI (avoids per-type C structs).

### Variant Operations

```c
DuStatus du_variant_get_sets(DuPrim* prim, const char*** out, uint32_t* count);
DuStatus du_variant_get_selection(DuPrim* prim, const char* set_name, const char** out);
DuStatus du_variant_set_selection(DuPrim* prim, const char* set_name, const char* variant);
```

### Hydra Rendering (Vulkan Shared)

```c
DuStatus du_hydra_create_with_vulkan(
    DuStage* stage,
    VkInstance instance,
    VkPhysicalDevice physical_device,
    VkDevice device,
    uint32_t queue_family_index,
    DuHydraEngine** out
);

DuStatus du_hydra_render(DuHydraEngine* engine, uint32_t width, uint32_t height);

DuStatus du_hydra_get_vk_image(
    DuHydraEngine* engine,
    VkImage* image,
    VkImageView* view,
    VkFormat* format,
    uint32_t* width,
    uint32_t* height
);

DuStatus du_hydra_get_render_semaphore(
    DuHydraEngine* engine,
    VkSemaphore* semaphore
);

DuStatus du_hydra_set_camera(DuHydraEngine* engine, double eye[3], double target[3], double up[3]);
DuStatus du_hydra_set_display_mode(DuHydraEngine* engine, DuDisplayMode mode);
void     du_hydra_destroy(DuHydraEngine* engine);
```

### Render Delegate Management

```c
DuStatus du_rd_list_available(const char*** names, uint32_t* count);
DuStatus du_rd_get_current(DuHydraEngine* engine, const char** name);
DuStatus du_rd_set_current(DuHydraEngine* engine, const char* name);
```

### Material and Texture

```c
DuStatus du_material_get_binding(DuPrim* prim, const char** material_path);
DuStatus du_material_get_params(DuPrim* material_prim, DuMaterialParam** params, uint32_t* count);
DuStatus du_material_set_param(DuPrim* material_prim, const char* param_name, const char* value);
DuStatus du_texture_get_thumbnail(const char* asset_path, uint8_t** rgba, uint32_t* w, uint32_t* h, uint32_t max_size);

typedef struct {
    const char* name;
    const char* type;
    const char* value;
    bool is_texture;
} DuMaterialParam;
```

### Undo/Redo

```c
DuStatus du_undo_begin(DuStage* stage);
DuStatus du_undo_end(DuStage* stage);
DuStatus du_undo(DuStage* stage);
DuStatus du_redo(DuStage* stage);
```

C++ side maintains operation stack recording SdfLayer deltas.

### Error Handling and Logging

```c
DuStatus du_get_last_error(const char** message);
typedef void (*DuLogCallback)(DuLogLevel level, const char* message);
DuStatus du_set_log_callback(DuLogCallback cb);
```

### Memory Management

```c
void du_free_string(char* s);
void du_free_string_array(const char** arr, uint32_t count);
void du_free_prim_array(DuPrim** arr, uint32_t count);
```

## Rust Safe Wrapper Pattern

```rust
pub struct Stage { raw: *mut ffi::DuStage }

impl Stage {
    pub fn open(path: &Path) -> Result<Self, DuError> { ... }
    pub fn save(&self) -> Result<(), DuError> { ... }
    pub fn root_prim(&self) -> Result<Prim, DuError> { ... }
}

impl Drop for Stage {
    fn drop(&mut self) { unsafe { ffi::du_stage_destroy(self.raw); } }
}
```

All unsafe confined to dreamusd-sys. dreamusd-core exposes only safe Rust API.

## Rendering Pipeline

### Vulkan Unified Architecture

All platforms use Vulkan (macOS via MoltenVK). Storm and wgpu share the same VkInstance.

```
Same VkInstance
├── Storm Hgi (Vulkan) → renders to VkImage
└── wgpu (Vulkan)      → samples VkImage as texture → egui displays
```

wgpu forced to Vulkan backend:
```rust
let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
    backends: wgpu::Backends::VULKAN,
    ..Default::default()
});
```

Storm's VkImage wrapped as wgpu::Texture via `wgpu::hal::vulkan::Device::create_texture_from_raw()`.

### Frame Loop

1. egui processes input events
2. Camera parameters updated -> `du_hydra_set_camera()`
3. `du_hydra_render()` - Storm renders to shared VkImage
4. Wait on render semaphore
5. wgpu samples the texture, egui::Image displays it
6. wgpu present

### Display Modes

```rust
pub enum DisplayMode {
    SmoothShaded,
    Wireframe,
    WireframeOnShaded,
    FlatShaded,
    Points,
    Textured,
}
```

Implemented via HdRprimCollection repr selectors and Storm drawMode settings.

## Camera Control

```rust
pub struct ViewportCamera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

pub enum CameraMode { Orbit, Pan, Fly }
```

Maya/Houdini-style controls: Alt+LMB orbit, Alt+MMB pan, Alt+RMB zoom.

## UI Layout

```
┌─────────────────────────────────────────────────────┐
│  Menu Bar: File | Edit | View | Tools | Help        │
├────────────┬────────────────────────┬───────────────┤
│  Scene     │     3D Viewport        │  Properties   │
│  Hierarchy │                        │  - Transform  │
│  (tree)    │                        │  - Attributes │
│            │                        │  - Variants   │
│            │                        │  - Material   │
├────────────┴────────────────────────┴───────────────┤
│  Status: FPS | Render Delegate | Display Mode       │
└─────────────────────────────────────────────────────┘
```

### Scene Hierarchy (left panel)
- Tree view of USD prim hierarchy, lazy-loaded
- Right-click: create prim, delete, reparent (drag & drop)
- Type icons (Xform, Mesh, Camera, Light, Material)
- Search/filter box

### 3D Viewport (center)
- egui CentralPanel
- Mouse picking via Hydra pick API
- Transform gizmo: W (translate), E (rotate), R (scale)
- Toolbar: display mode dropdown, render delegate dropdown, grid/axis toggle

### Properties Panel (right)
- Dynamic based on selected prim
- Transform: XYZ input fields + sliders
- Attributes: type-aware editors (float->slider, string->textbox, color->picker)
- Variants: dropdown to switch variant selections
- Material: bound material name, texture thumbnails

### Keyboard Shortcuts

| Action | Shortcut |
|--------|----------|
| Open | Ctrl+O |
| Save | Ctrl+S |
| Save As | Ctrl+Shift+S |
| Undo | Ctrl+Z |
| Redo | Ctrl+Shift+Z |
| Delete | Delete |
| Focus selected | F |
| Translate mode | W |
| Rotate mode | E |
| Scale mode | R |
| Toggle wireframe | Z |

## Dependencies

### Rust Crates
- eframe / egui - UI framework
- wgpu - GPU abstraction (Vulkan backend)
- ash - Raw Vulkan bindings (for interop)
- glam - Math (Vec3, Mat4)
- tracing - Logging
- rfd - Native file dialogs
- cmake - Build script helper

### C++ Libraries
- OpenUSD (pxr) - Stage, Prim, Hydra, Storm, Hgi
- Vulkan SDK

## Platform Notes

- macOS: Vulkan via MoltenVK, requires Vulkan SDK installed
- Linux: Native Vulkan
- Windows: Native Vulkan
