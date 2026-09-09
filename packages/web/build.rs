use std::{env, fs, path::Path};

/// Copy `packages/web/public/` into `<target>/<profile>/public/` so the
/// Dioxus SSR renderer finds its template next to the binary.
///
/// `serve_dioxus_application` reads `<bin dir>/public` and panics when
/// the directory is missing, so this is required for `cargo run` as
/// well as for release builds (the Dockerfile copies the directory
/// explicitly).
/// FNV-1a over the embedded assets: the URL version changes exactly
/// when the CSS/JS changes, so browsers never serve a stale island.
fn asset_version(manifest: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for name in ["src/data/app.js", "../../packages/ui/assets/workspace.css"] {
        let path = Path::new(manifest).join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        let Ok(bytes) = fs::read(&path) else { continue };
        for byte in bytes {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    println!("cargo:rustc-env=NOTEZ_ASSET_VERSION={:08x}", asset_version(&manifest));
    let src = Path::new(&manifest).join("public");
    if !src.exists() {
        return;
    }

    let target = env::var("CARGO_TARGET_DIR")
        .unwrap_or_else(|_| Path::new(&manifest).join("../../target").display().to_string());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let dest = Path::new(&target).join(&profile).join("public");

    if let Err(e) = copy_dir(&src, &dest) {
        eprintln!("notez-web build.rs: failed to sync public dir: {e}");
    }
    println!("cargo:rerun-if-changed={}", src.display());
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let p = entry.path();
        let d = dst.join(entry.file_name());
        if entry.metadata()?.is_dir() {
            copy_dir(&p, &d)?;
        } else {
            fs::copy(&p, &d)?;
        }
    }
    Ok(())
}
