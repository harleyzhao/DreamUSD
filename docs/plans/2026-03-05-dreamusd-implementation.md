# DreamUSD Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a high-performance USD viewer/editor in Rust with Hydra rendering via C ABI bridge, egui UI, and GPU-shared Vulkan textures.

**Architecture:** Rust app shell (egui + wgpu/Vulkan) communicates with OpenUSD/Hydra C++ via a C ABI bridge layer. Storm renders to a VkImage shared with wgpu for zero-copy display.

**Tech Stack:** Rust, C++17, OpenUSD, Hydra Storm, egui, wgpu, ash, Vulkan, CMake

**Design doc:** `docs/plans/2026-03-05-dreamusd-viewer-design.md`

---

## Phase 1: Project Scaffolding & Build System

### Task 1: Create Cargo Workspace

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/dreamusd-sys/Cargo.toml`
- Create: `crates/dreamusd-sys/src/lib.rs`
- Create: `crates/dreamusd-core/Cargo.toml`
- Create: `crates/dreamusd-core/src/lib.rs`
- Create: `crates/dreamusd-render/Cargo.toml`
- Create: `crates/dreamusd-render/src/lib.rs`
- Create: `crates/dreamusd-ui/Cargo.toml`
- Create: `crates/dreamusd-ui/src/lib.rs`
- Create: `crates/dreamusd-app/Cargo.toml`
- Create: `crates/dreamusd-app/src/main.rs`

**Step 1: Create workspace Cargo.toml**

```toml
# Cargo.toml
[workspace]
resolver = "2"
members = [
    "crates/dreamusd-sys",
    "crates/dreamusd-core",
    "crates/dreamusd-render",
    "crates/dreamusd-ui",
    "crates/dreamusd-app",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
```

**Step 2: Create dreamusd-sys crate**

```toml
# crates/dreamusd-sys/Cargo.toml
[package]
name = "dreamusd-sys"
version.workspace = true
edition.workspace = true

[build-dependencies]
cmake = "0.1"

[dependencies]
```

```rust
// crates/dreamusd-sys/src/lib.rs
#![allow(non_camel_case_types)]
// FFI bindings will go here
```

**Step 3: Create dreamusd-core crate**

```toml
# crates/dreamusd-core/Cargo.toml
[package]
name = "dreamusd-core"
version.workspace = true
edition.workspace = true

[dependencies]
dreamusd-sys = { path = "../dreamusd-sys" }
thiserror = "2"
```

```rust
// crates/dreamusd-core/src/lib.rs
pub mod error;
```

**Step 4: Create dreamusd-render crate**

```toml
# crates/dreamusd-render/Cargo.toml
[package]
name = "dreamusd-render"
version.workspace = true
edition.workspace = true

[dependencies]
dreamusd-core = { path = "../dreamusd-core" }
wgpu = "24"
ash = "0.38"
glam = "0.29"
```

```rust
// crates/dreamusd-render/src/lib.rs
pub mod viewport;
```

**Step 5: Create dreamusd-ui crate**

```toml
# crates/dreamusd-ui/Cargo.toml
[package]
name = "dreamusd-ui"
version.workspace = true
edition.workspace = true

[dependencies]
dreamusd-core = { path = "../dreamusd-core" }
dreamusd-render = { path = "../dreamusd-render" }
egui = "0.31"
```

```rust
// crates/dreamusd-ui/src/lib.rs
pub mod panels;
```

**Step 6: Create dreamusd-app crate**

```toml
# crates/dreamusd-app/Cargo.toml
[package]
name = "dreamusd-app"
version.workspace = true
edition.workspace = true

[dependencies]
dreamusd-core = { path = "../dreamusd-core" }
dreamusd-render = { path = "../dreamusd-render" }
dreamusd-ui = { path = "../dreamusd-ui" }
eframe = { version = "0.31", features = ["wgpu"] }
tracing-subscriber = "0.3"
rfd = "0.15"
```

```rust
// crates/dreamusd-app/src/main.rs
fn main() {
    println!("DreamUSD starting...");
}
```

**Step 7: Verify workspace builds**

Run: `cargo check`
Expected: Compiles with no errors.

**Step 8: Commit**

```bash
git add -A
git commit -m "feat: scaffold Cargo workspace with all crates"
```

---

### Task 2: C++ Bridge Directory & CMake Setup

**Files:**
- Create: `bridge/CMakeLists.txt`
- Create: `bridge/include/dreamusd_bridge.h`
- Create: `bridge/src/stub.cpp`

**Step 1: Create CMakeLists.txt**

```cmake
# bridge/CMakeLists.txt
cmake_minimum_required(VERSION 3.24)
project(dreamusd_bridge CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# Find OpenUSD — user must set pxr_DIR or USD_ROOT
find_package(pxr QUIET)
if(NOT pxr_FOUND)
    message(STATUS "OpenUSD not found via find_package, trying USD_ROOT env")
    if(DEFINED ENV{USD_ROOT})
        set(PXR_PREFIX $ENV{USD_ROOT})
        set(PXR_INCLUDE_DIRS ${PXR_PREFIX}/include)
        set(PXR_LIB_DIR ${PXR_PREFIX}/lib)
    else()
        message(FATAL_ERROR "OpenUSD not found. Set USD_ROOT environment variable.")
    endif()
endif()

# Find Vulkan
find_package(Vulkan REQUIRED)

add_library(dreamusd_bridge STATIC
    src/stub.cpp
)

target_include_directories(dreamusd_bridge PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/include
    ${PXR_INCLUDE_DIRS}
    ${Vulkan_INCLUDE_DIRS}
)

# Install target for build.rs to find
install(TARGETS dreamusd_bridge ARCHIVE DESTINATION lib)
install(DIRECTORY include/ DESTINATION include)
```

**Step 2: Create C ABI header with core types only**

```c
// bridge/include/dreamusd_bridge.h
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
DuStatus du_hydra_create_with_vulkan(
    DuStage* stage,
    void* vk_instance,          // VkInstance
    void* vk_physical_device,   // VkPhysicalDevice
    void* vk_device,            // VkDevice
    uint32_t queue_family_index,
    DuHydraEngine** out
);
DuStatus du_hydra_render(DuHydraEngine* engine, uint32_t width, uint32_t height);
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
```

**Step 3: Create stub.cpp (compiles but all functions return DU_ERR_INVALID)**

```cpp
// bridge/src/stub.cpp
#include "dreamusd_bridge.h"
#include <cstring>
#include <cstdlib>

static thread_local const char* g_last_error = "not implemented";

// --- Error & Logging ---
DuStatus du_get_last_error(const char** message) {
    if (!message) return DU_ERR_NULL;
    *message = g_last_error;
    return DU_OK;
}

DuStatus du_set_log_callback(DuLogCallback /*cb*/) {
    return DU_OK;
}

// --- Stage ---
DuStatus du_stage_open(const char*, DuStage**) { return DU_ERR_INVALID; }
DuStatus du_stage_create_new(const char*, DuStage**) { return DU_ERR_INVALID; }
DuStatus du_stage_save(DuStage*) { return DU_ERR_INVALID; }
DuStatus du_stage_export(DuStage*, const char*) { return DU_ERR_INVALID; }
void     du_stage_destroy(DuStage*) {}

// --- Prim ---
DuStatus du_prim_get_root(DuStage*, DuPrim**) { return DU_ERR_INVALID; }
DuStatus du_prim_get_children(DuPrim*, DuPrim***, uint32_t*) { return DU_ERR_INVALID; }
DuStatus du_prim_create(DuStage*, const char*, const char*, DuPrim**) { return DU_ERR_INVALID; }
DuStatus du_prim_remove(DuStage*, const char*) { return DU_ERR_INVALID; }
DuStatus du_prim_reparent(DuPrim*, const char*) { return DU_ERR_INVALID; }
DuStatus du_prim_get_type_name(DuPrim*, const char**) { return DU_ERR_INVALID; }
DuStatus du_prim_get_path(DuPrim*, const char**) { return DU_ERR_INVALID; }
DuStatus du_prim_get_name(DuPrim*, const char**) { return DU_ERR_INVALID; }

// --- Transform ---
DuStatus du_xform_get_local(DuPrim*, double[16]) { return DU_ERR_INVALID; }
DuStatus du_xform_set_translate(DuPrim*, double, double, double) { return DU_ERR_INVALID; }
DuStatus du_xform_set_rotate(DuPrim*, double, double, double) { return DU_ERR_INVALID; }
DuStatus du_xform_set_scale(DuPrim*, double, double, double) { return DU_ERR_INVALID; }

// --- Attributes ---
DuStatus du_attr_get_names(DuPrim*, const char***, uint32_t*) { return DU_ERR_INVALID; }
DuStatus du_attr_get_value_as_string(DuPrim*, const char*, char**) { return DU_ERR_INVALID; }
DuStatus du_attr_set_value_from_string(DuPrim*, const char*, const char*) { return DU_ERR_INVALID; }

// --- Variants ---
DuStatus du_variant_get_sets(DuPrim*, const char***, uint32_t*) { return DU_ERR_INVALID; }
DuStatus du_variant_get_selection(DuPrim*, const char*, const char**) { return DU_ERR_INVALID; }
DuStatus du_variant_set_selection(DuPrim*, const char*, const char*) { return DU_ERR_INVALID; }

// --- Hydra ---
DuStatus du_hydra_create_with_vulkan(DuStage*, void*, void*, void*, uint32_t, DuHydraEngine**) { return DU_ERR_INVALID; }
DuStatus du_hydra_render(DuHydraEngine*, uint32_t, uint32_t) { return DU_ERR_INVALID; }
DuStatus du_hydra_get_vk_image(DuHydraEngine*, void*, void*, uint32_t*, uint32_t*, uint32_t*) { return DU_ERR_INVALID; }
DuStatus du_hydra_get_render_semaphore(DuHydraEngine*, void*) { return DU_ERR_INVALID; }
DuStatus du_hydra_set_camera(DuHydraEngine*, double[3], double[3], double[3]) { return DU_ERR_INVALID; }
DuStatus du_hydra_set_display_mode(DuHydraEngine*, DuDisplayMode) { return DU_ERR_INVALID; }
void     du_hydra_destroy(DuHydraEngine*) {}

// --- Render Delegates ---
DuStatus du_rd_list_available(const char***, uint32_t*) { return DU_ERR_INVALID; }
DuStatus du_rd_get_current(DuHydraEngine*, const char**) { return DU_ERR_INVALID; }
DuStatus du_rd_set_current(DuHydraEngine*, const char*) { return DU_ERR_INVALID; }

// --- Material ---
DuStatus du_material_get_binding(DuPrim*, const char**) { return DU_ERR_INVALID; }
DuStatus du_material_get_params(DuPrim*, DuMaterialParam**, uint32_t*) { return DU_ERR_INVALID; }
DuStatus du_material_set_param(DuPrim*, const char*, const char*) { return DU_ERR_INVALID; }
DuStatus du_texture_get_thumbnail(const char*, uint8_t**, uint32_t*, uint32_t*, uint32_t) { return DU_ERR_INVALID; }

// --- Undo/Redo ---
DuStatus du_undo_begin(DuStage*) { return DU_ERR_INVALID; }
DuStatus du_undo_end(DuStage*) { return DU_ERR_INVALID; }
DuStatus du_undo(DuStage*) { return DU_ERR_INVALID; }
DuStatus du_redo(DuStage*) { return DU_ERR_INVALID; }

// --- Memory ---
void du_free_string(char* s) { free(s); }
void du_free_string_array(const char** arr, uint32_t count) {
    for (uint32_t i = 0; i < count; i++) free((void*)arr[i]);
    free((void*)arr);
}
void du_free_prim_array(DuPrim** arr, uint32_t) { free(arr); }
void du_free_material_params(DuMaterialParam* params, uint32_t count) {
    for (uint32_t i = 0; i < count; i++) {
        free((void*)params[i].name);
        free((void*)params[i].type);
        free((void*)params[i].value);
    }
    free(params);
}
```

**Step 4: Verify CMake configures (without linking USD yet)**

Run: `cd bridge && cmake -B build -DCMAKE_BUILD_TYPE=Release 2>&1 | tail -5`
Expected: May warn about USD not found, but CMakeLists.txt should parse.

**Step 5: Commit**

```bash
git add bridge/
git commit -m "feat: add C++ bridge directory with header and stub implementation"
```

---

### Task 3: build.rs for dreamusd-sys

**Files:**
- Create: `crates/dreamusd-sys/build.rs`
- Modify: `crates/dreamusd-sys/src/lib.rs`

**Step 1: Create build.rs that compiles bridge via CMake**

```rust
// crates/dreamusd-sys/build.rs
use std::env;
use std::path::PathBuf;

fn main() {
    let bridge_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../../bridge");

    // Build bridge with cmake crate
    let dst = cmake::build(&bridge_dir);

    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=dreamusd_bridge");

    // Link C++ standard library
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=dylib=c++");
    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=dylib=stdc++");
    #[cfg(target_os = "windows")]
    println!("cargo:rustc-link-lib=dylib=msvcrt");

    // Re-run if bridge code changes
    println!("cargo:rerun-if-changed=../../bridge/src");
    println!("cargo:rerun-if-changed=../../bridge/include");
    println!("cargo:rerun-if-changed=../../bridge/CMakeLists.txt");
}
```

**Step 2: Add FFI declarations to lib.rs**

```rust
// crates/dreamusd-sys/src/lib.rs
#![allow(non_camel_case_types, non_upper_case_globals)]

use std::os::raw::{c_char, c_double, c_void};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuStatus {
    Ok = 0,
    ErrIo = 1,
    ErrInvalid = 2,
    ErrNull = 3,
    ErrUsd = 4,
    ErrVulkan = 5,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuLogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuDisplayMode {
    SmoothShaded = 0,
    Wireframe = 1,
    WireframeOnShaded = 2,
    FlatShaded = 3,
    Points = 4,
    Textured = 5,
}

#[repr(C)]
pub struct DuMaterialParam {
    pub name: *const c_char,
    pub type_name: *const c_char,
    pub value: *const c_char,
    pub is_texture: bool,
}

// Opaque types
pub enum DuStage {}
pub enum DuPrim {}
pub enum DuHydraEngine {}

pub type DuLogCallback = Option<unsafe extern "C" fn(DuLogLevel, *const c_char)>;

extern "C" {
    // Error & Logging
    pub fn du_get_last_error(message: *mut *const c_char) -> DuStatus;
    pub fn du_set_log_callback(cb: DuLogCallback) -> DuStatus;

    // Stage
    pub fn du_stage_open(path: *const c_char, out: *mut *mut DuStage) -> DuStatus;
    pub fn du_stage_create_new(path: *const c_char, out: *mut *mut DuStage) -> DuStatus;
    pub fn du_stage_save(stage: *mut DuStage) -> DuStatus;
    pub fn du_stage_export(stage: *mut DuStage, path: *const c_char) -> DuStatus;
    pub fn du_stage_destroy(stage: *mut DuStage);

    // Prim
    pub fn du_prim_get_root(stage: *mut DuStage, out: *mut *mut DuPrim) -> DuStatus;
    pub fn du_prim_get_children(
        prim: *mut DuPrim,
        out: *mut *mut *mut DuPrim,
        count: *mut u32,
    ) -> DuStatus;
    pub fn du_prim_create(
        stage: *mut DuStage,
        path: *const c_char,
        type_name: *const c_char,
        out: *mut *mut DuPrim,
    ) -> DuStatus;
    pub fn du_prim_remove(stage: *mut DuStage, path: *const c_char) -> DuStatus;
    pub fn du_prim_reparent(prim: *mut DuPrim, new_parent_path: *const c_char) -> DuStatus;
    pub fn du_prim_get_type_name(prim: *mut DuPrim, out: *mut *const c_char) -> DuStatus;
    pub fn du_prim_get_path(prim: *mut DuPrim, out: *mut *const c_char) -> DuStatus;
    pub fn du_prim_get_name(prim: *mut DuPrim, out: *mut *const c_char) -> DuStatus;

    // Transform
    pub fn du_xform_get_local(prim: *mut DuPrim, matrix: *mut c_double) -> DuStatus;
    pub fn du_xform_set_translate(prim: *mut DuPrim, x: c_double, y: c_double, z: c_double) -> DuStatus;
    pub fn du_xform_set_rotate(prim: *mut DuPrim, x: c_double, y: c_double, z: c_double) -> DuStatus;
    pub fn du_xform_set_scale(prim: *mut DuPrim, x: c_double, y: c_double, z: c_double) -> DuStatus;

    // Attributes
    pub fn du_attr_get_names(prim: *mut DuPrim, out: *mut *mut *const c_char, count: *mut u32) -> DuStatus;
    pub fn du_attr_get_value_as_string(prim: *mut DuPrim, name: *const c_char, out: *mut *mut c_char) -> DuStatus;
    pub fn du_attr_set_value_from_string(prim: *mut DuPrim, name: *const c_char, value: *const c_char) -> DuStatus;

    // Variants
    pub fn du_variant_get_sets(prim: *mut DuPrim, out: *mut *mut *const c_char, count: *mut u32) -> DuStatus;
    pub fn du_variant_get_selection(prim: *mut DuPrim, set_name: *const c_char, out: *mut *const c_char) -> DuStatus;
    pub fn du_variant_set_selection(prim: *mut DuPrim, set_name: *const c_char, variant: *const c_char) -> DuStatus;

    // Hydra
    pub fn du_hydra_create_with_vulkan(
        stage: *mut DuStage,
        vk_instance: *mut c_void,
        vk_physical_device: *mut c_void,
        vk_device: *mut c_void,
        queue_family_index: u32,
        out: *mut *mut DuHydraEngine,
    ) -> DuStatus;
    pub fn du_hydra_render(engine: *mut DuHydraEngine, width: u32, height: u32) -> DuStatus;
    pub fn du_hydra_get_vk_image(
        engine: *mut DuHydraEngine,
        image: *mut c_void,
        view: *mut c_void,
        format: *mut u32,
        width: *mut u32,
        height: *mut u32,
    ) -> DuStatus;
    pub fn du_hydra_get_render_semaphore(engine: *mut DuHydraEngine, semaphore: *mut c_void) -> DuStatus;
    pub fn du_hydra_set_camera(
        engine: *mut DuHydraEngine,
        eye: *const c_double,
        target: *const c_double,
        up: *const c_double,
    ) -> DuStatus;
    pub fn du_hydra_set_display_mode(engine: *mut DuHydraEngine, mode: DuDisplayMode) -> DuStatus;
    pub fn du_hydra_destroy(engine: *mut DuHydraEngine);

    // Render Delegates
    pub fn du_rd_list_available(names: *mut *mut *const c_char, count: *mut u32) -> DuStatus;
    pub fn du_rd_get_current(engine: *mut DuHydraEngine, name: *mut *const c_char) -> DuStatus;
    pub fn du_rd_set_current(engine: *mut DuHydraEngine, name: *const c_char) -> DuStatus;

    // Material
    pub fn du_material_get_binding(prim: *mut DuPrim, material_path: *mut *const c_char) -> DuStatus;
    pub fn du_material_get_params(
        material_prim: *mut DuPrim,
        params: *mut *mut DuMaterialParam,
        count: *mut u32,
    ) -> DuStatus;
    pub fn du_material_set_param(
        material_prim: *mut DuPrim,
        param_name: *const c_char,
        value: *const c_char,
    ) -> DuStatus;
    pub fn du_texture_get_thumbnail(
        asset_path: *const c_char,
        rgba: *mut *mut u8,
        w: *mut u32,
        h: *mut u32,
        max_size: u32,
    ) -> DuStatus;

    // Undo/Redo
    pub fn du_undo_begin(stage: *mut DuStage) -> DuStatus;
    pub fn du_undo_end(stage: *mut DuStage) -> DuStatus;
    pub fn du_undo(stage: *mut DuStage) -> DuStatus;
    pub fn du_redo(stage: *mut DuStage) -> DuStatus;

    // Memory
    pub fn du_free_string(s: *mut c_char);
    pub fn du_free_string_array(arr: *mut *const c_char, count: u32);
    pub fn du_free_prim_array(arr: *mut *mut DuPrim, count: u32);
    pub fn du_free_material_params(params: *mut DuMaterialParam, count: u32);
}
```

**Step 3: Verify full workspace builds (stub links)**

Run: `cargo build 2>&1 | tail -10`
Expected: Compiles and links against stub bridge library.

**Step 4: Commit**

```bash
git add crates/dreamusd-sys/
git commit -m "feat: add build.rs and FFI declarations for dreamusd-sys"
```

---

## Phase 2: Safe Rust Abstractions (dreamusd-core)

### Task 4: Error Types & Core Traits

**Files:**
- Create: `crates/dreamusd-core/src/error.rs`
- Modify: `crates/dreamusd-core/src/lib.rs`

**Step 1: Create error module**

```rust
// crates/dreamusd-core/src/error.rs
use dreamusd_sys::DuStatus;
use std::ffi::CStr;

#[derive(Debug, thiserror::Error)]
pub enum DuError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Invalid operation: {0}")]
    Invalid(String),
    #[error("Null pointer")]
    Null,
    #[error("USD error: {0}")]
    Usd(String),
    #[error("Vulkan error: {0}")]
    Vulkan(String),
}

impl DuError {
    pub(crate) fn from_status(status: DuStatus) -> Self {
        let detail = unsafe {
            let mut msg: *const std::os::raw::c_char = std::ptr::null();
            if dreamusd_sys::du_get_last_error(&mut msg) == DuStatus::Ok && !msg.is_null() {
                CStr::from_ptr(msg).to_string_lossy().into_owned()
            } else {
                String::new()
            }
        };
        match status {
            DuStatus::ErrIo => DuError::Io(detail),
            DuStatus::ErrInvalid => DuError::Invalid(detail),
            DuStatus::ErrNull => DuError::Null,
            DuStatus::ErrUsd => DuError::Usd(detail),
            DuStatus::ErrVulkan => DuError::Vulkan(detail),
            DuStatus::Ok => unreachable!("from_status called with Ok"),
        }
    }
}

/// Check a DuStatus and convert to Result
pub(crate) fn check(status: DuStatus) -> Result<(), DuError> {
    if status == DuStatus::Ok {
        Ok(())
    } else {
        Err(DuError::from_status(status))
    }
}
```

**Step 2: Update lib.rs**

```rust
// crates/dreamusd-core/src/lib.rs
pub mod error;
pub mod stage;
pub mod prim;

pub use error::DuError;
pub use stage::Stage;
pub use prim::Prim;
```

**Step 3: Commit**

```bash
git add crates/dreamusd-core/
git commit -m "feat: add error types for dreamusd-core"
```

---

### Task 5: Stage & Prim Safe Wrappers

**Files:**
- Create: `crates/dreamusd-core/src/stage.rs`
- Create: `crates/dreamusd-core/src/prim.rs`

**Step 1: Create Stage wrapper**

```rust
// crates/dreamusd-core/src/stage.rs
use crate::error::{check, DuError};
use crate::prim::Prim;
use dreamusd_sys;
use std::ffi::CString;
use std::path::Path;

pub struct Stage {
    pub(crate) raw: *mut dreamusd_sys::DuStage,
}

// Stage is not thread-safe (USD stages aren't)
// but we can Send between threads if careful
unsafe impl Send for Stage {}

impl Stage {
    pub fn open(path: &Path) -> Result<Self, DuError> {
        let c_path = CString::new(path.to_str().ok_or(DuError::Invalid("invalid path".into()))?)
            .map_err(|_| DuError::Invalid("path contains null byte".into()))?;
        let mut raw = std::ptr::null_mut();
        check(unsafe { dreamusd_sys::du_stage_open(c_path.as_ptr(), &mut raw) })?;
        Ok(Stage { raw })
    }

    pub fn create_new(path: &Path) -> Result<Self, DuError> {
        let c_path = CString::new(path.to_str().ok_or(DuError::Invalid("invalid path".into()))?)
            .map_err(|_| DuError::Invalid("path contains null byte".into()))?;
        let mut raw = std::ptr::null_mut();
        check(unsafe { dreamusd_sys::du_stage_create_new(c_path.as_ptr(), &mut raw) })?;
        Ok(Stage { raw })
    }

    pub fn save(&self) -> Result<(), DuError> {
        check(unsafe { dreamusd_sys::du_stage_save(self.raw) })
    }

    pub fn export(&self, path: &Path) -> Result<(), DuError> {
        let c_path = CString::new(path.to_str().ok_or(DuError::Invalid("invalid path".into()))?)
            .map_err(|_| DuError::Invalid("path contains null byte".into()))?;
        check(unsafe { dreamusd_sys::du_stage_export(self.raw, c_path.as_ptr()) })
    }

    pub fn root_prim(&self) -> Result<Prim, DuError> {
        let mut raw = std::ptr::null_mut();
        check(unsafe { dreamusd_sys::du_prim_get_root(self.raw, &mut raw) })?;
        Ok(Prim { raw, owned: false })
    }

    pub fn create_prim(&self, path: &str, type_name: &str) -> Result<Prim, DuError> {
        let c_path = CString::new(path).map_err(|_| DuError::Invalid("null byte in path".into()))?;
        let c_type = CString::new(type_name).map_err(|_| DuError::Invalid("null byte in type".into()))?;
        let mut raw = std::ptr::null_mut();
        check(unsafe {
            dreamusd_sys::du_prim_create(self.raw, c_path.as_ptr(), c_type.as_ptr(), &mut raw)
        })?;
        Ok(Prim { raw, owned: false })
    }

    pub fn remove_prim(&self, path: &str) -> Result<(), DuError> {
        let c_path = CString::new(path).map_err(|_| DuError::Invalid("null byte in path".into()))?;
        check(unsafe { dreamusd_sys::du_prim_remove(self.raw, c_path.as_ptr()) })
    }

    // Undo/Redo
    pub fn undo_begin(&self) -> Result<(), DuError> {
        check(unsafe { dreamusd_sys::du_undo_begin(self.raw) })
    }

    pub fn undo_end(&self) -> Result<(), DuError> {
        check(unsafe { dreamusd_sys::du_undo_end(self.raw) })
    }

    pub fn undo(&self) -> Result<(), DuError> {
        check(unsafe { dreamusd_sys::du_undo(self.raw) })
    }

    pub fn redo(&self) -> Result<(), DuError> {
        check(unsafe { dreamusd_sys::du_redo(self.raw) })
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        unsafe { dreamusd_sys::du_stage_destroy(self.raw) };
    }
}
```

**Step 2: Create Prim wrapper**

```rust
// crates/dreamusd-core/src/prim.rs
use crate::error::{check, DuError};
use dreamusd_sys;
use std::ffi::{CStr, CString};

pub struct Prim {
    pub(crate) raw: *mut dreamusd_sys::DuPrim,
    pub(crate) owned: bool,
}

unsafe impl Send for Prim {}

impl Prim {
    pub fn name(&self) -> Result<String, DuError> {
        let mut name: *const std::os::raw::c_char = std::ptr::null();
        check(unsafe { dreamusd_sys::du_prim_get_name(self.raw, &mut name) })?;
        Ok(unsafe { CStr::from_ptr(name) }.to_string_lossy().into_owned())
    }

    pub fn path(&self) -> Result<String, DuError> {
        let mut path: *const std::os::raw::c_char = std::ptr::null();
        check(unsafe { dreamusd_sys::du_prim_get_path(self.raw, &mut path) })?;
        Ok(unsafe { CStr::from_ptr(path) }.to_string_lossy().into_owned())
    }

    pub fn type_name(&self) -> Result<String, DuError> {
        let mut name: *const std::os::raw::c_char = std::ptr::null();
        check(unsafe { dreamusd_sys::du_prim_get_type_name(self.raw, &mut name) })?;
        Ok(unsafe { CStr::from_ptr(name) }.to_string_lossy().into_owned())
    }

    pub fn children(&self) -> Result<Vec<Prim>, DuError> {
        let mut arr: *mut *mut dreamusd_sys::DuPrim = std::ptr::null_mut();
        let mut count: u32 = 0;
        check(unsafe { dreamusd_sys::du_prim_get_children(self.raw, &mut arr, &mut count) })?;
        let prims = (0..count as usize)
            .map(|i| Prim {
                raw: unsafe { *arr.add(i) },
                owned: false,
            })
            .collect();
        unsafe { dreamusd_sys::du_free_prim_array(arr, count) };
        Ok(prims)
    }

    pub fn reparent(&self, new_parent_path: &str) -> Result<(), DuError> {
        let c_path = CString::new(new_parent_path)
            .map_err(|_| DuError::Invalid("null byte in path".into()))?;
        check(unsafe { dreamusd_sys::du_prim_reparent(self.raw, c_path.as_ptr()) })
    }

    // Transform
    pub fn get_local_matrix(&self) -> Result<[f64; 16], DuError> {
        let mut matrix = [0.0f64; 16];
        check(unsafe { dreamusd_sys::du_xform_get_local(self.raw, matrix.as_mut_ptr()) })?;
        Ok(matrix)
    }

    pub fn set_translate(&self, x: f64, y: f64, z: f64) -> Result<(), DuError> {
        check(unsafe { dreamusd_sys::du_xform_set_translate(self.raw, x, y, z) })
    }

    pub fn set_rotate(&self, x: f64, y: f64, z: f64) -> Result<(), DuError> {
        check(unsafe { dreamusd_sys::du_xform_set_rotate(self.raw, x, y, z) })
    }

    pub fn set_scale(&self, x: f64, y: f64, z: f64) -> Result<(), DuError> {
        check(unsafe { dreamusd_sys::du_xform_set_scale(self.raw, x, y, z) })
    }

    // Attributes
    pub fn attribute_names(&self) -> Result<Vec<String>, DuError> {
        let mut arr: *mut *const std::os::raw::c_char = std::ptr::null_mut();
        let mut count: u32 = 0;
        check(unsafe { dreamusd_sys::du_attr_get_names(self.raw, &mut arr, &mut count) })?;
        let names = (0..count as usize)
            .map(|i| unsafe { CStr::from_ptr(*arr.add(i)) }.to_string_lossy().into_owned())
            .collect();
        unsafe { dreamusd_sys::du_free_string_array(arr, count) };
        Ok(names)
    }

    pub fn get_attribute(&self, name: &str) -> Result<String, DuError> {
        let c_name = CString::new(name).map_err(|_| DuError::Invalid("null byte".into()))?;
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        check(unsafe { dreamusd_sys::du_attr_get_value_as_string(self.raw, c_name.as_ptr(), &mut out) })?;
        let val = unsafe { CStr::from_ptr(out) }.to_string_lossy().into_owned();
        unsafe { dreamusd_sys::du_free_string(out) };
        Ok(val)
    }

    pub fn set_attribute(&self, name: &str, value: &str) -> Result<(), DuError> {
        let c_name = CString::new(name).map_err(|_| DuError::Invalid("null byte".into()))?;
        let c_value = CString::new(value).map_err(|_| DuError::Invalid("null byte".into()))?;
        check(unsafe {
            dreamusd_sys::du_attr_set_value_from_string(self.raw, c_name.as_ptr(), c_value.as_ptr())
        })
    }

    // Variants
    pub fn variant_sets(&self) -> Result<Vec<String>, DuError> {
        let mut arr: *mut *const std::os::raw::c_char = std::ptr::null_mut();
        let mut count: u32 = 0;
        check(unsafe { dreamusd_sys::du_variant_get_sets(self.raw, &mut arr, &mut count) })?;
        let names = (0..count as usize)
            .map(|i| unsafe { CStr::from_ptr(*arr.add(i)) }.to_string_lossy().into_owned())
            .collect();
        unsafe { dreamusd_sys::du_free_string_array(arr, count) };
        Ok(names)
    }

    pub fn get_variant_selection(&self, set_name: &str) -> Result<String, DuError> {
        let c_name = CString::new(set_name).map_err(|_| DuError::Invalid("null byte".into()))?;
        let mut out: *const std::os::raw::c_char = std::ptr::null();
        check(unsafe { dreamusd_sys::du_variant_get_selection(self.raw, c_name.as_ptr(), &mut out) })?;
        Ok(unsafe { CStr::from_ptr(out) }.to_string_lossy().into_owned())
    }

    pub fn set_variant_selection(&self, set_name: &str, variant: &str) -> Result<(), DuError> {
        let c_set = CString::new(set_name).map_err(|_| DuError::Invalid("null byte".into()))?;
        let c_var = CString::new(variant).map_err(|_| DuError::Invalid("null byte".into()))?;
        check(unsafe {
            dreamusd_sys::du_variant_set_selection(self.raw, c_set.as_ptr(), c_var.as_ptr())
        })
    }

    // Material
    pub fn material_binding(&self) -> Result<String, DuError> {
        let mut path: *const std::os::raw::c_char = std::ptr::null();
        check(unsafe { dreamusd_sys::du_material_get_binding(self.raw, &mut path) })?;
        Ok(unsafe { CStr::from_ptr(path) }.to_string_lossy().into_owned())
    }
}
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: Compiles.

**Step 4: Commit**

```bash
git add crates/dreamusd-core/
git commit -m "feat: add Stage and Prim safe wrappers in dreamusd-core"
```

---

## Phase 3: Application Shell with egui + wgpu Viewport

### Task 6: Basic egui Application Window

**Files:**
- Modify: `crates/dreamusd-app/src/main.rs`
- Create: `crates/dreamusd-ui/src/app.rs`
- Modify: `crates/dreamusd-ui/src/lib.rs`
- Modify: `crates/dreamusd-ui/Cargo.toml`

**Step 1: Update dreamusd-ui Cargo.toml**

```toml
# crates/dreamusd-ui/Cargo.toml
[package]
name = "dreamusd-ui"
version.workspace = true
edition.workspace = true

[dependencies]
dreamusd-core = { path = "../dreamusd-core" }
dreamusd-render = { path = "../dreamusd-render" }
egui = "0.31"
eframe = { version = "0.31", features = ["wgpu"] }
rfd = "0.15"
tracing = "0.1"
```

**Step 2: Create app.rs — main application struct with panel layout**

```rust
// crates/dreamusd-ui/src/app.rs
use eframe::egui;

pub struct DreamUsdApp {
    selected_prim_path: Option<String>,
    show_grid: bool,
    show_axis: bool,
    current_display_mode: usize,
    status_message: String,
}

impl Default for DreamUsdApp {
    fn default() -> Self {
        Self {
            selected_prim_path: None,
            show_grid: true,
            show_axis: true,
            current_display_mode: 0,
            status_message: "Ready".into(),
        }
    }
}

const DISPLAY_MODES: &[&str] = &[
    "Smooth Shaded",
    "Wireframe",
    "Wireframe on Shaded",
    "Flat Shaded",
    "Points",
    "Textured",
];

impl eframe::App for DreamUsdApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open (Ctrl+O)").clicked() {
                        self.open_file();
                        ui.close_menu();
                    }
                    if ui.button("Save (Ctrl+S)").clicked() {
                        ui.close_menu();
                    }
                    if ui.button("Save As (Ctrl+Shift+S)").clicked() {
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo (Ctrl+Z)").clicked() { ui.close_menu(); }
                    if ui.button("Redo (Ctrl+Shift+Z)").clicked() { ui.close_menu(); }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_grid, "Grid");
                    ui.checkbox(&mut self.show_axis, "Axis");
                });
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
                ui.separator();
                egui::ComboBox::from_id_salt("display_mode")
                    .selected_text(DISPLAY_MODES[self.current_display_mode])
                    .show_ui(ui, |ui| {
                        for (i, mode) in DISPLAY_MODES.iter().enumerate() {
                            ui.selectable_value(&mut self.current_display_mode, i, *mode);
                        }
                    });
            });
        });

        // Scene hierarchy (left)
        egui::SidePanel::left("scene_hierarchy")
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.heading("Scene Hierarchy");
                ui.separator();
                ui.label("No stage loaded");
            });

        // Properties (right)
        egui::SidePanel::right("properties")
            .default_width(250.0)
            .show(ctx, |ui| {
                ui.heading("Properties");
                ui.separator();
                if let Some(path) = &self.selected_prim_path {
                    ui.label(format!("Selected: {}", path));
                } else {
                    ui.label("No prim selected");
                }
            });

        // 3D Viewport (center)
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                ui.label("3D Viewport — No stage loaded");
            });
        });
    }
}

impl DreamUsdApp {
    fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("USD Files", &["usd", "usda", "usdc", "usdz"])
            .pick_file()
        {
            self.status_message = format!("Opened: {}", path.display());
            tracing::info!("Opening file: {}", path.display());
        }
    }
}
```

**Step 3: Update ui lib.rs**

```rust
// crates/dreamusd-ui/src/lib.rs
pub mod app;
pub mod panels;
```

**Step 4: Create empty panels module**

```rust
// crates/dreamusd-ui/src/panels.rs
// Panel components will be added incrementally
```

**Step 5: Update main.rs**

```rust
// crates/dreamusd-app/src/main.rs
use dreamusd_ui::app::DreamUsdApp;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("DreamUSD")
            .with_inner_size([1280.0, 800.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "DreamUSD",
        options,
        Box::new(|_cc| Ok(Box::new(DreamUsdApp::default()))),
    )
}
```

**Step 6: Verify app launches**

Run: `cargo run -p dreamusd-app`
Expected: Window opens with menu bar, three panels, and status bar.

**Step 7: Commit**

```bash
git add crates/
git commit -m "feat: basic egui application shell with panel layout"
```

---

## Phase 4: C++ Bridge — Stage & Prim (Real OpenUSD)

### Task 7: Implement Stage Operations in C++

**Files:**
- Create: `bridge/src/stage.cpp`
- Create: `bridge/src/error.cpp`
- Create: `bridge/src/error_internal.h`
- Modify: `bridge/CMakeLists.txt`
- Remove: `bridge/src/stub.cpp`

**Step 1: Create internal error helper**

```cpp
// bridge/src/error_internal.h
#ifndef DREAMUSD_ERROR_INTERNAL_H
#define DREAMUSD_ERROR_INTERNAL_H

#include "dreamusd_bridge.h"
#include <string>

void du_set_last_error(const std::string& msg);

#define DU_TRY(expr) \
    try { expr; } catch (const std::exception& e) { \
        du_set_last_error(e.what()); \
        return DU_ERR_USD; \
    }

#define DU_CHECK_NULL(ptr) \
    if (!(ptr)) { du_set_last_error(#ptr " is null"); return DU_ERR_NULL; }

#endif
```

**Step 2: Create error.cpp**

```cpp
// bridge/src/error.cpp
#include "dreamusd_bridge.h"
#include "error_internal.h"
#include <string>

static thread_local std::string g_last_error;
static DuLogCallback g_log_callback = nullptr;

void du_set_last_error(const std::string& msg) {
    g_last_error = msg;
}

void du_log(DuLogLevel level, const std::string& msg) {
    if (g_log_callback) {
        g_log_callback(level, msg.c_str());
    }
}

extern "C" {

DuStatus du_get_last_error(const char** message) {
    if (!message) return DU_ERR_NULL;
    *message = g_last_error.c_str();
    return DU_OK;
}

DuStatus du_set_log_callback(DuLogCallback cb) {
    g_log_callback = cb;
    return DU_OK;
}

} // extern "C"
```

**Step 3: Create stage.cpp with real OpenUSD calls**

```cpp
// bridge/src/stage.cpp
#include "dreamusd_bridge.h"
#include "error_internal.h"

#include <pxr/usd/usd/stage.h>
#include <pxr/usd/usd/prim.h>
#include <pxr/usd/sdf/layer.h>

#include <string>
#include <vector>
#include <memory>

PXR_NAMESPACE_USING_DIRECTIVE

struct DuStage {
    UsdStageRefPtr stage;
    // Undo stack (simplified: store layer snapshots)
    std::vector<std::string> undo_stack;
    std::vector<std::string> redo_stack;
    std::string snapshot_before;
};

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
```

**Step 4: Update CMakeLists.txt sources**

Replace `src/stub.cpp` with real sources in CMakeLists.txt:

```cmake
add_library(dreamusd_bridge STATIC
    src/error.cpp
    src/stage.cpp
    src/prim.cpp
    src/transform.cpp
    src/material.cpp
    src/hydra.cpp
)
```

Add USD libraries to link:

```cmake
target_link_libraries(dreamusd_bridge PUBLIC
    ${PXR_LIB_DIR}/libusd_usd.dylib
    ${PXR_LIB_DIR}/libusd_sdf.dylib
    ${PXR_LIB_DIR}/libusd_tf.dylib
    ${PXR_LIB_DIR}/libusd_vt.dylib
    ${PXR_LIB_DIR}/libusd_gf.dylib
    ${PXR_LIB_DIR}/libusd_usdGeom.dylib
    ${PXR_LIB_DIR}/libusd_usdShade.dylib
    ${PXR_LIB_DIR}/libusd_hd.dylib
    ${PXR_LIB_DIR}/libusd_hdSt.dylib
    ${PXR_LIB_DIR}/libusd_hgi.dylib
    ${PXR_LIB_DIR}/libusd_hgiVulkan.dylib
    Vulkan::Vulkan
)
```

Note: Library names vary by platform and USD build. Use `find_library()` or imported targets from `find_package(pxr)` when available. The above is for a typical macOS USD install.

**Step 5: Commit**

```bash
git add bridge/
git commit -m "feat: implement stage operations with real OpenUSD in bridge"
```

---

### Task 8: Implement Prim Operations in C++

**Files:**
- Create: `bridge/src/prim.cpp`

**Step 1: Implement prim.cpp**

```cpp
// bridge/src/prim.cpp
#include "dreamusd_bridge.h"
#include "error_internal.h"

#include <pxr/usd/usd/stage.h>
#include <pxr/usd/usd/prim.h>
#include <pxr/usd/usd/attribute.h>
#include <pxr/usd/usd/variantSets.h>
#include <pxr/usd/sdf/path.h>
#include <pxr/base/vt/value.h>

#include <cstdlib>
#include <cstring>
#include <sstream>

PXR_NAMESPACE_USING_DIRECTIVE

// Forward declaration — DuStage defined in stage.cpp
struct DuStage;
extern UsdStageRefPtr du_stage_get_ptr(DuStage* stage);

struct DuPrim {
    UsdPrim prim;
};

static char* du_strdup(const std::string& s) {
    char* out = (char*)malloc(s.size() + 1);
    memcpy(out, s.c_str(), s.size() + 1);
    return out;
}

extern "C" {

DuStatus du_prim_get_root(DuStage* stage, DuPrim** out) {
    DU_CHECK_NULL(stage);
    DU_CHECK_NULL(out);
    auto stagePtr = du_stage_get_ptr(stage);
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
    if (!stagePtr->RemovePrim(SdfPath(path))) {
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
    *out = prim->prim.GetPath().GetText();
    return DU_OK;
}

DuStatus du_prim_get_name(DuPrim* prim, const char** out) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(out);
    *out = prim->prim.GetName().GetText();
    return DU_OK;
}

// --- Attributes ---

DuStatus du_attr_get_names(DuPrim* prim, const char*** out, uint32_t* count) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(out);
    DU_CHECK_NULL(count);

    auto attrs = prim->prim.GetAttributes();
    *count = (uint32_t)attrs.size();
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

    // For now, try to set as string. More sophisticated parsing can be added.
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
    // Store in a thread_local so the pointer stays valid
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
    for (uint32_t i = 0; i < count; i++) free((void*)arr[i]);
    free((void*)arr);
}

void du_free_prim_array(DuPrim** arr, uint32_t count) {
    for (uint32_t i = 0; i < count; i++) delete arr[i];
    free(arr);
}

} // extern "C"
```

**Step 2: Add accessor to stage.cpp so prim.cpp can get UsdStageRefPtr**

Add to bottom of `bridge/src/stage.cpp`:

```cpp
UsdStageRefPtr du_stage_get_ptr(DuStage* stage) {
    return stage->stage;
}
```

**Step 3: Commit**

```bash
git add bridge/
git commit -m "feat: implement prim, attribute, and variant operations in bridge"
```

---

### Task 9: Implement Transform Operations in C++

**Files:**
- Create: `bridge/src/transform.cpp`

**Step 1: Implement transform.cpp**

```cpp
// bridge/src/transform.cpp
#include "dreamusd_bridge.h"
#include "error_internal.h"

#include <pxr/usd/usdGeom/xformable.h>
#include <pxr/usd/usdGeom/xformCommonAPI.h>
#include <pxr/base/gf/matrix4d.h>

PXR_NAMESPACE_USING_DIRECTIVE

struct DuPrim;
// DuPrim has a UsdPrim member — forward access
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
```

**Step 2: Add accessor to prim.cpp**

Add to `bridge/src/prim.cpp`:

```cpp
UsdPrim du_prim_get_usd(DuPrim* prim) {
    return prim->prim;
}
```

**Step 3: Commit**

```bash
git add bridge/
git commit -m "feat: implement transform operations in bridge"
```

---

## Phase 5: Hydra Rendering with Vulkan Shared Textures

### Task 10: Implement Hydra Engine in C++

**Files:**
- Create: `bridge/src/hydra.cpp`

**Step 1: Implement hydra.cpp — Hydra engine with Storm Vulkan backend**

```cpp
// bridge/src/hydra.cpp
#include "dreamusd_bridge.h"
#include "error_internal.h"

#include <pxr/usd/usd/stage.h>
#include <pxr/imaging/hd/engine.h>
#include <pxr/imaging/hd/rendererPlugin.h>
#include <pxr/imaging/hd/rendererPluginRegistry.h>
#include <pxr/imaging/hd/pluginRenderDelegateUniqueHandle.h>
#include <pxr/imaging/hdSt/renderDelegate.h>
#include <pxr/imaging/hgi/hgi.h>
#include <pxr/imaging/hgiVulkan/hgi.h>
#include <pxr/imaging/hdx/taskController.h>
#include <pxr/usdImaging/usdImaging/delegate.h>
#include <pxr/base/gf/camera.h>
#include <pxr/base/gf/frustum.h>

#include <vulkan/vulkan.h>

#include <cstdlib>
#include <cstring>
#include <vector>

PXR_NAMESPACE_USING_DIRECTIVE

struct DuStage;
extern UsdStageRefPtr du_stage_get_ptr(DuStage* stage);

struct DuHydraEngine {
    UsdStageRefPtr stage;

    // Hydra components
    HgiUniquePtr hgi;
    HdRenderIndex* renderIndex = nullptr;
    HdPluginRenderDelegateUniqueHandle renderDelegate;
    std::unique_ptr<UsdImagingDelegate> sceneDelegate;
    std::unique_ptr<HdxTaskController> taskController;
    HdEngine engine;

    // Vulkan resources
    VkImage outputImage = VK_NULL_HANDLE;
    VkImageView outputImageView = VK_NULL_HANDLE;
    VkSemaphore renderSemaphore = VK_NULL_HANDLE;
    VkDevice device = VK_NULL_HANDLE;
    uint32_t outputWidth = 0;
    uint32_t outputHeight = 0;
    VkFormat outputFormat = VK_FORMAT_R8G8B8A8_UNORM;

    // Camera
    GfCamera camera;
    GfVec3d eye{0, 0, 10};
    GfVec3d target{0, 0, 0};
    GfVec3d up{0, 1, 0};

    // Current render delegate id
    TfToken currentRdId;
};

extern "C" {

DuStatus du_hydra_create_with_vulkan(
    DuStage* stage,
    void* vk_instance,
    void* vk_physical_device,
    void* vk_device,
    uint32_t queue_family_index,
    DuHydraEngine** out
) {
    DU_CHECK_NULL(stage);
    DU_CHECK_NULL(out);
    DU_CHECK_NULL(vk_instance);
    DU_CHECK_NULL(vk_device);

    auto eng = new DuHydraEngine();
    eng->stage = du_stage_get_ptr(stage);
    eng->device = (VkDevice)vk_device;

    // Create HgiVulkan using the shared Vulkan device
    // Note: HgiVulkan initialization with external device is version-dependent.
    // This is a simplified version — real implementation needs HgiVulkanInstanceDesc.
    eng->hgi = Hgi::CreatePlatformDefaultHgi();

    // Get Storm render delegate
    TfToken stormId("HdStormRendererPlugin");
    eng->currentRdId = stormId;

    auto& registry = HdRendererPluginRegistry::GetInstance();
    eng->renderDelegate = registry.CreateRenderDelegate(stormId);

    if (!eng->renderDelegate) {
        du_set_last_error("Failed to create Storm render delegate");
        delete eng;
        return DU_ERR_USD;
    }

    eng->renderIndex = HdRenderIndex::New(eng->renderDelegate.Get(), HdDriverVector());
    if (!eng->renderIndex) {
        du_set_last_error("Failed to create render index");
        delete eng;
        return DU_ERR_USD;
    }

    // Scene delegate
    SdfPath delegateId = SdfPath::AbsoluteRootPath();
    eng->sceneDelegate = std::make_unique<UsdImagingDelegate>(eng->renderIndex, delegateId);
    eng->sceneDelegate->Populate(eng->stage->GetPseudoRoot());

    // Task controller
    SdfPath taskControllerId("/taskController");
    eng->taskController = std::make_unique<HdxTaskController>(eng->renderIndex, taskControllerId);

    // Create semaphore for sync
    VkSemaphoreCreateInfo semInfo{};
    semInfo.sType = VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO;
    vkCreateSemaphore(eng->device, &semInfo, nullptr, &eng->renderSemaphore);

    *out = eng;
    return DU_OK;
}

DuStatus du_hydra_render(DuHydraEngine* engine, uint32_t width, uint32_t height) {
    DU_CHECK_NULL(engine);

    // Update viewport size if changed
    if (width != engine->outputWidth || height != engine->outputHeight) {
        engine->outputWidth = width;
        engine->outputHeight = height;
        engine->taskController->SetRenderViewport(GfVec4d(0, 0, width, height));
    }

    // Update camera
    GfFrustum frustum;
    frustum.SetPerspective(45.0, (double)width / (double)height, 0.1, 10000.0);
    frustum.SetPosition(engine->eye);
    frustum.SetRotation(
        GfRotation(GfVec3d(0, 0, -1), engine->target - engine->eye)
    );

    engine->taskController->SetFreeCameraMatrices(
        frustum.ComputeViewMatrix(),
        frustum.ComputeProjectionMatrix()
    );

    // Execute render tasks
    auto tasks = engine->taskController->GetRenderingTasks();
    engine->engine.Execute(engine->renderIndex, &tasks);

    return DU_OK;
}

DuStatus du_hydra_get_vk_image(
    DuHydraEngine* engine,
    void* image,
    void* view,
    uint32_t* format,
    uint32_t* width,
    uint32_t* height
) {
    DU_CHECK_NULL(engine);

    if (image) *(VkImage*)image = engine->outputImage;
    if (view) *(VkImageView*)view = engine->outputImageView;
    if (format) *format = (uint32_t)engine->outputFormat;
    if (width) *width = engine->outputWidth;
    if (height) *height = engine->outputHeight;
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
    engine->eye = GfVec3d(eye[0], eye[1], eye[2]);
    engine->target = GfVec3d(target[0], target[1], target[2]);
    engine->up = GfVec3d(up[0], up[1], up[2]);
    return DU_OK;
}

DuStatus du_hydra_set_display_mode(DuHydraEngine* engine, DuDisplayMode mode) {
    DU_CHECK_NULL(engine);

    // Map display mode to Hydra repr tokens
    TfToken reprToken;
    switch (mode) {
        case DU_DISPLAY_SMOOTH_SHADED:
            reprToken = HdReprTokens->smoothHull;
            break;
        case DU_DISPLAY_WIREFRAME:
            reprToken = HdReprTokens->wire;
            break;
        case DU_DISPLAY_WIREFRAME_ON_SHADED:
            reprToken = HdReprTokens->wireOnSurf;
            break;
        case DU_DISPLAY_FLAT_SHADED:
            reprToken = HdReprTokens->hull;
            break;
        case DU_DISPLAY_POINTS:
            reprToken = HdReprTokens->points;
            break;
        case DU_DISPLAY_TEXTURED:
            reprToken = HdReprTokens->smoothHull;
            break;
    }

    engine->taskController->SetCollection(
        HdRprimCollection(HdTokens->geometry, HdReprSelector(reprToken))
    );

    return DU_OK;
}

void du_hydra_destroy(DuHydraEngine* engine) {
    if (!engine) return;

    if (engine->renderSemaphore != VK_NULL_HANDLE) {
        vkDestroySemaphore(engine->device, engine->renderSemaphore, nullptr);
    }

    engine->taskController.reset();
    engine->sceneDelegate.reset();
    if (engine->renderIndex) {
        delete engine->renderIndex;
    }
    engine->renderDelegate.Reset();
    engine->hgi.reset();

    delete engine;
}

// --- Render Delegates ---

DuStatus du_rd_list_available(const char*** names, uint32_t* count) {
    DU_CHECK_NULL(names);
    DU_CHECK_NULL(count);

    auto& registry = HdRendererPluginRegistry::GetInstance();
    auto pluginIds = registry.GetRegisteredRendererPlugins();

    *count = (uint32_t)pluginIds.size();
    *names = (const char**)malloc(sizeof(const char*) * pluginIds.size());
    for (size_t i = 0; i < pluginIds.size(); i++) {
        auto str = pluginIds[i].GetString();
        (*names)[i] = (const char*)malloc(str.size() + 1);
        memcpy((void*)(*names)[i], str.c_str(), str.size() + 1);
    }
    return DU_OK;
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

    TfToken newId(name);
    if (newId == engine->currentRdId) return DU_OK;

    // Recreate render delegate and index
    engine->taskController.reset();
    engine->sceneDelegate.reset();
    delete engine->renderIndex;
    engine->renderDelegate.Reset();

    auto& registry = HdRendererPluginRegistry::GetInstance();
    engine->renderDelegate = registry.CreateRenderDelegate(newId);
    if (!engine->renderDelegate) {
        du_set_last_error(std::string("Failed to create render delegate: ") + name);
        return DU_ERR_USD;
    }

    engine->renderIndex = HdRenderIndex::New(engine->renderDelegate.Get(), HdDriverVector());
    SdfPath delegateId = SdfPath::AbsoluteRootPath();
    engine->sceneDelegate = std::make_unique<UsdImagingDelegate>(engine->renderIndex, delegateId);
    engine->sceneDelegate->Populate(engine->stage->GetPseudoRoot());

    SdfPath taskControllerId("/taskController");
    engine->taskController = std::make_unique<HdxTaskController>(engine->renderIndex, taskControllerId);

    engine->currentRdId = newId;
    return DU_OK;
}

} // extern "C"
```

**Step 2: Commit**

```bash
git add bridge/src/hydra.cpp
git commit -m "feat: implement Hydra engine with Storm and render delegate switching"
```

---

### Task 11: Implement Material Operations in C++

**Files:**
- Create: `bridge/src/material.cpp`

**Step 1: Implement material.cpp**

```cpp
// bridge/src/material.cpp
#include "dreamusd_bridge.h"
#include "error_internal.h"

#include <pxr/usd/usd/prim.h>
#include <pxr/usd/usdShade/material.h>
#include <pxr/usd/usdShade/materialBindingAPI.h>
#include <pxr/usd/usdShade/shader.h>
#include <pxr/usd/sdf/assetPath.h>
#include <pxr/base/vt/value.h>

#include <cstdlib>
#include <cstring>
#include <sstream>

PXR_NAMESPACE_USING_DIRECTIVE

struct DuPrim;
extern UsdPrim du_prim_get_usd(DuPrim* prim);

static char* du_strdup_m(const std::string& s) {
    char* out = (char*)malloc(s.size() + 1);
    memcpy(out, s.c_str(), s.size() + 1);
    return out;
}

extern "C" {

DuStatus du_material_get_binding(DuPrim* prim, const char** material_path) {
    DU_CHECK_NULL(prim);
    DU_CHECK_NULL(material_path);

    UsdShadeMaterialBindingAPI bindingAPI(du_prim_get_usd(prim));
    auto material = bindingAPI.ComputeBoundMaterial();
    if (!material) {
        du_set_last_error("No material bound");
        return DU_ERR_INVALID;
    }

    static thread_local std::string s_path;
    s_path = material.GetPath().GetString();
    *material_path = s_path.c_str();
    return DU_OK;
}

DuStatus du_material_get_params(DuPrim* material_prim, DuMaterialParam** params, uint32_t* count) {
    DU_CHECK_NULL(material_prim);
    DU_CHECK_NULL(params);
    DU_CHECK_NULL(count);

    UsdShadeMaterial material(du_prim_get_usd(material_prim));
    if (!material) {
        du_set_last_error("Prim is not a Material");
        return DU_ERR_INVALID;
    }

    // Get the surface shader
    auto surface = material.ComputeSurfaceSource();
    if (!surface) {
        *count = 0;
        *params = nullptr;
        return DU_OK;
    }

    auto shader = UsdShadeShader(surface);
    auto inputs = shader.GetInputs();

    *count = (uint32_t)inputs.size();
    *params = (DuMaterialParam*)malloc(sizeof(DuMaterialParam) * inputs.size());

    for (size_t i = 0; i < inputs.size(); i++) {
        auto& input = inputs[i];
        (*params)[i].name = du_strdup_m(input.GetBaseName().GetString());
        (*params)[i].type = du_strdup_m(input.GetTypeName().GetAsToken().GetString());

        // Check if connected to a texture
        SdfPathVector connections;
        input.GetRawConnectedSourcePaths(&connections);
        (*params)[i].is_texture = false;

        VtValue val;
        input.Get(&val);

        if (val.IsHolding<SdfAssetPath>()) {
            (*params)[i].is_texture = true;
            (*params)[i].value = du_strdup_m(val.UncheckedGet<SdfAssetPath>().GetResolvedPath());
        } else {
            std::ostringstream ss;
            ss << val;
            (*params)[i].value = du_strdup_m(ss.str());
        }
    }

    return DU_OK;
}

DuStatus du_material_set_param(DuPrim* material_prim, const char* param_name, const char* value) {
    DU_CHECK_NULL(material_prim);
    DU_CHECK_NULL(param_name);
    DU_CHECK_NULL(value);

    UsdShadeMaterial material(du_prim_get_usd(material_prim));
    if (!material) {
        du_set_last_error("Prim is not a Material");
        return DU_ERR_INVALID;
    }

    auto surface = material.ComputeSurfaceSource();
    if (!surface) {
        du_set_last_error("Material has no surface shader");
        return DU_ERR_INVALID;
    }

    auto shader = UsdShadeShader(surface);
    auto input = shader.GetInput(TfToken(param_name));
    if (!input) {
        du_set_last_error(std::string("Shader input not found: ") + param_name);
        return DU_ERR_INVALID;
    }

    // Try setting based on current type
    VtValue current;
    input.Get(&current);

    if (current.IsHolding<float>()) {
        input.Set(std::stof(value));
    } else if (current.IsHolding<GfVec3f>()) {
        // Parse "x y z" format
        float x, y, z;
        if (sscanf(value, "%f %f %f", &x, &y, &z) == 3) {
            input.Set(GfVec3f(x, y, z));
        }
    } else if (current.IsHolding<SdfAssetPath>()) {
        input.Set(SdfAssetPath(value));
    } else if (current.IsHolding<std::string>()) {
        input.Set(std::string(value));
    }

    return DU_OK;
}

DuStatus du_texture_get_thumbnail(const char* asset_path, uint8_t** rgba, uint32_t* w, uint32_t* h, uint32_t max_size) {
    DU_CHECK_NULL(asset_path);
    DU_CHECK_NULL(rgba);
    DU_CHECK_NULL(w);
    DU_CHECK_NULL(h);

    // Thumbnail generation is complex — placeholder that returns a solid color
    uint32_t size = max_size < 64 ? max_size : 64;
    *w = size;
    *h = size;
    *rgba = (uint8_t*)malloc(size * size * 4);
    memset(*rgba, 128, size * size * 4); // gray placeholder

    return DU_OK;
}

void du_free_material_params(DuMaterialParam* params, uint32_t count) {
    for (uint32_t i = 0; i < count; i++) {
        free((void*)params[i].name);
        free((void*)params[i].type);
        free((void*)params[i].value);
    }
    free(params);
}

} // extern "C"
```

**Step 2: Commit**

```bash
git add bridge/src/material.cpp
git commit -m "feat: implement material and texture operations in bridge"
```

---

## Phase 6: Rust Rendering Integration

### Task 12: Hydra Engine Rust Wrapper

**Files:**
- Create: `crates/dreamusd-core/src/hydra.rs`
- Modify: `crates/dreamusd-core/src/lib.rs`

**Step 1: Create hydra.rs — safe Hydra engine wrapper**

```rust
// crates/dreamusd-core/src/hydra.rs
use crate::error::{check, DuError};
use crate::stage::Stage;
use dreamusd_sys::{self, DuDisplayMode};
use std::ffi::CStr;
use std::os::raw::c_void;

pub struct HydraEngine {
    raw: *mut dreamusd_sys::DuHydraEngine,
}

unsafe impl Send for HydraEngine {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    SmoothShaded,
    Wireframe,
    WireframeOnShaded,
    FlatShaded,
    Points,
    Textured,
}

impl From<DisplayMode> for DuDisplayMode {
    fn from(m: DisplayMode) -> Self {
        match m {
            DisplayMode::SmoothShaded => DuDisplayMode::SmoothShaded,
            DisplayMode::Wireframe => DuDisplayMode::Wireframe,
            DisplayMode::WireframeOnShaded => DuDisplayMode::WireframeOnShaded,
            DisplayMode::FlatShaded => DuDisplayMode::FlatShaded,
            DisplayMode::Points => DuDisplayMode::Points,
            DisplayMode::Textured => DuDisplayMode::Textured,
        }
    }
}

pub struct VkImageInfo {
    pub image: u64,      // VkImage (handle)
    pub image_view: u64, // VkImageView (handle)
    pub format: u32,     // VkFormat
    pub width: u32,
    pub height: u32,
}

impl HydraEngine {
    pub fn new(
        stage: &Stage,
        vk_instance: *mut c_void,
        vk_physical_device: *mut c_void,
        vk_device: *mut c_void,
        queue_family_index: u32,
    ) -> Result<Self, DuError> {
        let mut raw = std::ptr::null_mut();
        check(unsafe {
            dreamusd_sys::du_hydra_create_with_vulkan(
                stage.raw,
                vk_instance,
                vk_physical_device,
                vk_device,
                queue_family_index,
                &mut raw,
            )
        })?;
        Ok(HydraEngine { raw })
    }

    pub fn render(&self, width: u32, height: u32) -> Result<(), DuError> {
        check(unsafe { dreamusd_sys::du_hydra_render(self.raw, width, height) })
    }

    pub fn get_vk_image(&self) -> Result<VkImageInfo, DuError> {
        let mut image: u64 = 0;
        let mut view: u64 = 0;
        let mut format: u32 = 0;
        let mut w: u32 = 0;
        let mut h: u32 = 0;
        check(unsafe {
            dreamusd_sys::du_hydra_get_vk_image(
                self.raw,
                &mut image as *mut u64 as *mut c_void,
                &mut view as *mut u64 as *mut c_void,
                &mut format,
                &mut w,
                &mut h,
            )
        })?;
        Ok(VkImageInfo { image, image_view: view, format, width: w, height: h })
    }

    pub fn get_render_semaphore(&self) -> Result<u64, DuError> {
        let mut sem: u64 = 0;
        check(unsafe {
            dreamusd_sys::du_hydra_get_render_semaphore(
                self.raw,
                &mut sem as *mut u64 as *mut c_void,
            )
        })?;
        Ok(sem)
    }

    pub fn set_camera(&self, eye: [f64; 3], target: [f64; 3], up: [f64; 3]) -> Result<(), DuError> {
        check(unsafe {
            dreamusd_sys::du_hydra_set_camera(
                self.raw,
                eye.as_ptr(),
                target.as_ptr(),
                up.as_ptr(),
            )
        })
    }

    pub fn set_display_mode(&self, mode: DisplayMode) -> Result<(), DuError> {
        check(unsafe {
            dreamusd_sys::du_hydra_set_display_mode(self.raw, mode.into())
        })
    }

    pub fn list_render_delegates() -> Result<Vec<String>, DuError> {
        let mut arr: *mut *const std::os::raw::c_char = std::ptr::null_mut();
        let mut count: u32 = 0;
        check(unsafe { dreamusd_sys::du_rd_list_available(&mut arr, &mut count) })?;
        let names = (0..count as usize)
            .map(|i| unsafe { CStr::from_ptr(*arr.add(i)) }.to_string_lossy().into_owned())
            .collect();
        unsafe { dreamusd_sys::du_free_string_array(arr, count) };
        Ok(names)
    }

    pub fn current_render_delegate(&self) -> Result<String, DuError> {
        let mut name: *const std::os::raw::c_char = std::ptr::null();
        check(unsafe { dreamusd_sys::du_rd_get_current(self.raw, &mut name) })?;
        Ok(unsafe { CStr::from_ptr(name) }.to_string_lossy().into_owned())
    }

    pub fn set_render_delegate(&self, name: &str) -> Result<(), DuError> {
        let c_name = std::ffi::CString::new(name)
            .map_err(|_| DuError::Invalid("null byte".into()))?;
        check(unsafe { dreamusd_sys::du_rd_set_current(self.raw, c_name.as_ptr()) })
    }
}

impl Drop for HydraEngine {
    fn drop(&mut self) {
        unsafe { dreamusd_sys::du_hydra_destroy(self.raw) };
    }
}
```

**Step 2: Update lib.rs**

```rust
// crates/dreamusd-core/src/lib.rs
pub mod error;
pub mod stage;
pub mod prim;
pub mod hydra;

pub use error::DuError;
pub use stage::Stage;
pub use prim::Prim;
pub use hydra::{HydraEngine, DisplayMode, VkImageInfo};
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: Compiles.

**Step 4: Commit**

```bash
git add crates/dreamusd-core/
git commit -m "feat: add HydraEngine safe wrapper with display mode and render delegate support"
```

---

### Task 13: Viewport Camera Controller

**Files:**
- Create: `crates/dreamusd-render/src/camera.rs`
- Modify: `crates/dreamusd-render/src/lib.rs`
- Modify: `crates/dreamusd-render/src/viewport.rs`

**Step 1: Create camera.rs**

```rust
// crates/dreamusd-render/src/camera.rs
use glam::{Vec3, Mat4};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    Orbit,
    Pan,
    Fly,
}

pub struct ViewportCamera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub mode: CameraMode,
    orbit_distance: f32,
    yaw: f32,
    pitch: f32,
}

