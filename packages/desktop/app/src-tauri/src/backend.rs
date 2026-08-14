//! Spawning the backend and reading its stdout handshake and stderr tail.

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;

use crate::parse::parse_url_line;
use crate::spec::BackendSpec;

/// Upper bound on the stderr bytes kept for the error dialog.
const STDERR_TAIL_CAP: usize = 64 * 1024;

/// The spawned backend and its pipes, owned by the reader thread.
pub struct BackendProcess {
    pub child: Child,
    stdout: BufReader<ChildStdout>,
    stderr: ChildStderr,
}

/// What the reader thread learns about the backend.
pub enum BackendOutcome {
    /// The readiness line appeared; the shell opens the window on this URL.
    Url(String),
    /// The backend process ended.
    Exited {
        status: ExitStatus,
        stderr_tail: Vec<u8>,
    },
}

/// Spawn the backend exactly as the shell's launch spec describes: hidden on
/// Windows, no stdin, stdout/stderr piped, and the working directory set.
pub fn spawn_backend(spec: &BackendSpec) -> std::io::Result<BackendProcess> {
    let mut command = Command::new(&spec.node);
    command
        .args(crate::spec::backend_args(spec))
        .current_dir(&spec.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: the backend must never open a console.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn()?;
    let stdout = BufReader::new(child.stdout.take().expect("stdout was piped"));
    let stderr = child.stderr.take().expect("stderr was piped");
    Ok(BackendProcess {
        child,
        stdout,
        stderr,
    })
}

/// Own the backend process on a background thread: read stdout until the
/// readiness line, deliver the URL, drain both pipes to EOF, then report the
/// exit. Draining past the URL matters: the backend blocks when its stdout
/// pipe fills, and the shell must keep consuming even after the window is up.
pub fn read_backend(process: BackendProcess, tx: mpsc::Sender<BackendOutcome>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let BackendProcess {
            mut child,
            mut stdout,
            mut stderr,
        } = process;
        loop {
            let mut line = String::new();
            match stdout.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if let Some(url) = parse_url_line(&line) {
                        let _ = tx.send(BackendOutcome::Url(url));
                    }
                }
                Err(err) => {
                    eprintln!("desktop shell: reading backend stdout failed: {err}");
                    break
                }
            }
        }
        let mut stderr_tail = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            match stderr.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    stderr_tail.extend_from_slice(&chunk[..n]);
                    let overflow = stderr_tail.len().saturating_sub(STDERR_TAIL_CAP);
                    if overflow > 0 {
                        stderr_tail.drain(..overflow);
                    }
                }
                Err(err) => {
                    eprintln!("desktop shell: reading backend stderr failed: {err}");
                    break
                }
            }
        }
        let status = child.wait().expect("backend was already waited");
        let _ = tx.send(BackendOutcome::Exited { status, stderr_tail });
    })
}

/// How the backend process ended, from the shell's point of view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendEnd {
    /// The window closed first; the shell killed the backend itself.
    UserQuit,
    /// The backend ended without a user-initiated kill.
    Unexpected { code: Option<i32> },
}

/// Classify an exit: a user quit wins regardless of the reported code,
/// because the shell killed the tree itself.
pub fn classify(user_quit: bool, code: Option<i32>) -> BackendEnd {
    if user_quit {
        BackendEnd::UserQuit
    } else {
        BackendEnd::Unexpected { code }
    }
}

/// Kill the backend and its descendants. On Windows the process tree is
/// terminated explicitly; elsewhere the direct child is killed. Session data
/// is append-only per event, so a hard kill loses no committed data.
pub fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .args(["-9", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::classify;

    #[test]
    fn user_quit_wins_over_any_code() {
        assert_eq!(classify(true, Some(0)), super::BackendEnd::UserQuit);
        assert_eq!(classify(true, Some(1)), super::BackendEnd::UserQuit);
        assert_eq!(classify(true, None), super::BackendEnd::UserQuit);
    }

    #[test]
    fn unexpected_exit_reports_the_code() {
        assert_eq!(classify(false, Some(0)), super::BackendEnd::Unexpected { code: Some(0) });
        assert_eq!(classify(false, Some(1)), super::BackendEnd::Unexpected { code: Some(1) });
        assert_eq!(classify(false, None), super::BackendEnd::Unexpected { code: None });
    }
}
