use std::sync::Arc;

use zbus::{interface, ConnectionBuilder};

use crate::app::{App, RefreshStart};
use crate::error::AppError;
use crate::{DBUS_NAME, DBUS_OBJECT_PATH};

pub struct CodexbarInterface {
    app: Arc<App>,
}

impl CodexbarInterface {
    pub fn new(app: Arc<App>) -> Self {
        Self { app }
    }
}

#[interface(name = "org.codexbar.Linux1")]
impl CodexbarInterface {
    async fn get_snapshot(&self) -> Result<String, AppError> {
        self.app.get_snapshot_json()
    }

    async fn refresh(
        &self,
        options_json: &str,
        #[zbus(signal_context)] ctxt: zbus::object_server::SignalContext<'_>,
    ) -> Result<String, AppError> {
        let start = self.app.start_refresh(options_json)?;
        match start {
            RefreshStart::Started { refresh_id } => {
                let app = Arc::clone(&self.app);
                let ctxt = ctxt.to_owned();
                let spawned_refresh_id = refresh_id.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                    let _ = Self::refresh_started(&ctxt, &spawned_refresh_id).await;
                    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
                    if let Ok(completion) = app.finish_refresh(&spawned_refresh_id).await {
                        for (provider_id, provider_event_json) in completion.provider_events {
                            let _ =
                                Self::provider_changed(&ctxt, &provider_id, &provider_event_json)
                                    .await;
                        }
                        let _ = Self::snapshot_changed(&ctxt, &completion.snapshot_json).await;
                        let _ = Self::refresh_finished(
                            &ctxt,
                            &completion.refresh_id,
                            &completion.result_json,
                        )
                        .await;
                    }
                });
                Ok(refresh_id)
            }
            RefreshStart::Existing { refresh_id } => Ok(refresh_id),
        }
    }

    async fn get_diagnostics(&self, provider_id: &str) -> Result<String, AppError> {
        self.app.get_diagnostics_json(provider_id)
    }

    async fn get_daemon_info(&self) -> Result<String, AppError> {
        self.app.get_daemon_info_json()
    }

    async fn set_settings_patch(&self, patch_json: &str) -> Result<String, AppError> {
        self.app.set_settings_patch_json(patch_json)
    }

    async fn test_browser_import(&self, options_json: &str) -> Result<String, AppError> {
        self.app.test_browser_import_json(options_json)
    }

    #[zbus(signal)]
    pub async fn snapshot_changed(
        ctxt: &zbus::object_server::SignalContext<'_>,
        snapshot_json: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn refresh_started(
        ctxt: &zbus::object_server::SignalContext<'_>,
        refresh_id: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn refresh_finished(
        ctxt: &zbus::object_server::SignalContext<'_>,
        refresh_id: &str,
        result_json: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn provider_changed(
        ctxt: &zbus::object_server::SignalContext<'_>,
        provider_id: &str,
        provider_event_json: &str,
    ) -> zbus::Result<()>;
}

pub async fn serve_until_shutdown() -> Result<(), Box<dyn std::error::Error>> {
    let app = Arc::new(App::from_env()?);
    let _connection = ConnectionBuilder::session()?
        .name(DBUS_NAME)?
        .serve_at(DBUS_OBJECT_PATH, CodexbarInterface::new(app))?
        .build()
        .await?;

    wait_for_shutdown().await;
    Ok(())
}

async fn wait_for_shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigint = signal(SignalKind::interrupt()).expect("SIGINT handler");
        let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = sigint.recv() => {},
            _ = sigterm.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
