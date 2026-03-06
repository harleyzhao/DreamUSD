fn main() {
    let usd_root = std::env::var("USD_ROOT").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{}/opt/openusd", home)
    });
    let usd_lib = format!("{}/lib", usd_root);
    if std::path::Path::new(&usd_lib).exists() {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", usd_lib);
    }
}
