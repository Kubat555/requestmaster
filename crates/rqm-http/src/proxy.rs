//! Proxy choice: the rule that turns the setting into a proxy address, and the lookup of the system proxy.

use hyper_util::client::proxy::matcher::Matcher;
use reqwest::header::HeaderValue;
use reqwest::Url;

use crate::types::{HttpError, ProxySetting};

/// The proxy address to use for one request, `None` for a direct connection.
///
/// `system` is the proxy the operating system gives for the request's URL (the caller looks it up); it is used
/// only by [`ProxySetting::System`]. [`ProxySetting::Off`] always connects directly. A
/// [`ProxySetting::Manual`] address is returned as typed and must be an `http://` or `https://` URL,
/// otherwise the result is [`HttpError::InvalidUrl`].
pub fn resolve_proxy(setting: &ProxySetting, system: Option<String>) -> Result<Option<String>, HttpError> {
    match setting {
        ProxySetting::System => Ok(system),
        ProxySetting::Off => Ok(None),
        ProxySetting::Manual(address) => match Url::parse(address) {
            Ok(parsed) if matches!(parsed.scheme(), "http" | "https") && parsed.has_host() => Ok(Some(address.clone())),
            Ok(parsed) => Err(HttpError::InvalidUrl(format!("proxy: unsupported scheme `{}`", parsed.scheme()))),
            Err(e) => Err(HttpError::InvalidUrl(format!("proxy: {e}"))),
        },
    }
}

/// A proxy to connect through. Part of the key of the engine's client cache.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ProxyTarget {
    /// Address of the proxy.
    pub address: String,
    /// `Proxy-Authorization` value of a system proxy configured with a user name and a password; the system
    /// lookup returns it apart from the address. A manual address carries its credentials inside the URL.
    pub auth: Option<HeaderValue>,
}

/// The proxy configuration of the operating system, read once per send: the environment variables
/// (`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY`), then the static proxy of the Windows Internet
/// settings with its bypass list. PAC/WPAD is not supported.
pub(crate) struct SystemProxy(Matcher);

impl SystemProxy {
    /// Reads the current configuration.
    pub(crate) fn detect() -> Self {
        // The same matcher `reqwest` uses internally with its `system-proxy` feature, but does not expose.
        SystemProxy(Matcher::from_system())
    }

    /// The system proxy for `url`, `None` when the URL is to be reached directly.
    fn lookup(&self, url: &Url) -> Option<ProxyTarget> {
        let destination: http::Uri = url.as_str().parse().ok()?;
        let intercept = self.0.intercept(&destination)?;
        Some(ProxyTarget { address: intercept.uri().to_string(), auth: intercept.basic_auth().cloned() })
    }
}

/// The proxy for a request to `url` under `setting`; `system` is needed only for [`ProxySetting::System`].
pub(crate) fn target(
    setting: &ProxySetting,
    system: Option<&SystemProxy>,
    url: &Url,
) -> Result<Option<ProxyTarget>, HttpError> {
    let found = match setting {
        ProxySetting::System => system.and_then(|system| system.lookup(url)),
        ProxySetting::Off | ProxySetting::Manual(_) => None,
    };
    let (address, auth) = match found {
        Some(ProxyTarget { address, auth }) => (Some(address), auth),
        None => (None, None),
    };
    // `auth` belongs to the system address: it is `Some` only when `resolve_proxy` returns that address.
    Ok(resolve_proxy(setting, address)?.map(|address| ProxyTarget { address, auth }))
}
