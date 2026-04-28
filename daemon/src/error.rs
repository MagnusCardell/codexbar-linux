use zbus::DBusError;

#[derive(Debug, DBusError)]
#[zbus(prefix = "org.codexbar.Linux1.Error", impl_display = true)]
pub enum AppError {
    InvalidJson(String),
    InvalidSettingsPatch(String),
    RefreshBusy(String),
    DependencyUnavailable(String),
    CapabilityUnimplemented(String),
    Internal(String),
    #[zbus(error)]
    ZBus(zbus::Error),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn invalid_json() -> Self {
        Self::InvalidJson("input JSON is invalid or does not match the schema".to_string())
    }

    pub fn invalid_settings_patch(message: impl Into<String>) -> Self {
        Self::InvalidSettingsPatch(message.into())
    }

    pub fn refresh_busy(refresh_id: &str) -> Self {
        Self::RefreshBusy(format!("refresh already active: {refresh_id}"))
    }

    pub fn internal_redacted() -> Self {
        Self::Internal("internal daemon error; details redacted".to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(_: std::io::Error) -> Self {
        Self::internal_redacted()
    }
}

impl From<serde_json::Error> for AppError {
    fn from(_: serde_json::Error) -> Self {
        Self::invalid_json()
    }
}
