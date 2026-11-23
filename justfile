# The gates of AGENTS.md. `just check`, `just test`, `just test-tsx`; `just gates` runs all three.
# nsh is the nightly kept in this repository, never the one on PATH.

set shell := ["bash", "-euo", "pipefail", "-c"]

nsh := justfile_directory() / ".nsh-nightly/nsh/bin/nsh"
export NSH_HOME := justfile_directory() / ".nsh-home"

# List the recipes.
default:
    @just --list

# Format and Clippy, warnings as errors.
check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings

# The Rust tests, on this machine's GPU.
test:
    cargo test --workspace

# nsh check, nsh build (a warning fails), nsh test, in an app directory.
test-tsx app="examples/headless":
    mkdir -p "$NSH_HOME"
    cd {{ app }} && {{ nsh }} check
    cd {{ app }} && {{ nsh }} build 2>&1 | tee "$NSH_HOME/build.log"
    ! grep -i warn "$NSH_HOME/build.log"
    cd {{ app }} && {{ nsh }} test

# Clippy for a cross target (CI runs wasm32-unknown-unknown, aarch64-linux-android, aarch64-apple-ios-sim).
cross target:
    cargo clippy -p nsh-webgpu --target {{ target }} {{ if target == "wasm32-unknown-unknown" { "" } else { "--all-targets" } }} -- -D warnings

# Format the code.
fmt:
    cargo fmt --all

# Every gate, in CI's order.
gates: check test test-tsx
