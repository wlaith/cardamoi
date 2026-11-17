fn main() {
    let mut attributes = tauri_build::Attributes::new();
    let target = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|e| e == "msvc");
    if target == "windows" && msvc {
        attributes = attributes.windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        // A manifest the linker can't use is an error, not a silent fallback.
        println!("cargo:rustc-link-arg=/WX");
    }
    tauri_build::try_build(attributes).expect("tauri-build failed");
}
