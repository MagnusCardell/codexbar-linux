use std::time::Duration;

use super::runner::{CommandKind, CommandSpec};

pub const DEFAULT_PROVIDER: &str = "codex";

const VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const USAGE_TIMEOUT: Duration = Duration::from_secs(90);
const STATUS_TIMEOUT: Duration = Duration::from_secs(90);
const COST_TIMEOUT: Duration = Duration::from_secs(30);
const STDOUT_LIMIT: usize = 4 * 1024 * 1024;
const STDERR_LIMIT: usize = 128 * 1024;

pub fn version() -> CommandSpec {
    CommandSpec {
        kind: CommandKind::Version,
        args: vec!["--version".to_string()],
        timeout: VERSION_TIMEOUT,
        max_stdout_bytes: 16 * 1024,
        max_stderr_bytes: 16 * 1024,
    }
}

pub fn provider_inventory() -> CommandSpec {
    CommandSpec {
        kind: CommandKind::ProviderInventory,
        args: vec!["--help".to_string()],
        timeout: VERSION_TIMEOUT,
        max_stdout_bytes: 64 * 1024,
        max_stderr_bytes: 16 * 1024,
    }
}

pub fn usage_default(provider: &str) -> CommandSpec {
    CommandSpec {
        kind: CommandKind::Usage,
        args: vec![
            "--format".to_string(),
            "json".to_string(),
            "--json-only".to_string(),
            "--provider".to_string(),
            provider.to_string(),
            "--source".to_string(),
            "cli".to_string(),
        ],
        timeout: USAGE_TIMEOUT,
        max_stdout_bytes: STDOUT_LIMIT,
        max_stderr_bytes: STDERR_LIMIT,
    }
}

pub fn usage_subcommand(provider: &str) -> CommandSpec {
    CommandSpec {
        kind: CommandKind::Usage,
        args: vec![
            "usage".to_string(),
            "--format".to_string(),
            "json".to_string(),
            "--json-only".to_string(),
            "--provider".to_string(),
            provider.to_string(),
            "--source".to_string(),
            "cli".to_string(),
        ],
        timeout: USAGE_TIMEOUT,
        max_stdout_bytes: STDOUT_LIMIT,
        max_stderr_bytes: STDERR_LIMIT,
    }
}

pub fn status(provider: &str) -> CommandSpec {
    CommandSpec {
        kind: CommandKind::Status,
        args: vec![
            "--format".to_string(),
            "json".to_string(),
            "--json-only".to_string(),
            "--provider".to_string(),
            provider.to_string(),
            "--source".to_string(),
            "cli".to_string(),
            "--status".to_string(),
        ],
        timeout: STATUS_TIMEOUT,
        max_stdout_bytes: STDOUT_LIMIT,
        max_stderr_bytes: STDERR_LIMIT,
    }
}

pub fn cost_both() -> CommandSpec {
    CommandSpec {
        kind: CommandKind::Cost,
        args: vec![
            "cost".to_string(),
            "--format".to_string(),
            "json".to_string(),
            "--json-only".to_string(),
            "--provider".to_string(),
            "both".to_string(),
        ],
        timeout: COST_TIMEOUT,
        max_stdout_bytes: STDOUT_LIMIT,
        max_stderr_bytes: STDERR_LIMIT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_matrix_matches_v0251_strategy() {
        assert_eq!(provider_inventory().args, ["--help"]);
        assert_eq!(
            usage_default("codex").args,
            [
                "--format",
                "json",
                "--json-only",
                "--provider",
                "codex",
                "--source",
                "cli"
            ]
        );
        assert_eq!(
            usage_subcommand("codex").args,
            [
                "usage",
                "--format",
                "json",
                "--json-only",
                "--provider",
                "codex",
                "--source",
                "cli"
            ]
        );
        assert_eq!(
            status("codex").args,
            [
                "--format",
                "json",
                "--json-only",
                "--provider",
                "codex",
                "--source",
                "cli",
                "--status"
            ]
        );
        assert_eq!(
            usage_default("claude").args,
            [
                "--format",
                "json",
                "--json-only",
                "--provider",
                "claude",
                "--source",
                "cli"
            ]
        );
        assert_eq!(
            cost_both().args,
            [
                "cost",
                "--format",
                "json",
                "--json-only",
                "--provider",
                "both"
            ]
        );
        assert!(
            !cost_both().args.iter().any(|arg| arg == "--source"),
            "cost command must not pass --source"
        );
    }
}
