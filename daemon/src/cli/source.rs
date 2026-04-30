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
    if normalized.contains("api") {
        return SemanticSource::Api;
    }
    SemanticSource::Unknown
}
