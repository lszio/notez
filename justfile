# notez — quick-launch recipes for every surface.
#
# Usage:
#   just                  # list available recipes
#   just web              # Dioxus SSR development server with hot reload
#   just app              # desktop client (auto-detects platform)
#   just ios              # iOS simulator (macOS host required)
#   just android          # Android emulator / device
#
# All recipes accept common overrides:
#   just web PORT=3030 IP=127.0.0.1
#   just app-linux FEATURES="server,desktop"

set shell := ["bash", "-uc"]
set dotenv-load := false
set positional-arguments

# ---- Settings ----------------------------------------------------------------

# Build profile for cargo targets. Override per-invocation: `just web RELEASE=1`.
release := env_var_or_default("RELEASE", "0")

# Space root consumed by the web SSR server. Optional — the web client picks
# the space at runtime through its picker, so this is only a default hint.
space := env_var_or_default("NOTEZ_SPACE_ROOT", "")

# Default bind address for `just web`.
host := env_var_or_default("HOST", "127.0.0.1")
port := env_var_or_default("PORT", "8765")

# Extra features for packages that define them (desktop / mobile).
features := env_var_or_default("FEATURES", "")

# Package directories.
cli_pkg     := "cli"
web_pkg     := "web"
desktop_dir := "packages/desktop"
mobile_dir  := "packages/mobile"

# ---- Helpers -----------------------------------------------------------------

# A `cargo build`/`run` invocation that picks debug vs release.
_cargo_profile := if release == "1" { "--release" } else { "" }

# _features_flag "<csv>" → "--features <csv>" when non-empty, else "".
_features_flag features:
    @if [ -n "{{features}}" ]; then printf -- "--features %s" "{{features}}"; else printf -- ""; fi


# Detect host OS. Echoes one of: linux | macos | windows | unknown.
os:
    #!/usr/bin/env bash
    case "$(uname -s)" in
        Linux)   printf "linux\n" ;;
        Darwin)  printf "macos\n" ;;
        MINGW*|MSYS*|CYGWIN*) printf "windows\n" ;;
        *)       printf "unknown\n" ;;
    esac

# True when the host can open a desktop window (X11 or Wayland session).
has_display:
    #!/usr/bin/env bash
    if [ -n "${WAYLAND_DISPLAY:-}" ] || [ -n "${DISPLAY:-}" ]; then
        printf "yes\n"
    else
        printf "no\n"
    fi

# Find the Dioxus CLI; fail-fast with install hint if missing.
_require_dx:
    #!/usr/bin/env bash
    if ! command -v dx >/dev/null 2>&1; then
        echo "dx (Dioxus CLI) not found on PATH." >&2
        echo "Install with: cargo install dioxus-cli --locked" >&2
        exit 127
    fi

# ---- Default target ----------------------------------------------------------

# List recipes (default action).
default:
    @just --list --unsorted

# ---- CLI ---------------------------------------------------------------------

# Build and run the notez CLI (debug).
cli *args:
    cargo {{_cargo_profile}} run -p {{cli_pkg}} --bin notez -- {{args}}

# Build and run the notez CLI (release).
cli-release *args:
    cargo --release run -p {{cli_pkg}} --bin notez -- {{args}}

# Run the CLI with the `web` cargo feature (the `notez web` subcommand is currently a no-op stub; this keeps the recipe consistent with the spec).
cli-web *args:
    cargo {{_cargo_profile}} run -p {{cli_pkg}} --bin notez --features web -- {{args}}

# Just build the CLI binary without running it.
cli-build:
    cargo {{_cargo_profile}} build -p {{cli_pkg}} --bin notez

# ---- Web development --------------------------------------------------------
#
# `just web` launches the browser target. The Dioxus CLI owns watching,
# rebuilding, and serving the Web bundle.

web:
    #!/usr/bin/env bash
    set -euo pipefail
    exec dx serve --platform web --package {{web_pkg}} --bin {{web_pkg}} --addr "{{host}}" --port "{{port}}"

# Production-like SSR launch with the custom public shell.
web-prod:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -n "{{space}}" ]; then export NOTEZ_SPACE_ROOT="{{space}}"; fi
    export IP="{{host}}"
    export PORT="{{port}}"
    exec cargo {{_cargo_profile}} run -p {{web_pkg}} --bin {{web_pkg}}

# Explicit Web development alias.
web-dx:
    #!/usr/bin/env bash
    set -euo pipefail
    exec dx serve --platform web --package {{web_pkg}} --bin {{web_pkg}} --addr "{{host}}" --port "{{port}}"

#

# Build the web client only (no run).
web-build:
    cargo {{_cargo_profile}} build -p {{web_pkg}} --bin {{web_pkg}}

