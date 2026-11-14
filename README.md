# Livery AI

Livery AI is a Windows desktop app for designing iRacing liveries. A Driver writes a brief, a
Claude-driven designer renders the paint and spec map, and the app checks the result in the iRacing
Paint Shop. It is a [Sim RaceCenter](https://simracecenter.com) tool. Powered by Claude.

**Status: early development.** The app shell runs: Garage, New livery, Studio, Cars and Settings, with
real project files, the bundled panel atlases for 11 cars, and the Claude connection check. Design
runs, rendering in the app, showroom capture and export are not built yet, and the screens say so.

## How it connects to Claude

Livery AI runs the [Claude Code](https://code.claude.com/docs) CLI already installed on the PC, signed
in with the Driver's own Claude subscription. Sign-in happens inside Claude Code. Livery AI never
sees, stores or logs a credential.

## Prerequisites

- Windows 10 or 11 (x64). WebView2 is part of Windows 11 and current Windows 10.
- [Node.js](https://nodejs.org) 22.12 or later.
- [Rust](https://rustup.rs) stable (1.85 or later) with the MSVC build tools.
  See the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).
- Claude Code on `PATH`, signed in (`claude auth login`), to use the designer.

The core also builds and tests on Linux for development. Install the
[Tauri Linux dependencies](https://v2.tauri.app/start/prerequisites/#linux) first.

## Develop

```sh
npm ci
npm run tauri dev     # run the app with hot reload
```

Checks, the same ones CI runs:

```sh
npm run lint
npm run typecheck
npm test
npm run build         # frontend to dist/ (the Rust build embeds it)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Build

```sh
npm run tauri build   # NSIS installer in target/release/bundle/nsis/
```

## Release

The version lives in two places that must match: `[workspace.package]` in `Cargo.toml` and
`package.json`. `tauri.conf.json` has no version; Tauri reads it from Cargo.

1. Set the new version in both files (`node scripts/check-version.mjs` confirms they agree) and merge.
2. Push a tag `v<version>`, for example `v0.0.1`.
3. The `Release` workflow checks the tag against both files, builds the NSIS installer on Windows and
   creates a **draft** GitHub Release. Review the draft and publish it.

Builds are not code-signed yet, so Windows SmartScreen shows a warning on first run.

## Layout

| Path | What it is |
|---|---|
| `src/` | UI: React, TypeScript, Vite |
| `src-tauri/` | Tauri 2 core in Rust: projects, settings, car library, Claude connection |
| `crates/liverai-engine/` | Native render engine |
| `resources/cars/` | Panel atlases (`panels.json`) for the calibrated cars |
| `docs/` | Design and implementation plan |

Projects are plain files under `Documents\Livery AI\<project>\`: `project.json`, `brief.md` and
`revisions\`.

## iRacing templates

iRacing paint templates are not included in this repository or in the installer, and Livery AI does
not redistribute them. Download the template for your car from iRacing and select it in the app. It
stays on your PC.

Livery AI is not affiliated with or endorsed by iRacing.com Motorsport Simulations.

## Licence

[GPL-3.0-or-later](LICENSE).
