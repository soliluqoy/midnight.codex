# Midnight for Codex CLI

This fork draws Midnight inside Codex's existing terminal UI. It is maintained on
`midnight/sidebar`, based on OpenAI's `rust-v0.162.1`
(`092d3acd6bec3e3a14bdc7e7a2810ab628ab759d`). It is an unofficial modified build.
The CLI reports `0.162.1-midnight.1`; its protocol and workspace crate versions stay
at the upstream version. No extra runtime dependencies or model calls are added.

## Use

Download the [Windows x64 prerelease](https://github.com/soliluqoy/midnight.codex/releases/tag/v0.162.1-midnight.1),
verify its ZIP against the release's `SHA256SUMS.txt`, and extract the entire
bundle. It contains all seven executables and their individual checksums. This
initial download uses the development build profile; the validation limits and
Windows runtime-lock issue are documented in the release notes.

From your working project:

```powershell
& "C:/path/to/midnight-codex/midnight-codex.exe" -c tui.fullscreen_transcript=true
```

Run `midnight-codex.exe` from your working project with your usual Codex options
and authentication. This is the CLI itself, not a terminal multiplexer or a second
UI process. Keep this executable separate from the official `codex` installation.

The sidebar is enabled by default in fullscreen mode at 110 columns or wider.
Use `-c tui.fullscreen_transcript=true` if your configuration uses inline mode.

- **Alt+S** toggles the 36-column sidebar.
- **Alt+R** refreshes its Git snapshot.
- The mouse wheel scrolls while the pointer is over the sidebar.

It shows the current title, model, project, input/output tokens, context usage,
Git branch, staged/unstaged/untracked/conflict counts, and changed paths. Usage is
read from the active chat widget. Changes describe the whole repository,
including work that predates the session. The pane does not infer session authorship.

The transcript and composer use the remaining width. Narrow terminals and inline
mode keep the ordinary layout. Dialogs and overlays keep input ownership. The
sidebar is a status view; for browsing, file previews, or diffs, use Codex's normal
tools or the [Midnight plugin](https://github.com/soliluqoy/midnight.server).

Git runs asynchronously through Codex's workspace command runner, so a remote
workspace is queried on its own host. Reads occur on first display, after a turn,
on project changes, and on explicit refresh. They coalesce and use a three-second
timeout and capped output. Status at or above 8,000 bytes is labeled unavailable
rather than silently truncated. No polling, watchers, or transcript scans are added.
Hide/drop cancels the pending reader; project changes detach its old cache.

Git refresh uses the host's normal command-execution and sandbox setup. In the
Windows live check, both this build and the stock CLI reported a setup failure
because an active `node_repl.exe` runtime file was locked. The pane and session
fields still worked, with Git labeled unavailable. Resolve the host setup error
before retrying **Alt+R**; this fork does not change the sandbox policy.

## Build on Windows

Prerequisites: Git, Rust/rustup, and Visual Studio C++ build tools with a Windows
SDK. The checked-in toolchain pins Rust 1.95.0. Run from this repository:

```powershell
Set-Location codex-rs
$env:CARGO_PROFILE_DEV_DEBUG = '0'
$env:CARGO_PROFILE_TEST_DEBUG = '0'
$env:RUST_MIN_STACK = '8388608'
cargo fmt -p codex-tui -p codex-cli -- --check
cargo test --locked -p codex-tui --lib midnight
cargo test --locked -p codex-tui --lib owned_transcript -- --test-threads=1
../scripts/build-midnight.ps1
../midnight-dist/midnight-codex.exe --version
```

The script builds the CLI and five Windows helpers from source. It downloads the
unchanged code-mode host from the exact upstream `rust-v0.162.1` release and checks
SHA-256 `9f865a748c984492ca9b69aac75680e4954f31311c43ec33ddbdae0ed77bd4bd`.
Rusty V8 150.4.0 does not publish its sandboxed Windows archive, so the normal Cargo
build of that host fails; the official host retains its V8 sandbox. This bundle
targets Windows x64. Keep all files in
`midnight-dist` together when copying the bundle. It also includes checksums and
license notices. `-OutputDirectory PATH` selects another destination.

The development profile avoids a long optimized build and is functional for
local use. Pass `-Release` to the script for an optimized build when needed.
Normal upstream build instructions and licensing still apply. `Cargo.lock` includes Cargo's workspace
version normalization from the release tag's 0.0.0 entries to 0.162.1.
The test stack setting matches upstream's test workflow for heavyweight App fixtures.

The **Midnight Windows build** GitHub Actions workflow checks formatting, runs
the sidebar and transcript tests, builds the Windows bundle, and uploads
`midnight-codex-windows-x64` with executables and SHA-256 checksums. It runs on this branch's pushes and supports
manual dispatch. Those are build artifacts, not automatically published releases.

## Maintain against upstream

Keep `upstream` pointed at `https://github.com/openai/codex.git` and `origin` at
this fork. Update on a separate branch so the working sidebar branch remains
usable during an upgrade:

```powershell
git fetch upstream --tags
git switch midnight/sidebar
git switch -c midnight/update-UPSTREAM_VERSION
git merge UPSTREAM_RELEASE_TAG
```

Resolve integrations against that release's source, especially the owned
transcript renderer, App initialization, key routing, workspace command runner,
and cached ChatWidget fields. Update the base and CLI suffix in this file and
`codex-rs/cli/src/main.rs`, and the official host's release URL/checksum in
`scripts/build-midnight.ps1`. Verify that checksum against the matching release
asset metadata. Do not replace the fork with an official auto-update.

Run the focused tests above, then `cargo test --locked -p codex-tui --lib
owned_transcript` and any suites affected by merge conflicts. Build the executable
and exercise a wide terminal, resize, Alt+S, Alt+R, wheel scrolling, an open popup,
session switching, non-Git cwd, and inline mode. No provider call is needed for
the focused sidebar tests. Review the diff and merge the update branch into
`midnight/sidebar` only after validation. This is an explicit upgrade workflow;
upstream changes are not automatically merged.

The implementation is concentrated in `codex-rs/tui/src/midnight.rs`,
`chatwidget/midnight.rs`, and small App integration points. Keep all I/O out of
rendering, retain output bounds and cancellation, and preserve prompt ownership.
