# Livery AI: design and implementation plan

Status: **approved by the owner on 2026-11-14; implementation in progress.**

Owner decisions (2026-11-14): native Rust render engine (the proof-of-concept Python is not shipped);
Claude subscription mode only; Sim RaceCenter's own tool, public and open source; name "Livery AI"
(`liverai`); installer-only releases; no third-party decals in the bundled kit; mockups approved.

Livery AI is a Windows desktop app (Tauri 2) that turns the livery process proven in
`margic/sandbox` into a product: a Driver gives a brief, a Claude-driven designer renders an iRacing
paint and spec map, and the app verifies the result in the iRacing Paint Shop. The Driver brings
their own Claude account.

UI mockups: https://claude.ai/artifact/TKgqMr4Ho6SNoT2G6BKDUy (private to the owner).

## 1. What the sandbox process is

Researched from `margic/sandbox` at `6701197` (`AGENTS.md`, `.agents/skills/livery-design/SKILL.md`,
`docs/*.md`, `scripts/*.py`, `cars/*/atlas/panels.json`, `liveries/*`).

| Step | What happens | Where it lives today |
|---|---|---|
| 1. Template intake | Unpack the iRacing template zip and read the PSD layer tree with `psd-tools`: paintable `Mask`, `Wire`, `Number Blocks`, `Car Mandatory`, `Car Decal`, `car_pattern_NNN`, `Custom Spec Map`. Layer names differ per car. | Inline snippets in `AGENTS.md` |
| 2. B0 calibration | Segment UV islands, paint each a unique colour and id, load in the Paint Shop, capture, then label panels (`label`, `up_angle_deg`, `forward`, `mirror_of`) until at least 80% of paintable area is confirmed. | `scripts/render_calibration_livery.py`, `cars/<car>/atlas/panels.json` (11 cars, 80.5% to 90.5%) |
| 3. Design | Pick a seam-safe built-in pattern, recolour its three slots, place decals and text per panel. Run the 4-mask intrusion scan on every decal box. | One hand-written script per car and per livery (18 scripts, 6,415 lines) |
| 4. Render and lint (Loop A) | Write `car_<custID>.tga` and `car_spec_<custID>.tga` (2048², 24-bit, RLE; spec R metallic, G roughness, B clear coat). Lint: 0 px on number blocks and mandatory, 0 px outside the mask, hardware spec preserved. | Each renderer script; `lint.json` |
| 5. Showroom (Loop B) | Copy into `Documents\iRacing\paint\<car>\`, send `Ctrl+R` to the Paint Shop, capture 9 views with `PrintWindow`, review, write `REVIEW.md`. Copy the compiled `.mip` back for Trading Paints. | Described in the skill. **The Win32 capture code is not in the repo**; only the docs mention it |
| 6. Feedback | One line per review item: `ok`, `wrong-panel`, `rotated`, `mirrored`, `too-big`, `too-small`, `seam-mismatch`, `colour-off`, `finish-off`. Atlas errors fix `panels.json`; design errors fix the livery. | `docs/test_feedback_plan.md` |

What does not carry over as-is:

- Paths, the customer id and the Python venv are hard-coded for one PC.
- Each livery is a bespoke script with pixel coordinates. `scripts/render_livery.py` and the three
  `livery.json` files are an early declarative renderer that the later scripts outgrew.
- Showroom automation uses fixed viewport offsets and has no committed implementation.
- Shared decal assets include third-party badges ("Created by Devin.ai", IRN-BRU) that should not ship
  as defaults.

## 2. Product scope

In scope for v1:

- Windows 10/11 x64, single user, local files. No account, no server, no telemetry.
- Garage of livery projects with revisions.
- Design run: brief → render → lint → install (with approval) → reload → capture → review → iterate.
- Car library with the 11 calibrated atlases from the sandbox, plus calibration for new templates.
- Brand kits (palette, finishes, logos, fonts). The Sim RaceCenter kit ships as the default.
- Trading Paints export package with the upload-slot checks.
- Claude connection settings (section 4).

Livery AI is Sim RaceCenter's own tool, published as open source for interest. It is not a product for
other teams, so there is one brand kit to begin with.

Out of scope for v1: macOS/Linux builds, a Trading Paints upload API, a 3D viewer of our own (iRacing
stays the source of 3D truth), cross-seam custom geometry (`seams.json`), multi-user sync.

"Hosted" here means a packaged, installable app that hosts the process. If a cloud-hosted service was
meant, say so; that is a different design.

## 3. Architecture

```
┌───────────────────────── Livery AI (Tauri 2) ─────────────────────────┐
│  UI: React + TypeScript + Vite (WebView2)                             │
│      Garage · New livery · Studio · Showroom · Cars · Settings        │
│                 ▲ Tauri commands + events                             │
│  Core: Rust                                                           │
│   ├─ project store (files under Documents\Livery AI)                  │
│   ├─ settings + secrets (Windows Credential Manager)                  │
│   ├─ iRacing bridge (Win32: find window, PrintWindow, SendInput)      │
│   ├─ approvals (install, input to iRacing)                            │
│   ├─ tool server: MCP over loopback HTTP, per-run bearer token        │
│   └─ Claude runner: spawns the Claude runtime, parses stream-json     │
│          │ spawns                          │ spawns                   │
│          │ links                           │ spawns                   │
│  ┌───────▼─────────┐              ┌────────▼──────────────────────┐   │
│  │ liverai-engine  │              │ Claude Code CLI (user's own)  │   │
│  │ Rust crate      │◄── tools ────│ `claude -p`, stream-json,     │   │
│  │ PSD→TGA, lint   │   via core   │ `--mcp-config` → tool server  │   │
│  └─────────────────┘              └───────────────────────────────┘   │
└───────────────────────────────────────────────────────────────────────┘
```

### 3.1 Engine (native Rust crate)

`crates/liverai-engine` is a Rust library, linked into the app, with a small CLI of the same name for
testing against real templates. It has no network access and no platform-specific code, so it is tested
on Linux and Windows. It is a port of the proof-of-concept scripts, checked against them: the test
fixture's reference values are produced by the proof-of-concept's own Python code.

| Module | Does |
|---|---|
| `template` | Reads the PSD (`psd` crate): layer tree, paintable mask, keep-outs, patterns, default spec map and hardware mask. Layer names are normalised |
| `islands` | UV island segmentation, island geometry, calibration colours, and matching islands to a car's `panels.json` by geometry |
| `calibrate` | B0 calibration paint with island ids and up arrows, plus `islands.json` |
| `scan` | 4-mask intrusion scan for decal boxes |
| `livery` | The `livery.json` schema and validation |
| `render` | Pattern recolour, panel finishes, decals, spec map, preview and lint |
| `tga` | 24-bit RLE TGA writer (and a reader for the lint) |

CLI: `liverai-engine inspect | calibrate | scan | render`, each printing JSON.

**Declarative first.** `livery.json` covers the base pattern, slot colours, per-panel colour and
finish, decals and finishes. The proof-of-concept's bespoke per-livery effects (the Saltire carbon
weave, borders derived from pattern channels) are not expressible yet. They will be added as named,
parameterised engine features (procedural fills, pattern-derived borders), not as scripts: with a
native engine there is no script interpreter, which also removes the risk of running generated code.

The PSD parser has only been run against a synthetic template so far. The first real test is the
owner running `liverai-engine inspect` on an iRacing template.

### 3.2 Core (Rust)

- **Project store.** `Documents\Livery AI\<project>\`: `project.json`, `brief.md`, `livery.json`,
  `assets\`, `revisions\rev-N\` (TGAs, previews, `lint.json`, `showroom\*.png`,
  `review.json`), `export\`. Plain files so a Driver can back them up or version them.
- **iRacing bridge** (`windows` crate). Finds the Paint Shop window, detects the 3D viewport bounds
  from the captured frame, sends `Ctrl+R` and orbit drags with `SendInput`, captures with
  `PrintWindow(PW_RENDERFULLCONTENT)`. It never resizes, moves or restores the window. Customer id
  detection reads the iRacing UI cache with shared-read access, with a manual override.
- **Install.** Copies the TGA pair into `<paint folder>\<car>\` after backing up existing files, and
  copies the compiled `.mip` back after reload.
- **Tool server.** One MCP endpoint on `127.0.0.1:<random port>` with a per-run bearer token. Tools:
  the engine commands above, plus `install_paint`, `reload_paint_shop`, `capture_showroom`,
  `read_image`, `write_review`, `ask_driver`. `install_paint` and anything that sends input to iRacing
  block until the Driver approves in the UI (configurable).
- **Claude runner.** See section 4.

### 3.3 UI (React + TypeScript)

Seven screens, mocked up on the canvas linked above: Garage, New livery, Studio, Showroom review,
Trading Paints export, Car library, Settings. The product follows the Sim RaceCenter brand guide:
Race Control `#05070A` ground, Cockpit Grey `#111827` surfaces, Apex Orange `#FF5F1F` for the one
primary action per screen, Telemetry Blue `#0EA5E9` for data, Flag Yellow for caution only, Roboto and
Roboto Mono, crisp technical copy.

## 4. Claude connection (bring your own)

One mode: **Claude subscription**. The app never implements a claude.ai login of its own and never
handles a Claude credential.

The app runs the Driver's own installed Claude Code CLI as a subprocess
(`claude -p --output-format stream-json`). Claude Code uses the sign-in the Driver already made with
their Pro/Max/Team/Enterprise plan. Settings shows whether the CLI is found and signed in, and a button
opens Claude Code's own sign-in in a terminal.

The runner passes `--mcp-config` (our tool server only, `--strict-mcp-config`), an `--allowedTools`
list limited to our tools, the livery-design instructions as an appended system prompt, and
`--resume <session id>` to continue a project's conversation. Default model `claude-opus-5-5`,
selectable; effort selectable. It never passes `--bare`, which ignores subscription sign-ins.

**Policy note.** Anthropic's Agent SDK documentation says: "Unless previously approved,
Anthropic does not allow third party developers to offer claude.ai login or rate limits for their
products, including agents built on the Claude Agent SDK." Driving the Driver's own Claude Code
install, with no login UI of ours and no token handling, keeps the sign-in inside Anthropic's product,
and is how the sandbox process already runs. Livery AI is Sim RaceCenter's own tool, so this is the
same as running Claude Code by hand. Because the source is public, the README says the same thing to
anyone who builds it: bring your own Claude Code install, and the app does not sign you in. If the app
were ever offered to others as a product, this would need confirming with Anthropic first. The branding
rules also apply: say "Powered by Claude", and do not present the app as Claude Code.

Not chosen: an API-key mode (owner decision), asking the Driver to paste a `claude setup-token` value
into the app (handling a subscription token is the thing the policy restricts), and calling the Messages
API directly with a hand-written agent loop.

Things to verify during M3: exact CLI flags on the minimum supported Claude Code version, how to keep
the Driver's global `~/.claude` hooks and skills out of subscription-mode runs (`--bare` does not read
subscription sign-ins), and image input to the designer (tool results returning PNGs vs file reads).

