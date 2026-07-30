//! Build script: copies `static/` (any file under `crates/web/static/`)
//! into `OUT_DIR/static/` so the binary can `include_str!` them at
//! compile time. Any change inside `static/` triggers a rebuild.

use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=static");

    let out_dir = match std::env::var("OUT_DIR") {
        Ok(v) => Path::new(&v).to_path_buf(),
        Err(e) => {
            eprintln!("warning: OUT_DIR not set ({e}); skipping static/ copy");
            return;
        }
    };
    let out_static = out_dir.join("static");
    if let Err(err) = copy_dir(Path::new("static"), &out_static) {
        // We never want the build to fail because of static file
        // management — emit a warning and continue. The static_file
        // route will simply 404 for missing assets.
        eprintln!("warning: failed to copy static/ to OUT_DIR: {err}");
    }
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}