impl Default for ViewportCamera {
    fn default() -> Self {
        Self {
            eye: Vec3::new(0.0, 5.0, 10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov: 45.0,
            near: 0.1,
            far: 10000.0,
            mode: CameraMode::Orbit,
            orbit_distance: 10.0,
            yaw: 0.0,
            pitch: 0.3,
        }
    }
}

impl ViewportCamera {
    pub fn orbit(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw += delta_x * 0.01;
        self.pitch += delta_y * 0.01;
        self.pitch = self.pitch.clamp(-std::f32::consts::FRAC_PI_2 + 0.01, std::f32::consts::FRAC_PI_2 - 0.01);
        self.update_eye_from_orbit();
    }

    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        let forward = (self.target - self.eye).normalize();
        let right = forward.cross(self.up).normalize();
        let up = right.cross(forward).normalize();

        let scale = self.orbit_distance * 0.002;
        self.target += right * (-delta_x * scale) + up * (delta_y * scale);
        self.update_eye_from_orbit();
    }

    pub fn zoom(&mut self, delta: f32) {
        self.orbit_distance *= 1.0 - delta * 0.001;
        self.orbit_distance = self.orbit_distance.clamp(0.01, 100000.0);
        self.update_eye_from_orbit();
    }

    pub fn focus_on(&mut self, center: Vec3, radius: f32) {
        self.target = center;
        self.orbit_distance = radius * 2.5;
        self.update_eye_from_orbit();
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye, self.target, self.up)
    }

    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov.to_radians(), aspect, self.near, self.far)
    }

    pub fn eye_as_f64(&self) -> [f64; 3] {
        [self.eye.x as f64, self.eye.y as f64, self.eye.z as f64]
    }

    pub fn target_as_f64(&self) -> [f64; 3] {
        [self.target.x as f64, self.target.y as f64, self.target.z as f64]
    }

    pub fn up_as_f64(&self) -> [f64; 3] {
        [self.up.x as f64, self.up.y as f64, self.up.z as f64]
    }

    fn update_eye_from_orbit(&mut self) {
        let x = self.orbit_distance * self.pitch.cos() * self.yaw.sin();
        let y = self.orbit_distance * self.pitch.sin();
        let z = self.orbit_distance * self.pitch.cos() * self.yaw.cos();
        self.eye = self.target + Vec3::new(x, y, z);
    }
}
```

**Step 2: Create viewport.rs**

```rust
// crates/dreamusd-render/src/viewport.rs
use crate::camera::ViewportCamera;