## 5. The design run

1. **Pre-flight.** Claude connected, template valid, atlas calibrated, paint folder writable. The Paint
   Shop is only required from step 5.
2. **Plan.** The designer inspects the template, reads the atlas and brand kit, picks a pattern from
   the contact sheet, and proposes decal boxes.
3. **Scan.** Every box goes through the 4-mask scan. Text lockups need zero unpaintable pixels.
4. **Render and lint.** A new revision. If lint fails, the designer fixes and re-renders, up to the
   auto-fix limit.
5. **Install and reload.** Held for approval. Backs up, copies, sends `Ctrl+R`, waits for the `.mip`.
6. **Capture.** 9 views. A failed view is reported as missing, never faked.
7. **Review.** The designer checks each capture against its review items and writes a verdict per item.
   The Driver confirms or overrides with the feedback chips and an optional note.
8. **Iterate or accept.** Feedback is triaged: atlas errors update `panels.json` for the car; design
   errors update `livery.json`. Accepting marks the revision final and enables export.

Runs are resumable: every step writes its result to the revision folder, and the run log in the Studio
is rebuilt from those files plus the CLI session.

## 6. Repository layout (`simracecenter/liverai`)

```
.github/workflows/ci.yml, release.yml
docs/                     this plan, ADRs, tool contract, livery.json schema
Cargo.toml                workspace: crates/liverai-engine, src-tauri
crates/liverai-engine/    native render engine (library + CLI), tests and synthetic fixture
src/                      React + TypeScript UI
src-tauri/                Rust core, tauri.conf.json, capabilities/
resources/cars/           panels.json for the 11 calibrated cars (no template-derived images)
resources/brand/          Sim RaceCenter kit (palette, logos, Roboto)
resources/prompts/        designer instructions derived from the livery-design skill
```

