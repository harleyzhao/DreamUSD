use std::env;
use std::path::PathBuf;

fn main() {
    let bridge_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../../bridge");

    let mut cmake_cfg = cmake::Config::new(&bridge_dir);

    // Pass USD_ROOT to CMake if available
    if let Ok(usd_root) = env::var("USD_ROOT") {
        cmake_cfg.define("USD_ROOT", &usd_root);
    } else {
        // Default to ~/opt/openusd if it exists
        let home = env::var("HOME").unwrap_or_default();
        let default_usd = format!("{}/opt/openusd", home);
        if std::path::Path::new(&default_usd).exists() {
            cmake_cfg.define("USD_ROOT", &default_usd);
            env::set_var("USD_ROOT", &default_usd);
        }
    }

    let dst = cmake_cfg.build();

    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=dreamusd_bridge");

    // Link USD dynamic libraries if USD_ROOT is set
    if let Ok(usd_root) = env::var("USD_ROOT") {
        let usd_lib_dir = format!("{}/lib", usd_root);
        println!("cargo:rustc-link-search=native={}", usd_lib_dir);

        // Link all USD dylibs
        if let Ok(entries) = std::fs::read_dir(&usd_lib_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if cfg!(target_os = "macos") && name.starts_with("libusd_") && name.ends_with(".dylib") {
                    let lib_name = name.strip_prefix("lib").unwrap().strip_suffix(".dylib").unwrap();
                    println!("cargo:rustc-link-lib=dylib={}", lib_name);
                } else if cfg!(target_os = "linux") && name.starts_with("libusd_") && name.ends_with(".so") {
                    let lib_name = name.strip_prefix("lib").unwrap().strip_suffix(".so").unwrap();
                    println!("cargo:rustc-link-lib=dylib={}", lib_name);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=dylib=c++");
        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=QuartzCore");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=IOKit");
        println!("cargo:rustc-link-lib=framework=AppKit");
    }
    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=dylib=stdc++");

    println!("cargo:rerun-if-changed=../../bridge/src");
    println!("cargo:rerun-if-changed=../../bridge/include");
    println!("cargo:rerun-if-changed=../../bridge/CMakeLists.txt");
    println!("cargo:rerun-if-env-changed=USD_ROOT");
}