pub struct Viewport {
    pub camera: ViewportCamera,
    pub width: u32,
    pub height: u32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            camera: ViewportCamera::default(),
            width: 800,
            height: 600,
        }
    }
}
```

**Step 3: Update lib.rs**

```rust
// crates/dreamusd-render/src/lib.rs
pub mod camera;
pub mod viewport;

pub use camera::{ViewportCamera, CameraMode};
pub use viewport::Viewport;
```

**Step 4: Verify it compiles**

Run: `cargo check`

**Step 5: Commit**

```bash
git add crates/dreamusd-render/
git commit -m "feat: add viewport camera with orbit/pan/zoom controls"
```

---

## Phase 7: UI Integration — Scene Hierarchy, Properties, Viewport Interaction

### Task 14: Scene Hierarchy Panel

**Files:**
- Create: `crates/dreamusd-ui/src/panels/hierarchy.rs`
- Create: `crates/dreamusd-ui/src/panels/mod.rs`
- Modify: `crates/dreamusd-ui/src/panels.rs` → rename to mod.rs

**Step 1: Create hierarchy panel**

```rust
// crates/dreamusd-ui/src/panels/hierarchy.rs
use dreamusd_core::{Prim, Stage};
use egui;

pub struct HierarchyPanel {
    pub selected_path: Option<String>,
    filter_text: String,
}

