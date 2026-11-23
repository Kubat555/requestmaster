//! The application settings of [`AppCore`]: what every send takes — the timeout, the certificate check, the
//! proxy, whether scripts run and for how long — and the motion level of the window, which the core only keeps.

use std::time::Duration;

use rqm_http::{resolve_proxy, HttpError, ProxySetting};
use rqm_runner::AppSettings;
use rqm_store::Store;

use crate::core::{message, AppCore};
use crate::dto::{MotionLevel, ProxyDto, SettingsDto};

/// Key of `app_state` holding the application settings: the JSON of [`SettingsDto`]. Written by
/// `save_settings`; read by `get_settings` and by every send.
pub const SETTINGS_KEY: &str = "settings";

/// The longest timeout that can be saved, in milliseconds: one hour.
pub const MAX_TIMEOUT_MS: u64 = 3_600_000;

/// The shortest time limit of a script that can be saved, in milliseconds.
pub const MIN_SCRIPT_TIMEOUT_MS: u64 = 100;

/// The longest time limit of a script that can be saved, in milliseconds: ten minutes.
pub const MAX_SCRIPT_TIMEOUT_MS: u64 = 600_000;

/// The time limit of a script while nothing else is saved, in milliseconds.
pub const DEFAULT_SCRIPT_TIMEOUT_MS: u64 = 10_000;

impl AppCore {
    /// The application settings; the defaults (no timeout, the certificate is verified, the system proxy,
    /// the normal motion, scripts run for 10 s at most) while nothing is saved or the saved value cannot be
    /// read.
    pub fn get_settings(&self) -> Result<SettingsDto, String> {
        let value = self.store()?.state_get(SETTINGS_KEY).map_err(message)?;
        Ok(value.as_deref().and_then(read_settings).unwrap_or_else(default_settings))
    }

    /// Saves the application settings; they are used from the next send on. A timeout above
    /// [`MAX_TIMEOUT_MS`], a time limit of a script outside [`MIN_SCRIPT_TIMEOUT_MS`]…[`MAX_SCRIPT_TIMEOUT_MS`]
    /// and a manual proxy address that is not an `http://` or `https://` URL are refused, and the saved
    /// settings stay as they were. The proxy address is saved trimmed.
    pub fn save_settings(&self, mut settings: SettingsDto) -> Result<(), String> {
        if settings.timeout_ms > MAX_TIMEOUT_MS {
            return Err("timeout is out of range".to_string());
        }
        if !(MIN_SCRIPT_TIMEOUT_MS..=MAX_SCRIPT_TIMEOUT_MS).contains(&settings.script_timeout_ms) {
            return Err("script timeout is out of range".to_string());
        }
        if let ProxyDto::Manual { url } = &mut settings.proxy {
            *url = url.trim().to_string();
            if let Err(error) = resolve_proxy(&ProxySetting::Manual(url.clone()), None) {
                let reason = match error {
                    // The engine's text starts with `proxy: `, which the message already says.
                    HttpError::InvalidUrl(reason) => reason.trim_start_matches("proxy: ").to_string(),
                    other => other.to_string(),
                };
                return Err(format!("invalid proxy address: {reason}"));
            }
        }
        let value = serde_json::to_string(&settings).map_err(message)?;
        self.store()?.state_set(SETTINGS_KEY, &value).map_err(message)
    }
}

impl From<SettingsDto> for AppSettings {
    fn from(settings: SettingsDto) -> Self {
        AppSettings {
            timeout: (settings.timeout_ms > 0).then(|| Duration::from_millis(settings.timeout_ms)),
            verify_tls: settings.verify_tls,
            proxy: match settings.proxy {
                ProxyDto::System => ProxySetting::System,
                ProxyDto::Off => ProxySetting::Off,
                ProxyDto::Manual { url } => ProxySetting::Manual(url),
            },
            scripts_enabled: settings.scripts_enabled,
            script_timeout: Duration::from_millis(settings.script_timeout_ms),
        }
    }
}

/// The stored settings as the send takes them; absent or unreadable — `AppSettings::default()`.
pub(crate) fn app_settings_in(store: &Store) -> AppSettings {
    match store.state_get(SETTINGS_KEY) {
        Ok(Some(value)) => read_settings(&value).map(AppSettings::from).unwrap_or_default(),
        // A store that cannot be read stops the send by itself, right after this, with its own message.
        Ok(None) | Err(_) => AppSettings::default(),
    }
}

/// The settings stored as `value`; `None` when the value is not the JSON of [`SettingsDto`].
fn read_settings(value: &str) -> Option<SettingsDto> {
    serde_json::from_str(value).ok()
}

/// The settings before anything was saved; for a send the same as `AppSettings::default()`.
fn default_settings() -> SettingsDto {
    SettingsDto {
        timeout_ms: 0,
        verify_tls: true,
        proxy: ProxyDto::System,
        motion: MotionLevel::Normal,
        scripts_enabled: true,
        script_timeout_ms: DEFAULT_SCRIPT_TIMEOUT_MS,
    }
}
