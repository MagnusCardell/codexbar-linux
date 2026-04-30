use crate::model::{Provider, UpstreamCliInfo};

#[derive(Clone, Debug)]
pub struct AdapterSnapshotParts {
    pub providers: Vec<Provider>,
    pub upstream_cli: UpstreamCliInfo,
    pub diagnostics: Vec<AdapterDiagnostic>,
    pub usage_success: bool,
}

#[derive(Clone, Debug)]
pub struct AdapterDiagnostic {
    pub code: String,
    pub provider: Option<String>,
    pub safe_message: String,
    pub severity: AdapterDiagnosticSeverity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

impl AdapterDiagnostic {
    pub fn info(code: &str, safe_message: &str, provider: Option<String>) -> Self {
        Self {
            code: code.to_string(),
            provider,
            safe_message: safe_message.to_string(),
            severity: AdapterDiagnosticSeverity::Info,
        }
    }

    pub fn warning(code: &str, safe_message: &str, provider: Option<String>) -> Self {
        Self {
            code: code.to_string(),
            provider,
            safe_message: safe_message.to_string(),
            severity: AdapterDiagnosticSeverity::Warning,
        }
    }

    pub fn error(code: &str, safe_message: &str, provider: Option<String>) -> Self {
        Self {
            code: code.to_string(),
            provider,
            safe_message: safe_message.to_string(),
            severity: AdapterDiagnosticSeverity::Error,
        }
    }
}