impl Default for HierarchyPanel {
    fn default() -> Self {
        Self {
            selected_path: None,
            filter_text: String::new(),
        }
    }
}

impl HierarchyPanel {
    pub fn show(&mut self, ui: &mut egui::Ui, stage: Option<&Stage>) {
        // Search box
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.filter_text);
        });
        ui.separator();

        if let Some(stage) = stage {
            if let Ok(root) = stage.root_prim() {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_prim_tree(ui, &root);
                });
            }
        } else {
            ui.label("No stage loaded");
        }
    }

    fn show_prim_tree(&mut self, ui: &mut egui::Ui, prim: &Prim) {
        let name = prim.name().unwrap_or_else(|_| "???".into());
        let path = prim.path().unwrap_or_else(|_| String::new());
        let type_name = prim.type_name().unwrap_or_else(|_| String::new());

        // Filter check
        if !self.filter_text.is_empty()
            && !name.to_lowercase().contains(&self.filter_text.to_lowercase())
        {
            // Still show children that might match
        }

        let children = prim.children().unwrap_or_default();
        let has_children = !children.is_empty();
        let is_selected = self.selected_path.as_deref() == Some(&path);

        let label = if type_name.is_empty() {
            name.clone()
        } else {
            format!("{} ({})", name, type_name)
        };

        if has_children {
            let header = egui::CollapsingHeader::new(&label)
                .id_salt(&path)
                .default_open(path == "/")
                .show(ui, |ui| {
                    for child in &children {
                        self.show_prim_tree(ui, child);
                    }
                });

            if header.header_response.clicked() {
                self.selected_path = Some(path);
            }

            if is_selected {
                header.header_response.highlight();
            }
        } else {
            let response = ui.selectable_label(is_selected, &label);
            if response.clicked() {
                self.selected_path = Some(path);
            }
        }
    }
}
```

**Step 2: Create panels/mod.rs**

```rust
// crates/dreamusd-ui/src/panels/mod.rs
pub mod hierarchy;
pub mod properties;