# ---- Desktop app (auto-detect platform) -------------------------------------

# Run the desktop client. Auto-detects host OS; on Linux falls back to a headless build when no display server is detected.
app: _app_run

# Force-run the desktop client built for the current Linux host.
app-linux: _app_linux

# Force-run the desktop client on macOS.
app-macos: _app_macos

# Force-run the desktop client on Windows.
app-windows: _app_windows

# Build the desktop binary for the current host without running it.
app-build:
    cargo {{_cargo_profile}} build -p desktop --bin desktop {{ if features != "" { "--features " + features } else { "" } }}

# ---- Mobile (per-target recipes) --------------------------------------------

# Open the iOS simulator (macOS host required).
ios: _ios_run

# Open the Android emulator / attached device.
android: _android_run

# ---- Workspace utilities -----------------------------------------------------

# cargo check across the workspace.
check:
    cargo check --workspace --all-targets

# Run the workspace test suite.
test:
    cargo test --workspace

# Run clippy with workspace lints.
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Format the workspace in place.
fmt:
    cargo fmt --all

# Remove the cargo target/ directory.
clean:
    cargo clean

# Build every binary target (debug).
build:
    cargo build --workspace --bins

# Create a minimal notez space at PATH (default ./demo-space); pass `--scan` to also run `notez scan`.
seed-space path="demo-space" *extra:
    #!/usr/bin/env bash
    set -euo pipefail
    target="{{path}}"
    mkdir -p "$target"
    name="$(basename "$target")"
    cat > "$target/notez.toml" <<EOF
    version = 1

    [space]
    name = "$name"
    database = ".notez/index.sqlite"
    EOF
    mkdir -p "$target/.notez"
    echo "seeded space at $target (name=$name)"
    if [ "{{extra}}" = "--scan" ]; then
        just cli --space "$target" scan
    fi

# Show what the web picker would auto-discover from $HOME / cwd right now.
discover-spaces:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo run -q -p core --example smoke_discover

# ---- Acceptance helpers ------------------------------------------------------

# Run the web acceptance script (builds CLI + web, exercises SSR endpoints).
acceptance-web:
    bash scripts/acceptance-web.sh

# Run every acceptance-* script under scripts/.
acceptance:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "$(dirname "{{justfile()}}")"
    for s in scripts/acceptance-*.sh; do
        echo "==> $s"
        bash "$s"
    done

# ---- Private platform selectors ----------------------------------------------

# Pick the right desktop launcher based on host OS.
_app_run: _require_dx
    #!/usr/bin/env bash
    case "$(just os)" in
        linux)   exec just _app_linux ;;
        macos)   exec just _app_macos ;;
        windows) exec just _app_windows ;;
        *) echo "unsupported host: $(uname -s)" >&2; exit 1 ;;
    esac

# Linux desktop: prefer `dx serve` when a display is available; otherwise build
# only (headless CI, containers, SSH sessions).
_app_linux: _require_dx
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{desktop_dir}}"
    if [ "$(just has_display)" = "yes" ]; then
        echo "→ launching desktop (linux, display detected)"
        exec dx serve {{ if features != "" { "--features " + features } else { "" } }}
    else
        echo "→ no display server detected; building desktop binary only"
        cargo {{_cargo_profile}} build -p desktop --bin desktop {{ if features != "" { "--features " + features } else { "" } }}
    fi

# macOS desktop launcher.
_app_macos: _require_dx
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{desktop_dir}}"
    echo "→ launching desktop (macos)"
    exec dx serve {{ if features != "" { "--features " + features } else { "" } }}

# Windows desktop launcher (Git-Bash / MSYS / WSL).
_app_windows: _require_dx
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{desktop_dir}}"
    echo "→ launching desktop (windows)"
    exec dx serve {{ if features != "" { "--features " + features } else { "" } }}

# iOS: requires macOS host with Xcode + iOS simulator.
_ios_run: _require_dx
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "$(just os)" != "macos" ]; then
        echo "iOS builds require a macOS host (got $(uname -s))." >&2
        exit 1
    fi
    cd "{{mobile_dir}}"
    echo "→ launching ios simulator"
    exec dx serve --platform ios

# Android: needs adb + an emulator or attached device.
_android_run: _require_dx
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{mobile_dir}}"
    if ! command -v adb >/dev/null 2>&1; then
        echo "adb not found on PATH." >&2
        echo "Install Android platform-tools or set ANDROID_HOME." >&2
        exit 127
    fi
    echo "→ launching android (target $(adb devices | sed -n '2p' | awk '{print $2}' || echo unknown))"
    exec dx serve --platform android
