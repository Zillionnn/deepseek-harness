//! Resolving how the backend is launched: the node executable, the CLI script,
//! and the working directory.

use std::fmt;
use std::path::PathBuf;

/// Full override for the backend working directory (normally the repository
/// root, where `apps/cli/lib/bin.js` and the root `.env` live).
pub const ENV_BACKEND_CWD: &str = "DSH_DESKTOP_BACKEND_CWD";

/// Override for the compile-time repository-root fallback, for when the
/// repository moved after the shell was built.
pub const ENV_REPO_ROOT: &str = "DSH_DESKTOP_REPO_ROOT";

/// Override for the node executable (default: `node` resolved from PATH).
pub const ENV_NODE: &str = "DSH_DESKTOP_NODE";

/// Everything the shell needs to spawn the backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendSpec {
    /// The node executable to run.
    pub node: String,
    /// The built CLI entry: `<repo>/apps/cli/lib/bin.js`.
    pub cli: PathBuf,
    /// The backend working directory; also the base of `cli` and of `.env`.
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecError {
    /// Neither an environment variable nor the injected fallback names a
    /// repository root.
    MissingCwd,
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecError::MissingCwd => write!(
                f,
                "no backend working directory: set {ENV_BACKEND_CWD} to the repository root"
            ),
        }
    }
}

/// The repository root as known when the shell was compiled: the Tauri project
/// lives at `<repo>/packages/desktop/app/src-tauri`, four levels below the root.
pub fn baked_repo_root() -> Option<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .map(PathBuf::from)
}

/// Resolve the backend launch spec. `env` reads an environment variable by
/// name (tests inject a map); `baked` is the compile-time repository-root
/// fallback and stays `None` only in tests that pin the failure path.
pub fn resolve_spec(
    env: &impl Fn(&str) -> Option<String>,
    baked: Option<PathBuf>,
) -> Result<BackendSpec, SpecError> {
    let cwd = env(ENV_BACKEND_CWD)
        .map(PathBuf::from)
        .or_else(|| env(ENV_REPO_ROOT).map(PathBuf::from))
        .or(baked)
        .ok_or(SpecError::MissingCwd)?;
    Ok(BackendSpec {
        node: env(ENV_NODE).unwrap_or_else(|| "node".to_string()),
        cli: cwd.join("apps").join("cli").join("lib").join("bin.js"),
        cwd,
    })
}

/// The backend command line, excluding the node executable itself.
pub fn backend_args(spec: &BackendSpec) -> Vec<String> {
    vec![
        spec.cli.to_string_lossy().into_owned(),
        "--profile".to_string(),
        "web".to_string(),
        "--port".to_string(),
        "0".to_string(),
        "--no-open".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::{backend_args, resolve_spec, BackendSpec, ENV_BACKEND_CWD, ENV_NODE, ENV_REPO_ROOT};
    use std::path::PathBuf;

    fn map(entries: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let owned: Vec<(String, String)> = entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key| {
            owned
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
        }
    }

    fn baked() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(4)
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn env_backend_cwd_wins_over_everything() {
        let env = map(&[
            (ENV_BACKEND_CWD, "C:\\repo"),
            (ENV_REPO_ROOT, "C:\\wrong"),
            (ENV_NODE, "C:\\node.exe"),
        ]);
        let spec = resolve_spec(&env, Some(PathBuf::from("C:\\baked"))).unwrap();
        assert_eq!(
            spec,
            BackendSpec {
                node: "C:\\node.exe".to_string(),
                cli: PathBuf::from(r"C:\repo\apps\cli\lib\bin.js"),
                cwd: PathBuf::from("C:\\repo"),
            }
        );
    }

    #[test]
    fn repo_root_env_overrides_the_baked_fallback() {
        let env = map(&[(ENV_REPO_ROOT, "C:\\other")]);
        let spec = resolve_spec(&env, Some(PathBuf::from("C:\\baked"))).unwrap();
        assert_eq!(spec.cwd, PathBuf::from("C:\\other"));
        assert_eq!(spec.node, "node".to_string());
    }

    #[test]
    fn baked_fallback_is_the_manifest_four_levels_up() {
        let spec = resolve_spec(&map(&[]), Some(baked())).unwrap();
        assert_eq!(spec.cwd, baked());
        assert_eq!(spec.node, "node".to_string());
    }

    #[test]
    fn missing_every_source_fails_loud() {
        assert_eq!(
            resolve_spec(&|_| None, None),
            Err(super::SpecError::MissingCwd)
        );
    }

    #[test]
    fn node_env_overrides_the_default() {
        let env = map(&[(ENV_BACKEND_CWD, "C:\\repo"), (ENV_NODE, "node18.exe")]);
        assert_eq!(resolve_spec(&env, None).unwrap().node, "node18.exe");
    }

    #[test]
    fn args_are_profile_web_on_port_zero() {
        let spec = BackendSpec {
            node: "node".to_string(),
            cli: PathBuf::from(r"C:\repo\apps\cli\lib\bin.js"),
            cwd: PathBuf::from("C:\\repo"),
        };
        assert_eq!(
            backend_args(&spec),
            vec![
                r"C:\repo\apps\cli\lib\bin.js",
                "--profile",
                "web",
                "--port",
                "0",
                "--no-open"
            ]
        );
    }
}
