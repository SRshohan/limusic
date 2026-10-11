// libmpv2-sys only emits `cargo:rustc-link-lib=mpv`, with no `-L` search path, so it
// relies on mpv's `.dylib`/`.so` already sitting in a directory the linker searches by
// default. That's true for `/usr/local/lib` (Intel Mac Homebrew) and `/usr/lib` (most
// Linux distros), but not for `/opt/homebrew/lib` (Apple Silicon Homebrew), so an
// otherwise correct install fails with "library 'mpv' not found". Point the linker at
// wherever pkg-config (or, failing that, a known Homebrew prefix) actually finds it.
fn main() {
    if pkg_config::Config::new().probe("mpv").is_ok() {
        return;
    }

    if cfg!(target_os = "macos") {
        for prefix in ["/opt/homebrew/lib", "/usr/local/lib"] {
            if std::path::Path::new(prefix).exists() {
                println!("cargo:rustc-link-search=native={prefix}");
            }
        }
    }
}
