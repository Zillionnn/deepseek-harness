# `@deepseek-ai/dsh-desktop`

The desktop shell: a Tauri v2 application that gives the DeepSeek Harness browser surface a double-click, no-console entry point on Windows. It spawns the harness backend (`dsh --profile web`) as a hidden child process, parses the `dsh web:` readiness line the backend prints on stdout, and hosts the UI in an embedded WebView2 window.

The shell is a carrier on the existing wire, not a Cordis plugin: the web surface it loads is the unmodified [`dsh-web-app`](../../bundle/web-app/README.md) composition, and it owns no harness behavior. All model-visible behavior, session data, and tool execution belong to the backend; the shell only hosts it.

## How it works

- `node <repo>/apps/cli/lib/bin.js --profile web --port 0` runs with the repository root as its working directory, `CREATE_NO_WINDOW` on Windows, stdin null, and stdout/stderr piped. `--port 0` lets the OS pick a free port; `printUrl` is `true` by default, so the backend prints `dsh web: http://127.0.0.1:<port>` once its Loader tree settles.
- The shell reads stdout until that line, opens the main window on the URL, then keeps draining both pipes. stderr is kept as a capped tail for the error dialog.
- Single instance: a second launch focuses the existing window. Closing the window kills the backend process tree (`taskkill /PID <pid> /T /F`); session data is append-only per event, so a hard kill loses no committed data. A backend exit without a user close shows a dialog with the stderr tail and the exit code.
- The shell's release build carries `windows_subsystem = "windows"`; the backend never shares a console.

## Building

Prerequisites: Node (the harness engine floor, `^22.19 || >=24`), `pnpm`, the Rust toolchain (rustup + the MSVC target), and WebView2 (preinstalled on Windows 10/11).

```sh
pnpm install
pnpm run build          # built CLI (apps/cli/lib/bin.js) and the frontend dist
pnpm run build:desktop  # portable exe: packages/desktop/app/src-tauri/target/release/dsh-desktop.exe
```

Development: `pnpm run dev:desktop` runs the shell from source; the backend is always the built CLI, so `pnpm run build` must have run once.

## Configuration

The repository root is resolved at compile time from the Cargo manifest location (four levels above `src-tauri`). When the repository moves after a build, override at runtime:

- `DSH_DESKTOP_BACKEND_CWD` — full override of the backend working directory (also the base of the CLI path and of the root `.env`).
- `DSH_DESKTOP_REPO_ROOT` — override of the compile-time repository root only.
- `DSH_DESKTOP_NODE` — the node executable (default: `node` from `PATH`).

`DEEPSEEK_API_KEY` and any other backend configuration come from the repository-root `.env` or the in-app settings UI, exactly as with `pnpm dsh --profile web`.

The backend reads the same per-user configuration as `pnpm dsh --profile web`: the optional `$DSH_HOME/cordis.patch.yml` (a top-level YAML array of loader patch rows, no wrapper object) overrides profile composition, and `$DSH_HOME/.agent-presets/<id>/agent.cordis.yml` defines per-session agent presets. The settings page's plugin list is a read-only view of the loaded composition; enabling or disabling a plugin means patching its composition row (`- id: <row-id>` with `disabled: true/false`), never a UI toggle.

Custom skills: `skill-filesystem` discovers only one directory level (`<root>/<name>/SKILL.md`), so a nested `~/.agents/skills/<category>/<name>/SKILL.md` layout needs each category directory listed in `Config.customSkillDirs` on the preset's `skill-filesystem` row. A writable preset must use an id that does not collide with a shipped preset id (`standard`, `minimal`, `cordis`, `code`): preset roots resolve first-root-wins, so a same-named copy in the user root is silently shadowed. The working recipe is a `desktop` preset copied from the shipped `standard` file with `customSkillDirs` added, plus a `$DSH_HOME/cordis.patch.yml` row `- id: agent-presets` with `config.default: desktop`; sessions created afterwards mount it, while sessions already joined keep the generation they started on.

## Known Limitations and Deferred Work

- **Windows-only v1** — the code keeps non-Windows `cfg` branches, but only Windows is built and exercised.
- **System Node required** — the shell does not bundle a Node runtime; bundling through the existing pkg `--sea` pipeline is a later distribution milestone.
- **Backend hard-killed on close** — closing the window terminates the backend tree instead of a graceful dispose; per-event persistence keeps committed data safe.
- **Frontend dist must exist** — the web-app bundle fails loud at activation when `pnpm run build` has not produced the frontend dist.
- **No tray, no auto-restart, no installer** — `tauri build` produces the portable exe only; a tray and an installer are deferred until there is a real need.
