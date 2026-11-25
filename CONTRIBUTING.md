# Contributing

[AGENTS.md](AGENTS.md) holds the rules every contributor follows, human or agent. This page is how the work is split and checked.

## Toolchain

- The CLI of the NSH release 0.7.1, kept in the checkout at `.nsh-cli/bin/nsh` (ignored by git): `cargo install --locked nsh-cli --version 0.7.1 --root .nsh-cli`. Do not use another `nsh` on `PATH`, and do not run `nsh update`.
- `NSH_HOME="$PWD/.nsh-home"` for every `nsh` command, so installs and build caches stay in the checkout. The `justfile` sets it.
- Every `nsh-*` crate comes from crates.io, `0.7.1` or a later 0.7, in the crate and in each example's `native/` crate. Keep the `Cargo.lock` files.
- Rust stable, with the targets you check: `wasm32-unknown-unknown`, `aarch64-linux-android` (and an NDK), `aarch64-apple-ios-sim`.

## Areas and worktrees

The work is split into areas whose files do not overlap. [docs/PLAN.md](docs/PLAN.md) "Ownership" and [docs/architecture.md](docs/architecture.md) "Phase 2: ownership" list them. During the port each area worked in a git worktree of its own on the branch `area/<area>`, merged by the orchestrator; a contributor today works on a branch and opens a pull request.

- Find your area before you touch a file, and stay in it. A change you need in another area's file goes in your report as a request, with the reason.
- New files go in your area's directories. Rust: a native function lives in the file of the interface it belongs to. TSX: a spec class's factory lives in that class's file, never in `GPUDevice.ts`.
- Each area keeps its rows in `docs/parity/<area>.md` (export, status, reason); the orchestrator merges them into `docs/parity.md`.
- Agents do not run git: no commit, branch, stash or reset. The orchestrator commits.
- Simulators and AVDs you create are named `nsh-webgpu-*`. Never kill a process you did not start.

## Gates

Before calling work done, from the repository root:

```sh
just check      # cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings
just test       # cargo test --workspace
just test-tsx   # in examples/headless: nsh check, nsh build (a warning fails), nsh test
```

`just test-tsx examples/canvas-apple` runs the TSX gates in another app, and `just cross <target>` runs Clippy for a cross target as CI does. CI (`.github/workflows/ci.yml`) runs all of them; see the README's "Checks".

Tests go through the UI or the module's public functions, on a manual clock, with no real network. Every new test is seen to fail once: break the code on purpose, run the test, save the output, restore the code.

## Evidence

- Every claim that something works carries evidence: the command you ran and its output, saved under `docs/evidence/<area>/`.
- Look at every PNG you save. A screenshot nobody looked at proves nothing.
- A claim without evidence is reported as unverified, in your report and in the docs you write.
- Say which machine, GPU and NSH build the evidence comes from. Evidence from one platform says nothing about another.
- Keep it small: summaries and gate outputs, not multi-megabyte logs; PNGs downscaled (`sips -Z 800`); home directories written as `~`.

## Licences

Code, shaders or data copied or adapted from another project are listed in [NOTICE.md](NOTICE.md) with the source and its licence text (and, in the gallery, in `examples/gallery/LICENSES.md`) in the same change.

## Fidelity

Match [react-native-webgpu at `06599f9`](https://github.com/wcandillon/react-native-webgpu/tree/06599f9): exported names, signatures, prop names, defaults and error behaviour. Where NSH or wgpu cannot do something, keep the name, throw or reject with `code: "ERR_WEBGPU_UNSUPPORTED"` and a message that says why, and add a row to your area's parity file. Never drop an export silently.

## Docs

The README, this file, `CHANGELOG.md` and `docs/guide/` describe the code as it is. Every code sample in them is taken from, or checked against, code that ran: the examples, the tests, or the samples in `examples/headless/samples/` (type-checked by `nsh check` there; the README's example is `samples/readme.app.tsx`, mounted by `samples.test.tsx` and kept identical to the README by `cargo test --test readme`). Mark a sample nobody ran as untested.
