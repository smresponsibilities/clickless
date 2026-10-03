//! Platform lifecycle dispatch; native socket APIs stay in backend crates.
#[cfg(target_os = "linux")]
pub use clickless_linux::lifecycle::{SettingsRequest, SingleInstance, notify_open_settings};
#[cfg(target_os = "macos")]
pub use clickless_macos::lifecycle::{SettingsRequest, SingleInstance, notify_open_settings};
