pub mod app;
pub mod cache;
pub mod clock;
pub mod config;
pub mod dbus;
pub mod error;
pub mod fixtures;
pub mod model;
pub mod paths;
pub mod redact;

pub const APP_ID: &str = "codexbar-linux";
pub const DAEMON_NAME: &str = "codexbar-linuxd";
pub const DBUS_INTERFACE: &str = "org.codexbar.Linux1";
pub const DBUS_NAME: &str = DBUS_INTERFACE;
pub const DBUS_OBJECT_PATH: &str = "/org/codexbar/Linux1";
pub const EXTENSION_UUID: &str = "codexbar-linux@codexbar.dev";
pub const GSETTINGS_SCHEMA_ID: &str = "org.gnome.shell.extensions.codexbar-linux";

pub const UPSTREAM_CONFIG_PATH_HINT: &str = "~/.codexbar/config.json";