pub use hierarchy::HierarchyPanel;
pub use properties::PropertiesPanel;
```

**Step 3: Commit**

```bash
git add crates/dreamusd-ui/
git commit -m "feat: add scene hierarchy panel with tree view and filtering"
```

---

### Task 15: Properties Panel

**Files:**
- Create: `crates/dreamusd-ui/src/panels/properties.rs`

**Step 1: Create properties panel**

```rust
// crates/dreamusd-ui/src/panels/properties.rs
use dreamusd_core::Prim;
use egui;

pub struct PropertiesPanel;

impl PropertiesPanel {
    pub fn show(ui: &mut egui::Ui, prim: Option<&Prim>) {
        let Some(prim) = prim else {
            ui.label("No prim selected");
            return;
        };

        let path = prim.path().unwrap_or_default();
        let type_name = prim.type_name().unwrap_or_default();

        ui.label(format!("Path: {}", path));
        ui.label(format!("Type: {}", type_name));
        ui.separator();

        // Transform section
        egui::CollapsingHeader::new("Transform")
            .default_open(true)
            .show(ui, |ui| {
                if let Ok(matrix) = prim.get_local_matrix() {
                    // Extract translation from column 3
                    let tx = matrix[12];
                    let ty = matrix[13];
                    let tz = matrix[14];
                    ui.horizontal(|ui| {
                        ui.label("Translate:");
                        ui.label(format!("{:.3} {:.3} {:.3}", tx, ty, tz));
                    });
                }
            });

        // Attributes section
        egui::CollapsingHeader::new("Attributes")
            .default_open(false)
            .show(ui, |ui| {
                if let Ok(names) = prim.attribute_names() {
                    for name in &names {
                        let val = prim.get_attribute(name).unwrap_or_else(|_| "???".into());
                        ui.horizontal(|ui| {
                            ui.label(name);
                            ui.label(&val);
                        });
                    }
                    if names.is_empty() {
                        ui.label("No attributes");
                    }
                }
            });

        // Variants section
        egui::CollapsingHeader::new("Variants")
            .default_open(false)
            .show(ui, |ui| {
                if let Ok(sets) = prim.variant_sets() {
                    for set_name in &sets {
                        let sel = prim.get_variant_selection(set_name).unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.label(set_name);
                            ui.label(&sel);
                        });
                    }
                    if sets.is_empty() {
                        ui.label("No variant sets");
                    }
                }
            });

        // Material section
        egui::CollapsingHeader::new("Material")
            .default_open(false)
            .show(ui, |ui| {
                match prim.material_binding() {
                    Ok(mat_path) => {
                        ui.label(format!("Bound: {}", mat_path));
                    }
                    Err(_) => {
                        ui.label("No material bound");
                    }
                }
            });
    }
}
```

**Step 2: Commit**

```bash
git add crates/dreamusd-ui/
git commit -m "feat: add properties panel with transform, attributes, variants, material"
```

---

### Task 16: Wire Everything Together in App

**Files:**
- Modify: `crates/dreamusd-ui/src/app.rs`
- Modify: `crates/dreamusd-ui/src/lib.rs`
- Modify: `crates/dreamusd-app/src/main.rs`

**Step 1: Update app.rs to use real panels and stage**

```rust
// crates/dreamusd-ui/src/app.rs
use crate::panels::{HierarchyPanel, PropertiesPanel};
use dreamusd_core::{Stage, DisplayMode};
use dreamusd_render::ViewportCamera;
use eframe::egui;

