use crate::model::SemanticSource;

pub fn map_upstream_source(value: Option<&str>) -> SemanticSource {
    let Some(value) = value else {
        return SemanticSource::Unknown;
    };
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return SemanticSource::Unknown;
    }
    if matches!(
        normalized.as_str(),
        "codex-cli" | "claude" | "cli" | "local"
    ) || normalized.starts_with("local-")
        || normalized.ends_with("-cli")
        || normalized.contains("cli")
    {
        return SemanticSource::Local;
    }
    if matches!(normalized.as_str(), "openai-web" | "web")
        || normalized.contains("web")
        || normalized.contains("browser")
    {
        return SemanticSource::Web;
    }
    if matches!(normalized.as_str(), "oauth" | "api")
        || normalized.contains("api")
        || normalized.contains("oauth")
    {
        return SemanticSource::Api;
    }
    SemanticSource::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_v0330_upstream_source_labels() {
        for label in ["codex-cli", "claude", "cli", "local"] {
            assert_eq!(
                map_upstream_source(Some(label)),
                SemanticSource::Local,
                "{label} should map to local semantic source"
            );
        }
        for label in ["openai-web", "web"] {
            assert_eq!(
                map_upstream_source(Some(label)),
                SemanticSource::Web,
                "{label} should map to web semantic source"
            );
        }
        for label in ["oauth", "api", "oauth-api"] {
            assert_eq!(
                map_upstream_source(Some(label)),
                SemanticSource::Api,
                "{label} should map to api semantic source"
            );
        }
        assert_eq!(map_upstream_source(None), SemanticSource::Unknown);
        assert_eq!(map_upstream_source(Some("")), SemanticSource::Unknown);
    }
}