The repo holds only a GPL-3.0 `LICENSE` today, so the app is GPL-3.0-or-later like the simulator MCP
servers. iRacing templates (`*.psd`) and rendered paints are never committed or bundled.

## 7. CI/CD

**`ci.yml`** on every pull request and on `main`:

- UI: `npm ci`, lint, typecheck, unit tests (Vitest).
- Core: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` on `windows-latest`.
- Engine: `cargo test -p liverai-engine` on `ubuntu-latest` as well, using a small synthetic PSD
  fixture (no iRacing template in the repo), with reference values from the proof-of-concept code.

**`release.yml`** on a pushed tag `v*.*.*`:

1. Fail unless the tag equals the version in the root `Cargo.toml` and `package.json`.
2. `tauri-apps/tauri-action@v1` on `windows-latest` builds the NSIS installer and creates a **draft**
   GitHub Release for the tag with the `-setup.exe` attached.
3. The owner reviews the draft and publishes it.

`permissions: contents: write` on the release job only.

Later, each needing a secret the owner adds to the repo: code signing (unsigned builds show a
SmartScreen warning) and the Tauri updater (`TAURI_SIGNING_PRIVATE_KEY`).

The workflow follows the current Tauri GitHub pipeline guide (checked 2026-11-14).

## 8. Milestones

Each milestone is one or more pull requests and ends with something the owner can run.

| # | Milestone | Done when |
|---|---|---|
| M0 | Scaffold and pipeline | Tauri 2 + React shell with the brand theme; `ci.yml` green; tag `v0.0.1` produces a draft release with a Windows installer that launches |
| M1 | Engine | `liverai-engine` (Rust) ports template reading, calibration, scan, `livery.json` render, spec map, lint; tests green; then one real template inspected and one sandbox livery re-rendered and compared on the owner's PC |
| M2 | Garage, Cars, manual Studio | Create a project, import a template, see the UV preview and lint for a hand-edited `livery.json`; 11 atlases bundled |
| M3 | Claude connection and design run to lint | Settings detects and tests both modes; a brief produces a linted revision with the run log streaming |
| M4 | iRacing bridge and showroom loop | Install with backup, reload, 9-view capture, designer review, Driver feedback, next revision |
| M5 | Export, procedural fills, calibration for new cars | Trading Paints package with checks; carbon weave and pattern-derived borders in the engine; a new template calibrated end to end |
| M6 | Release hardening | Signing, updater, first-run setup, error states, docs; `v0.1.0` |

M1 and the UI half of M2 can run in parallel. M4 needs testing on a PC with iRacing; a cloud session
cannot run it, so those pull requests ship with a manual test script for the owner.

## 9. Risks

| Risk | Mitigation |
|---|---|
| Subscription use from a third-party app | Section 4. The app only drives the Driver's own Claude Code install and handles no credential. Confirm with Anthropic before offering it to others as a product |
| Paint Shop UI changes break window detection or capture | Detect the viewport from pixels, not fixed offsets; a calibration check in Settings; a manual "I reloaded, capture now" path |
| The `psd` crate fails on a real iRacing template | Owner runs `liverai-engine inspect` on real templates early (M1 exit); fall back to another PSD crate or a focused parser for the layer records we need |
| Declarative schema cannot express a wanted effect | Add it as a named engine feature with tests; no generated scripts |
| Template layer names vary per car | `template inspect` normalises names with a per-car override file; unknown layouts stop with a clear error |
| Unsigned installer shows a SmartScreen warning | Code signing in M6 |
| Token cost of image-heavy review rounds | Downscale captures to 1024 px; auto-fix round limit; show usage per run |

## 10. Decisions (owner, 2026-11-14)

1. **Claude connection.** Subscription mode only, through the Driver's own Claude Code install.
2. **Engine.** Native Rust. The proof-of-concept Python is not shipped.
3. **Audience.** Sim RaceCenter's own tool; the repo is public and open source for interest.
4. **Name.** "Livery AI" in the UI; `liverai` for the repo, binary and folders.
5. **Release artefacts.** NSIS installer only.
6. **Default assets.** No "Created by Devin.ai" or IRN-BRU decals in the bundled kit.
7. **Mockups.** Approved.
