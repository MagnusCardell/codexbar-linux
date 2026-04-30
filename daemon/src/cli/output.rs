use serde_json::Value;

use super::runner::CommandOutput;

#[derive(Clone, Debug, PartialEq)]
pub enum OutputClassification {
    Empty,
    SingleJson(Value),
    MultipleJsonDocuments,
    NonJsonText,
    Truncated,
    Binary,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StderrClassification {
    Empty,
    Text,
    Json,
    Truncated,
    Binary,
}

pub(crate) fn classify_stdout(output: &CommandOutput) -> OutputClassification {
    classify_json_like(&output.stdout, output.stdout_truncated)
}

pub(crate) fn classify_stderr(output: &CommandOutput) -> StderrClassification {
    if output.stderr_truncated {
        return StderrClassification::Truncated;
    }
    if output.stderr.is_empty() {
        return StderrClassification::Empty;
    }
    if is_binary(&output.stderr) {
        return StderrClassification::Binary;
    }
    let Ok(text) = std::str::from_utf8(&output.stderr) else {
        return StderrClassification::Binary;
    };
    if serde_json::from_str::<Value>(text).is_ok() {
        StderrClassification::Json
    } else {
        StderrClassification::Text
    }
}

pub(crate) fn classify_json_like(bytes: &[u8], truncated: bool) -> OutputClassification {
    if truncated {
        return OutputClassification::Truncated;
    }
    if bytes.is_empty() {
        return OutputClassification::Empty;
    }
    if is_binary(bytes) {
        return OutputClassification::Binary;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return OutputClassification::Binary;
    };
    if text.trim().is_empty() {
        return OutputClassification::Empty;
    }
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        return OutputClassification::SingleJson(value);
    }

    let mut count = 0usize;
    for value in serde_json::Deserializer::from_str(text).into_iter::<Value>() {
        if value.is_err() {
            break;
        }
        count += 1;
    }
    if count > 1 {
        return OutputClassification::MultipleJsonDocuments;
    }

    OutputClassification::NonJsonText
}

pub fn diagnostic_code_for_stdout(classification: &OutputClassification) -> &'static str {
    match classification {
        OutputClassification::Empty => "upstream_cli_empty_stdout",
        OutputClassification::SingleJson(_) => "upstream_cli_ok",
        OutputClassification::MultipleJsonDocuments => "upstream_cli_parse_error",
        OutputClassification::NonJsonText => "upstream_cli_parse_error",
        OutputClassification::Truncated => "upstream_cli_output_truncated",
        OutputClassification::Binary => "upstream_cli_parse_error",
    }
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes.contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::runner::{CommandKind, CommandOutput};

    fn output(stdout: &[u8], truncated: bool) -> CommandOutput {
        CommandOutput {
            kind: CommandKind::Usage,
            exit_code: Some(0),
            timed_out: false,
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
            stdout_truncated: truncated,
            stderr_truncated: false,
            duration_ms: 1,
        }
    }

    #[test]
    fn classifies_single_json_array_and_object() {
        assert!(matches!(
            classify_stdout(&output(br#"[{"provider":"codex"}]"#, false)),
            OutputClassification::SingleJson(Value::Array(_))
        ));
        assert!(matches!(
            classify_stdout(&output(br#"{"provider":"codex"}"#, false)),
            OutputClassification::SingleJson(Value::Object(_))
        ));
    }

    #[test]
    fn classifies_empty_multiple_text_truncated_and_binary() {
        assert_eq!(
            classify_stdout(&output(b"", false)),
            OutputClassification::Empty
        );
        assert_eq!(
            classify_stdout(&output(b"[]\n[]\n", false)),
            OutputClassification::MultipleJsonDocuments
        );
        assert_eq!(
            classify_stdout(&output(b"not json", false)),
            OutputClassification::NonJsonText
        );
        assert_eq!(
            classify_stdout(&output(b"{", true)),
            OutputClassification::Truncated
        );
        assert_eq!(
            classify_stdout(&output(b"{\"x\":\"a\0b\"}", false)),
            OutputClassification::Binary
        );
    }
}
