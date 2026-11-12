//! The application's cookie jar kept between runs in the store's `app_state` table.

use rqm_http::CookieJar;
use rqm_store::{Store, StoreError};

/// Key of the `app_state` row that holds the cookie jar as JSON ([`CookieJar::to_json`]).
pub const COOKIE_JAR_KEY: &str = "cookie_jar";

/// Empty jar when nothing is stored or the stored value is unreadable.
pub fn load_cookies(store: &Store) -> Result<CookieJar, StoreError> {
    Ok(store
        .state_get(COOKIE_JAR_KEY)?
        .and_then(|json| CookieJar::from_json(&json).ok())
        .unwrap_or_else(CookieJar::new))
}

/// Writes the unexpired cookies of `jar` to the store, replacing what was stored before.
pub fn save_cookies(store: &mut Store, jar: &CookieJar) -> Result<(), StoreError> {
    store.state_set(COOKIE_JAR_KEY, &jar.to_json())
}
