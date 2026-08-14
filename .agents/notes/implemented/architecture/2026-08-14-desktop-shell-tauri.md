# Agent Note: Desktop shell surface (Tauri)

Status: implemented

English | [中文](2026-08-14-desktop-shell-tauri.zh.md)

## Problem

The harness's only interactive GUI is the browser surface: starting it means typing `pnpm dsh --profile web` in a terminal, which leaves a console window open for the life of the process, and the UI lives in a browser tab. There is no double-click entry point, and no native window. This fork wants a Codex-Desktop-like experience: double-click an executable, get a desktop window, no console.

## Decision

### The shell is a native Tauri v2 app, not a Cordis plugin

`packages/desktop/app` (`@deepseek-ai/dsh-desktop`) is a Tauri v2 application whose core is Rust: it spawns the harness backend as a hidden child process and renders the UI in an embedded WebView2 window (the Windows system runtime, not a bundled Chromium). The shell is a new carrier on the existing wire — the web surface it loads is the unmodified `dsh-web-app` bundle — so it is deliberately not a Cordis plugin and owns no harness behavior. There is no separate TypeScript plugin package; the package carries an empty TypeScript face (`src/index.ts`) with tsconfig/tsdown configs only so the workspace build globs (`packages/*/*`) and the host aggregate accept it, while the spawn spec and the handshake parsing live in Rust and are covered by cargo unit tests.

### Backend child process and the stdout handshake

The shell spawns `node <repo>/apps/cli/lib/bin.js --profile web --port 0` with `current_dir` = the repo root, `CREATE_NO_WINDOW` on Windows, stdin null, and stdout/stderr piped. `--port 0` lets the OS pick a free port; `printUrl` is `true` by default in the web-app bundle, so the backend prints `dsh web: http://127.0.0.1:<port>` (possibly with a ` (LAN: ...)` suffix) on stdout once its Loader tree settles. The shell reads stdout lines until the URL line, loads that URL in the main window, then keeps draining stdout to EOF. stderr is drained into a capped tail buffer that the error dialog includes when the backend dies.

The shell and the backend must not share a console: the backend is spawned with `CREATE_NO_WINDOW`, and the shell's release build carries `windows_subsystem = "windows"`.

### Path resolution

`DSH_DESKTOP_BACKEND_CWD` (runtime environment, full override) wins; otherwise the repo root baked at compile time (`CARGO_MANIFEST_DIR` walked four ancestors up) is used; otherwise startup fails loud with a dialog naming the variable. `DSH_DESKTOP_NODE` overrides the node executable (default `node` resolved from `PATH`). The CLI path is `<cwd>/apps/cli/lib/bin.js`, so the backend needs a prior `pnpm run build` (the web-app bundle fails loud at activation when the frontend dist is missing).

### Lifecycle: single instance, close = quit

`tauri-plugin-single-instance` focuses the existing window on a second launch. Closing the window is quitting: the shell marks the child as user-terminated and kills the process tree (`taskkill /PID <pid> /T /F` on Windows, `child.kill()` elsewhere). Session data is append-only per event, so a hard kill does not lose committed data. When the backend exits without a user close, the shell shows a dialog with the stderr tail and exits. There is no tray, no auto-restart, and no installer: `bundle.active` is `false`, and `tauri build` produces the portable exe only. v1 targets Windows only, though the code keeps `cfg` branches for non-Windows.

### Verification

Cargo unit tests only, per the fork's milestone scope: spawn-spec resolution order (env override, baked fallback, loud failure), URL-line parsing (prefix, whitespace, `http://` requirement, LAN suffix cut), and backend-exit classification (user-initiated vs unexpected). No integration smoke and no manual checklist in v1.

## Alternatives considered

**Electron.** The reference product (Codex Desktop) is Electron, and the first plan proposed it. Rejected for weight: a bundled Chromium is a ~200MB download and a memory-hungry runtime for a shell whose only job is hosting a loopback URL. Tauri's WebView2 reuse also keeps the OS webview up to date without app updates.

**Raw WebView2 host.** A minimal hand-written host (Rust/C#/C++) would be a few MB smaller than Tauri, but window management, single-instance, error dialogs, packaging, and future cross-platform support would all be hand-rolled. Tauri provides those for one dependency, which the repo's prefer-maintained-dependencies policy supports.

**Hidden-console launcher + default browser.** A tiny GUI stub that spawns the backend hidden and opens the default browser solves the console pain at zero cost, but delivers no desktop window and was rejected against the explicit Codex-Desktop-like goal.

**Backend in the shell process.** Running the harness inside the shell process couples lifecycles and risks the Node version floor (`^22.19 || >=24`): Electron and embedded runtimes ship older Node. The child process reuses the existing host↔client wire unchanged and keeps the shell and harness independent.

## Consequences

**Bought**: double-click native window with no console on Windows; the entire web surface, wire, and backend composition work unchanged (a loopback carrier is exactly what the browser-trust fence and native-operation gating already support); single instance and loud error dialogs; a small portable exe with no installer; distribution later can bundle Node into the existing pkg `--sea` pipeline without touching the shell.

**Paid**: the fork gains a Rust toolchain (rustup + MSVC) alongside the otherwise TypeScript/C repo; WebView2 is a system runtime dependency (preinstalled on Windows 10/11); spawn-spec and handshake logic is duplicated across the shell and any future carrier rather than shared; v1 is Windows-only and requires system Node plus a prior `pnpm run build`; the backend is hard-killed on close (safe because per-event persistence, but not a graceful dispose); `DEEPSEEK_API_KEY` still comes from the repo-root `.env`, with no in-app key entry.
