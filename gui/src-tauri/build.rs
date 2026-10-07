fn main() {
    postkit_ffmpeg_link_search::emit_ffmpeg_link_search();
    // ld64 keeps a load command for every linked dylib, used or not
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-dead_strip_dylibs");
    }
    tauri_build::build()
}
