use crate::model::SemanticSource;

pub fn map_upstream_source(value: Option<&str>) -> SemanticSource {
    let Some(value) = value else {
        return SemanticSource::Unknown;
    };
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return SemanticSource::Unknown;
    }
    if normalized == "local" || normalized.starts_with("local-") || normalized.contains("cli") {
        return SemanticSource::Local;
    }
    if normalized.contains("web") || normalized.contains("browser") {
        return SemanticSource::Web;
    }
    if normalized.contains("api") || normalized.contains("oauth") {
        return SemanticSource::Api;
    }
    SemanticSource::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_current_upstream_source_labels() {
        assert_eq!(
            map_upstream_source(Some("codex-cli")),
            SemanticSource::Local
        );
        assert_eq!(map_upstream_source(Some("local")), SemanticSource::Local);
        assert_eq!(map_upstream_source(Some("openai-web")), SemanticSource::Web);
        assert_eq!(map_upstream_source(Some("oauth")), SemanticSource::Api);
        assert_eq!(map_upstream_source(Some("api")), SemanticSource::Api);
    }
}
