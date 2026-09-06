use std::{env, fs, path::Path};

/// Copy `packages/web/public/` into `<target>/<profile>/public/` so
/// `dioxus::serve` finds the static shell next to the binary in dev
/// (`cargo run`) mode. Production builds use the Docker
/// `COPY --from=builder` step instead; this keeps dev parity without
/// shipping the asset pipeline to users.
fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
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
        let meta = entry.metadata()?;
        if meta.is_dir() {
            copy_dir(&p, &d)?;
        } else {
            fs::copy(&p, &d)?;
        }
    }
    Ok(())
}
