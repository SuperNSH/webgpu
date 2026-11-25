# AGENTS.md

Goal: a 1:1 Native Shell (NSH) port of [react-native-webgpu](https://github.com/wcandillon/react-native-webgpu), at
commit [`06599f9`](https://github.com/wcandillon/react-native-webgpu/tree/06599f9) (package in `packages/webgpu`,
example app in `apps/example`). This repository is `github.com/SuperNSH/webgpu`. The plan, API inventory and phase
log are in [docs/PLAN.md](docs/PLAN.md); the area owners are in its "Ownership" section.

## Toolchain

- NSH is the **release 0.7.1**, its CLI kept inside the checkout at `./.nsh-cli/bin/nsh` (ignored by git):
  `cargo install --locked nsh-cli --version 0.7.1 --root .nsh-cli`, as `.github/workflows/ci.yml` does. Do not use
  another `nsh` on `PATH`, and do not run `nsh update`.
- Export `NSH_HOME="$PWD/.nsh-home"` before every `nsh` command so installs and build caches stay in the checkout
  (the `justfile` does).
- Rust crates take NSH from crates.io: `nsh-core = { version = "0.7.0" }` (and every other `nsh-*` crate; the `Cargo.lock`s have 0.7.1). Read NSH's
  source on GitHub at the tag `v0.7.1`, or in Cargo's registry (`cargo metadata --format-version 1` gives the path).
- GPU backend: `wgpu` 29 (the version NSH itself uses), standing in for Dawn. WebGPU in Rust, so the JS API maps
  onto wgpu calls almost one to one.

## Rules

- This is an **nshm**: a Rust crate with `#[native_module("nsh-webgpu/native")]` plus TSX exports in an `nsh.json`
  beside its `Cargo.toml`, at the repository's root. Platform views come from `#[native_components(<platform>)]`; no
  third-party frameworks are linked into NSH, system frameworks go through `objc2` / `windows` / `jni`.
- Stay inside this checkout. Do not write to any other repository. Do not run `killall`, `shutdown-all`,
  `erase-all`, `kill-server`, and never kill a process you did not start. Simulators/AVDs you create are named
  `nsh-webgpu-*`.
- **No git for agents**: agents do not commit, branch, stash or reset. The orchestrator (or the human maintainer)
  commits.
- Stay in your area (docs/PLAN.md "Ownership"). Need a change in another area? Say so in your report.
- Gates before calling work done: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`; for TSX, `nsh check`, `nsh build` (every warning fixed), `nsh test` in the app directory
  (`examples/headless`, `examples/canvas-apple`, `examples/gallery`). `just check`, `just test`, `just test-tsx <app>`
  run them.
- Every claim of "works" carries evidence: the command and its output saved under `docs/evidence/<area>/`, with home
  directories written as `~`. Look at every PNG you save. Keep evidence small (summaries, not multi-megabyte logs;
  PNGs downscaled to 800 px). A claim without evidence is reported as unverified.
- Fidelity: match the reference's exported names, signatures, prop names, defaults and error behaviour. Where NSH or
  wgpu cannot do something, keep the name, make it throw/reject with a clear `ERR_WEBGPU_UNSUPPORTED` message, and
  list it in `docs/parity.md` with the reason. Never silently drop an export.
- Licences: code or data copied or adapted from elsewhere is listed in [NOTICE.md](NOTICE.md) (and, for the gallery,
  `examples/gallery/LICENSES.md`) with its source and licence before it lands.
