fn main() {
    let target =
        if   cfg!(target_arch = "wasm32") { "wasm32 (web)" }
        else if cfg!(target_os = "windows") { "windows (desktop)" }
        else if cfg!(target_os = "macos")   { "macos (desktop)" }
        else if cfg!(target_os = "linux")   { "linux (desktop)" }
        else if cfg!(target_os = "ios")     { "ios (mobile)" }
        else if cfg!(target_os = "android") { "android (mobile)" }
        else { "headless" };
    println!("notez_server target = {target}");
}
