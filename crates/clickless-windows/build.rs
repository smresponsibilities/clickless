// Compiles settings-manifest.rc (RT_MANIFEST resource 1) into every binary
// that links this crate. The resource activates comctl32 v6 visual styles,
// per-monitor v2 DPI awareness, and the UTF-8 code page for the Settings UI.
fn main() {
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        let _ = embed_resource::compile("settings-manifest.rc", embed_resource::NONE);
    }
    println!("cargo:rerun-if-changed=settings-manifest.rc");
    println!("cargo:rerun-if-changed=clickless-settings.manifest");
}