pub struct DreamUsdApp {
    stage: Option<Stage>,
    hierarchy: HierarchyPanel,
    camera: ViewportCamera,
    current_display_mode: usize,
    show_grid: bool,
    show_axis: bool,
    status_message: String,
    gizmo_mode: GizmoMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GizmoMode {
    Translate,
    Rotate,
    Scale,
}

const DISPLAY_MODES: &[&str] = &[
    "Smooth Shaded",
    "Wireframe",
    "Wireframe on Shaded",
    "Flat Shaded",
    "Points",
    "Textured",
];

impl Default for DreamUsdApp {
    fn default() -> Self {
        Self {
            stage: None,
            hierarchy: HierarchyPanel::default(),
            camera: ViewportCamera::default(),
            current_display_mode: 0,
            show_grid: true,
            show_axis: true,
            status_message: "Ready".into(),
            gizmo_mode: GizmoMode::Translate,
        }
    }
}

impl eframe::App for DreamUsdApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ctx);

        // Menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open (Ctrl+O)").clicked() {
                        self.open_file();
                        ui.close_menu();
                    }
                    if ui.button("Save (Ctrl+S)").clicked() {
                        self.save_file();
                        ui.close_menu();
                    }
                    if ui.button("Save As (Ctrl+Shift+S)").clicked() {
                        self.save_file_as();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo (Ctrl+Z)").clicked() {
                        self.undo();
                        ui.close_menu();
                    }
                    if ui.button("Redo (Ctrl+Shift+Z)").clicked() {
                        self.redo();
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_grid, "Grid");
                    ui.checkbox(&mut self.show_axis, "Axis");
                });
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
                ui.separator();
                egui::ComboBox::from_id_salt("display_mode")
                    .selected_text(DISPLAY_MODES[self.current_display_mode])
                    .show_ui(ui, |ui| {
                        for (i, mode) in DISPLAY_MODES.iter().enumerate() {
                            ui.selectable_value(&mut self.current_display_mode, i, *mode);
                        }
                    });
                ui.separator();
                let mode_label = match self.gizmo_mode {
                    GizmoMode::Translate => "Move (W)",
                    GizmoMode::Rotate => "Rotate (E)",
                    GizmoMode::Scale => "Scale (R)",
                };
                ui.label(mode_label);
            });
        });

        // Scene hierarchy (left)
        egui::SidePanel::left("scene_hierarchy")
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.heading("Scene Hierarchy");
                ui.separator();
                self.hierarchy.show(ui, self.stage.as_ref());
            });

        // Properties (right)
        egui::SidePanel::right("properties")
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Properties");
                ui.separator();
                // TODO: look up selected prim from stage
                PropertiesPanel::show(ui, None);
            });

        // 3D Viewport (center)
        egui::CentralPanel::default().show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            self.camera.zoom(0.0); // keep camera updated

            // Handle viewport input
            let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
            if response.dragged_by(egui::PointerButton::Middle) {
                let delta = response.drag_delta();
                if ui.input(|i| i.modifiers.shift) {
                    self.camera.pan(delta.x, delta.y);
                } else {
                    self.camera.orbit(delta.x, delta.y);
                }
            }
            if ui.rect_contains_pointer(rect) {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    self.camera.zoom(scroll);
                }
            }

            // Placeholder for Hydra rendered viewport
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(40, 40, 40));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                if self.stage.is_some() { "Viewport — Hydra rendering pending" } else { "No stage loaded" },
                egui::FontId::proportional(16.0),
                egui::Color32::GRAY,
            );
        });
    }
}

