use std::env;
use std::path::PathBuf;

fn main() {
    let bridge_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../../bridge");

    let dst = cmake::build(&bridge_dir);

    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=dreamusd_bridge");

    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=dylib=c++");
    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=dylib=stdc++");

    println!("cargo:rerun-if-changed=../../bridge/src");
    println!("cargo:rerun-if-changed=../../bridge/include");
    println!("cargo:rerun-if-changed=../../bridge/CMakeLists.txt");
}
