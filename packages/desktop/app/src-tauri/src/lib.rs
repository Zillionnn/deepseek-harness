//! The DeepSeek Harness desktop shell.
//!
//! A Tauri v2 application that spawns the harness backend (`dsh --profile web`)
//! as a hidden child process and hosts the harness UI in an embedded WebView2
//! window. The shell is a carrier on the existing wire: it parses the `dsh web:`
//! URL line the backend prints on stdout, loads that URL, and tears the backend
//! down when the window closes.

mod backend;
mod parse;
mod spec;

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;

use backend::{kill_tree, spawn_backend, BackendOutcome};
use spec::{resolve_spec, SpecError};

const WINDOW_LABEL: &str = "main";

struct DesktopState {
    pid: u32,
    user_quit: AtomicBool,
}

fn fatal(app: &AppHandle, message: &str) -> ! {
    let _ = app
        .dialog()
        .message(message)
        .title("DeepSeek Harness Desktop")
        .blocking_show();
    app.exit(1);
    unreachable!("app.exit is final")
}

fn create_main_window(app: &AppHandle, url: &str) {
    if app.get_webview_window(WINDOW_LABEL).is_some() {
        return
    }
    let Ok(url) = url.parse::<tauri::Url>() else {
        fatal(app, &format!("The backend printed an unparsable URL line: {url}"))
    };
    if let Err(err) = WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::External(url))
        .title("DeepSeek Harness")
        .inner_size(1280.0, 800.0)
        .build()
    {
        fatal(app, &format!("Failed to open the harness window: {err}"))
    }
}

fn handle_outcome(app: &AppHandle, outcome: BackendOutcome) {
    match outcome {
        BackendOutcome::Url(url) => create_main_window(app, &url),
        BackendOutcome::Exited { status, stderr_tail } => {
            let state = app.state::<DesktopState>();
            match backend::classify(state.user_quit.load(Ordering::SeqCst), status.code()) {
                backend::BackendEnd::UserQuit => app.exit(0),
                backend::BackendEnd::Unexpected { code } => {
                    let code_text = code
                        .map(|code| format!(" (exit code {code})"))
                        .unwrap_or_default();
                    let tail = String::from_utf8_lossy(&stderr_tail);
                    let detail = if tail.trim().is_empty() {
                        String::new()
                    } else {
                        format!("\n\nThe backend's last output was:\n{tail}")
                    };
                    fatal(
                        app,
                        &format!(
                            "The harness backend exited unexpectedly{code_text}.{detail}"
                        ),
                    )
                }
            }
        }
    }
}

/// Application entry point, called by `main.rs`.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let spec = match resolve_spec(&|key| std::env::var(key).ok(), spec::baked_repo_root()) {
                Ok(spec) => spec,
                Err(SpecError::MissingCwd) => fatal(
                    &handle,
                    "The shell could not locate the harness repository.\n\
                     Set DSH_DESKTOP_BACKEND_CWD to the repository root (where \
                     apps/cli/lib/bin.js and the root .env live) and start again.",
                ),
            };
            let process = match spawn_backend(&spec) {
                Ok(process) => process,
                Err(err) => fatal(&handle, &format!("Failed to start the harness backend: {err}")),
            };
            let pid = process.child.id();
            app.manage(DesktopState {
                pid,
                user_quit: AtomicBool::new(false),
            });

            let (tx, rx) = mpsc::channel();
            let _reader = backend::read_backend(process, tx);
            thread::spawn(move || {
                while let Ok(outcome) = rx.recv() {
                    let app = handle.clone();
                    let app_inner = app.clone();
                    let _ = app.run_on_main_thread(move || handle_outcome(&app_inner, outcome));
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != WINDOW_LABEL {
                return
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let state = window.state::<DesktopState>();
                state.user_quit.store(true, Ordering::SeqCst);
                kill_tree(state.pid);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running the DeepSeek Harness desktop shell");
}