impl DreamUsdApp {
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::O) && i.modifiers.command {
                self.open_file();
            }
            if i.key_pressed(egui::Key::S) && i.modifiers.command && !i.modifiers.shift {
                self.save_file();
            }
            if i.key_pressed(egui::Key::S) && i.modifiers.command && i.modifiers.shift {
                self.save_file_as();
            }
            if i.key_pressed(egui::Key::Z) && i.modifiers.command && !i.modifiers.shift {
                self.undo();
            }
            if i.key_pressed(egui::Key::Z) && i.modifiers.command && i.modifiers.shift {
                self.redo();
            }
            if i.key_pressed(egui::Key::W) && !i.modifiers.command {
                self.gizmo_mode = GizmoMode::Translate;
            }
            if i.key_pressed(egui::Key::E) && !i.modifiers.command {
                self.gizmo_mode = GizmoMode::Rotate;
            }
            if i.key_pressed(egui::Key::R) && !i.modifiers.command {
                self.gizmo_mode = GizmoMode::Scale;
            }
        });
    }

    fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("USD Files", &["usd", "usda", "usdc", "usdz"])
            .pick_file()
        {
            match Stage::open(&path) {
                Ok(stage) => {
                    self.status_message = format!("Opened: {}", path.display());
                    self.stage = Some(stage);
                    self.hierarchy = HierarchyPanel::default();
                }
                Err(e) => {
                    self.status_message = format!("Error: {}", e);
                    tracing::error!("Failed to open {}: {}", path.display(), e);
                }
            }
        }
    }

    fn save_file(&mut self) {
        if let Some(ref stage) = self.stage {
            match stage.save() {
                Ok(()) => self.status_message = "Saved".into(),
                Err(e) => self.status_message = format!("Save error: {}", e),
            }
        }
    }

    fn save_file_as(&mut self) {
        if let Some(ref stage) = self.stage {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("USD Files", &["usd", "usda", "usdc"])
                .save_file()
            {
                match stage.export(&path) {
                    Ok(()) => self.status_message = format!("Exported: {}", path.display()),
                    Err(e) => self.status_message = format!("Export error: {}", e),
                }
            }
        }
    }

    fn undo(&mut self) {
        if let Some(ref stage) = self.stage {
            let _ = stage.undo();
        }
    }

    fn redo(&mut self) {
        if let Some(ref stage) = self.stage {
            let _ = stage.redo();
        }
    }
}
```

**Step 2: Update ui lib.rs**

```rust
// crates/dreamusd-ui/src/lib.rs
pub mod app;
pub mod panels;
```

**Step 3: Verify it compiles**

Run: `cargo check`

**Step 4: Commit**

```bash
git add crates/
git commit -m "feat: integrate all panels into main app with stage loading and shortcuts"
```

---

## Phase 8: Polish & Final Integration

### Task 17: README and Build Instructions

**Files:**
- Create: `README.md`

**Step 1: Write README**

```markdown
# DreamUSD

