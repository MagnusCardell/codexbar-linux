use std::path::Path;
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandKind {
    Version,
    Usage,
    Cost,
    Status,
}

impl CommandKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::Usage => "usage",
            Self::Cost => "cost",
            Self::Status => "status",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CommandSpec {
    pub kind: CommandKind,
    pub args: Vec<String>,
    pub timeout: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct CommandOutput {
    pub(crate) kind: CommandKind,
    pub(crate) exit_code: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) stdout_truncated: bool,
    pub(crate) stderr_truncated: bool,
    pub(crate) duration_ms: u64,
}

impl CommandOutput {
    pub(crate) fn success(&self) -> bool {
        !self.timed_out && self.exit_code == Some(0)
    }

    pub(crate) fn summary(&self) -> CommandSummary {
        CommandSummary {
            kind: self.kind,
            exit_code: self.exit_code,
            timed_out: self.timed_out,
            stdout_bytes: self.stdout.len(),
            stderr_bytes: self.stderr.len(),
            stdout_truncated: self.stdout_truncated,
            stderr_truncated: self.stderr_truncated,
            duration_ms: self.duration_ms,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSummary {
    pub kind: CommandKind,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandRunError {
    Spawn,
    Io,
    Join,
}

#[derive(Clone, Debug, Default)]
pub struct CommandRunner;

impl CommandRunner {
    pub(crate) async fn run(
        &self,
        binary: &Path,
        spec: &CommandSpec,
    ) -> Result<CommandOutput, CommandRunError> {
        let started = Instant::now();
        let mut command = Command::new(binary);
        command
            .args(&spec.args)
            .env_clear()
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        copy_allowed_environment(&mut command);

        let mut child = command.spawn().map_err(|_| CommandRunError::Spawn)?;

        let stdout = child.stdout.take().ok_or(CommandRunError::Io)?;
        let stderr = child.stderr.take().ok_or(CommandRunError::Io)?;
        let stdout_limit = spec.max_stdout_bytes;
        let stderr_limit = spec.max_stderr_bytes;
        let stdout_handle = tokio::spawn(read_limited(stdout, stdout_limit));
        let stderr_handle = tokio::spawn(read_limited(stderr, stderr_limit));

        let wait = tokio::time::timeout(spec.timeout, child.wait()).await;
        let (status, timed_out) = match wait {
            Ok(Ok(status)) => (status, false),
            Ok(Err(_)) => return Err(CommandRunError::Io),
            Err(_) => {
                let _ = child.start_kill();
                let status = child.wait().await.map_err(|_| CommandRunError::Io)?;
                (status, true)
            }
        };

        let stdout = stdout_handle
            .await
            .map_err(|_| CommandRunError::Join)?
            .map_err(|_| CommandRunError::Io)?;
        let stderr = stderr_handle
            .await
            .map_err(|_| CommandRunError::Join)?
            .map_err(|_| CommandRunError::Io)?;

        Ok(CommandOutput {
            kind: spec.kind,
            exit_code: status.code(),
            timed_out,
            stdout: stdout.bytes,
            stderr: stderr.bytes,
            stdout_truncated: stdout.truncated,
            stderr_truncated: stderr.truncated,
            duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        })
    }
}

#[derive(Debug)]
struct LimitedRead {
    bytes: Vec<u8>,
    truncated: bool,
}

async fn read_limited(
    mut reader: impl AsyncRead + Unpin,
    limit: usize,
) -> std::io::Result<LimitedRead> {
    let mut stored = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;

    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(stored.len());
        if remaining > 0 {
            let keep = read.min(remaining);
            stored.extend_from_slice(&buffer[..keep]);
            if keep < read {
                truncated = true;
            }
        } else {
            truncated = true;
        }
    }

    Ok(LimitedRead {
        bytes: stored,
        truncated,
    })
}

fn copy_allowed_environment(command: &mut Command) {
    for key in [
        "HOME",
        "PATH",
        "XDG_CONFIG_HOME",
        "XDG_CACHE_HOME",
        "XDG_DATA_HOME",
        "LANG",
        "LC_ALL",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;

    fn runner_test_lock() -> &'static tokio::sync::Mutex<()> {
        static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    fn fake_executable(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        let mut file = fs::File::create(&path).expect("fake executable");
        file.write_all(format!("#!/bin/sh\n{body}\n").as_bytes())
            .expect("write fake executable");
        file.sync_all().expect("sync fake executable");
        drop(file);
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&path, permissions).expect("chmod");
        path
    }

    fn spec(args: Vec<&str>) -> CommandSpec {
        CommandSpec {
            kind: CommandKind::Usage,
            args: args.into_iter().map(str::to_string).collect(),
            timeout: Duration::from_secs(2),
            max_stdout_bytes: 1024,
            max_stderr_bytes: 1024,
        }
    }

    #[tokio::test]
    async fn runner_uses_exact_argv_and_separate_streams() {
        let _guard = runner_test_lock().lock().await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let argv_file = tmp.path().join("argv.txt");
        let script = fake_executable(
            tmp.path(),
            "codexbar",
            &format!(
                "printf '%s\\n' \"$@\" > {}\nprintf '{{\"ok\":true}}\\n'\nprintf 'diagnostic\\n' >&2",
                argv_file.display()
            ),
        );
        let output = CommandRunner
            .run(
                &script,
                &spec(vec!["--format", "json", "--provider", "codex"]),
            )
            .await
            .expect("run");
        assert!(output.success());
        assert_eq!(
            fs::read_to_string(argv_file).expect("argv"),
            "--format\njson\n--provider\ncodex\n"
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), "{\"ok\":true}\n");
        assert_eq!(String::from_utf8_lossy(&output.stderr), "diagnostic\n");
    }

    #[tokio::test]
    async fn runner_excludes_secret_and_proxy_environment() {
        let _guard = runner_test_lock().lock().await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let script = fake_executable(
            tmp.path(),
            "codexbar",
            "env | sort | grep -E 'TOKEN|SECRET|KEY|COOKIE|AUTH|PROXY|proxy' || true",
        );
        std::env::set_var("OPENAI_API_KEY", "sk-test");
        std::env::set_var("GITHUB_TOKEN", "ghp_secret");
        std::env::set_var("HTTP_PROXY", "http://user:password@proxy.invalid");
        std::env::set_var("COOKIE", "session=secret");
        let output = CommandRunner
            .run(&script, &spec(Vec::new()))
            .await
            .expect("run");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("GITHUB_TOKEN");
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("COOKIE");
        assert!(output.stdout.is_empty(), "secret env leaked to child");
    }

    #[tokio::test]
    async fn runner_times_out_and_reaps_child() {
        let _guard = runner_test_lock().lock().await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let script = fake_executable(tmp.path(), "codexbar", "sleep 5");
        let mut timeout_spec = spec(Vec::new());
        timeout_spec.timeout = Duration::from_millis(50);
        let output = CommandRunner
            .run(&script, &timeout_spec)
            .await
            .expect("run");
        assert!(output.timed_out);
    }

    #[tokio::test]
    async fn runner_truncates_stdout_and_stderr_independently() {
        let _guard = runner_test_lock().lock().await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let script = fake_executable(
            tmp.path(),
            "codexbar",
            "printf '1234567890'; printf 'abcdefghij' >&2",
        );
        let mut limited = spec(Vec::new());
        limited.max_stdout_bytes = 4;
        limited.max_stderr_bytes = 3;
        let output = CommandRunner.run(&script, &limited).await.expect("run");
        assert_eq!(&output.stdout, b"1234");
        assert_eq!(&output.stderr, b"abc");
        assert!(output.stdout_truncated);
        assert!(output.stderr_truncated);
    }
}
