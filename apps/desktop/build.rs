fn main() {
    // D-33 (audit lot 4, spike 2026-09-07): `generate_context!` embeds the
    // dist at macro expansion, and tauri-build only watches the dist under
    // its optional `codegen` feature. Without this line a file ADDED to the
    // dist (a renamed Vite asset) is invisible to a bare `cargo build`; with
    // it the build script reruns, main.rs recompiles and the dist is
    // re-embedded. A no-op build stays at its 0.5 s floor.
    println!("cargo:rerun-if-changed=ui-v2/dist");
    tauri_build::build()
}