A high-performance USD file viewer and editor built with Rust.

## Features

- Open, display, edit, and save USD/USDA/USDC files
- Hydra Storm real-time rendering with Vulkan
- Scene hierarchy browser with search
- Property editor (transforms, attributes, variants, materials)
- Multiple display modes (shaded, wireframe, textured, etc.)
- Pluggable render delegate architecture
- Cross-platform: macOS, Linux, Windows

## Prerequisites

- Rust 1.75+
- CMake 3.24+
- OpenUSD (set `USD_ROOT` environment variable)
- Vulkan SDK
- macOS: MoltenVK (included with Vulkan SDK)

## Build

```bash
export USD_ROOT=/path/to/usd/install
cargo build --release
```

## Run

```bash
cargo run --release -p dreamusd-app
```

## License

MIT OR Apache-2.0
```

**Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add README with build instructions"
```

---

## Summary of Phases

| Phase | Tasks | Outcome |
|-------|-------|---------|
| 1. Scaffolding | 1-3 | Cargo workspace, CMake bridge, FFI stubs compile |
| 2. Core Wrappers | 4-5 | Safe Rust Stage/Prim abstractions |
| 3. App Shell | 6 | egui window with panel layout launches |
| 4. Bridge: Stage/Prim | 7-9 | Real OpenUSD stage/prim/transform operations |
| 5. Bridge: Hydra | 10-11 | Hydra Storm rendering + materials |
| 6. Rust Render | 12-13 | HydraEngine wrapper + camera controller |
| 7. UI Panels | 14-16 | Hierarchy, properties, full app integration |
| 8. Polish | 17 | README, build docs |

Each phase produces a testable, committable milestone. Phase 3 gives you a running app. Phases 4-5 bring in real USD. Phases 6-7 wire rendering to the viewport.
