use std::env;
use std::path::Path;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(videocut_media_present)");
    let manifest_dir =
        Path::new(&env::var("CARGO_MANIFEST_DIR").expect("manifest dir")).to_path_buf();
    let media = manifest_dir.join("../../../crates/media/Cargo.toml");
    println!("cargo:rerun-if-changed={}", media.display());
    if media.is_file() {
        println!("cargo:rustc-cfg=videocut_media_present");
    }
    println!("cargo:rerun-if-changed=tauri.conf.json");
    if env::var_os("CARGO_FEATURE_GUI").is_some() {
        tauri_build::build();
    }
}
