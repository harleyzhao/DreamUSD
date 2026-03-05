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

## Architecture

Rust application shell using egui/wgpu communicates with OpenUSD/Hydra C++ via a C ABI bridge layer. Storm renders to a VkImage shared with wgpu for zero-copy display.

```
Rust (egui + wgpu/Vulkan) <-- FFI --> C ABI Bridge <--> OpenUSD/Hydra C++
```

## Project Structure

- `crates/dreamusd-app` - Main application
- `crates/dreamusd-ui` - egui UI panels
- `crates/dreamusd-core` - Safe Rust wrappers for USD/Hydra
- `crates/dreamusd-render` - wgpu/Vulkan viewport rendering
- `crates/dreamusd-sys` - Raw FFI bindings
- `bridge/` - C++ bridge code wrapping OpenUSD

## License

MIT OR Apache-2.0
