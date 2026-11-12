//! Cookie jar of the engine: keeps cookies from `Set-Cookie` headers and picks the ones to send (RFC 6265).

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::types::HttpResponse;

/// One cookie, as stored in the jar and as shown to the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cookie {
    /// Name from `Set-Cookie`.
    pub name: String,
    /// Value from `Set-Cookie`, as received (quotes included).
    pub value: String,
    /// Host the cookie belongs to. Without a `Domain` attribute it is the host of the request and the cookie
    /// is sent to that host only; with one it is the attribute with a leading dot (`.example.com`) and the
    /// cookie is also sent to subdomains.
    pub domain: String,
    /// `Path` attribute, or the directory of the request path when the attribute is absent.
    pub path: String,
    /// Expiry as Unix seconds, from `Max-Age` (counted from the moment the header was read) or `Expires`;
    /// `None` for a session cookie.
    pub expires_unix: Option<i64>,
    /// `Secure` attribute: the cookie is sent over `https` only.
    pub secure: bool,
    /// `HttpOnly` attribute; kept for display, it does not change what the engine sends.
    pub http_only: bool,
}

impl Cookie {
    fn expired(&self, now: i64) -> bool {
        self.expires_unix.is_some_and(|expires| expires <= now)
    }

    /// Whether two cookies are the same cookie: the newer one replaces the older.
    fn same_key(&self, other: &Cookie) -> bool {
        self.name == other.name && self.domain == other.domain && self.path == other.path
    }

    /// Whether the cookie is sent with a request to `url`.
    fn matches(&self, url: &Url) -> bool {
        let Some(host) = url.host_str() else { return false };
        let domain_matches = match self.domain.strip_prefix('.') {
            Some(domain) => host == domain || (url.domain().is_some() && host.ends_with(&self.domain)),
            None => host == self.domain,
        };
        domain_matches && path_matches(url.path(), &self.path) && (!self.secure || url.scheme() == "https")
    }
}

/// The cookies of the application, shared by all requests of an engine. Thread-safe: share it behind an `Arc`.
///
/// The engine adds cookies from every response and sends the matching ones with every request; the
/// application saves [`CookieJar::to_json`] and restores the jar with [`CookieJar::from_json`].
#[derive(Default)]
pub struct CookieJar {
    /// In the order the cookies were first set.
    cookies: Mutex<Vec<Cookie>>,
}

impl std::fmt::Debug for CookieJar {
    /// Shows the number of cookies only: their values are secrets.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CookieJar").field("cookies", &self.lock().len()).finish()
    }
}

impl CookieJar {
    /// An empty jar.
    pub fn new() -> Self {
        Self::default()
    }

    /// Restores a jar from the text made by [`CookieJar::to_json`]; cookies that expired meanwhile are dropped.
    /// Unparseable input yields an error; the caller then starts with an empty jar.
    pub fn from_json(json: &str) -> Result<Self, String> {
        // The position only: the text of a `serde_json` error may quote a value from the input.
        let mut cookies: Vec<Cookie> = serde_json::from_str(json)
            .map_err(|e| format!("invalid cookie jar JSON at line {} column {}", e.line(), e.column()))?;
        let now = now();
        cookies.retain(|cookie| !cookie.expired(now));
        Ok(CookieJar { cookies: Mutex::new(cookies) })
    }

    /// All unexpired cookies, session cookies included, as a JSON array of [`Cookie`] objects.
    pub fn to_json(&self) -> String {
        // Cannot fail: a `Cookie` holds only strings, a number and booleans.
        serde_json::to_string(&self.list()).expect("cookies serialise to JSON")
    }

    /// All unexpired cookies in the order they were first set.
    pub fn list(&self) -> Vec<Cookie> {
        self.unexpired().clone()
    }

    /// Removes every cookie.
    pub fn clear(&self) {
        self.lock().clear();
    }

    /// Takes one `Set-Cookie` header of a response to a request to `url`. A header that cannot be parsed or
    /// names a foreign domain is ignored; an already expired cookie only removes the stored one.
    pub(crate) fn store(&self, url: &Url, set_cookie: &str) {
        let now = now();
        let Some(cookie) = parse_set_cookie(set_cookie, url, now) else { return };
        let mut cookies = self.lock();
        let existing = cookies.iter().position(|stored| stored.same_key(&cookie));
        match (existing, cookie.expired(now)) {
            (Some(index), true) => drop(cookies.remove(index)),
            // Replaced in place: a cookie keeps its position (RFC 6265: its creation time).
            (Some(index), false) => cookies[index] = cookie,
            (None, true) => {}
            (None, false) => cookies.push(cookie),
        }
    }

    /// Value of the `Cookie` header for a request to `url`: `name=value` pairs joined with `; `, longer paths
    /// first, then older cookies first. `None` when no cookie matches.
    pub(crate) fn header_for(&self, url: &Url) -> Option<String> {
        let cookies = self.unexpired();
        let mut matching: Vec<&Cookie> = cookies.iter().filter(|cookie| cookie.matches(url)).collect();
        if matching.is_empty() {
            return None;
        }
        // Stable: cookies with equal path lengths stay in the order they were set.
        matching.sort_by_key(|cookie| std::cmp::Reverse(cookie.path.len()));
        let pairs: Vec<String> = matching.iter().map(|cookie| format!("{}={}", cookie.name, cookie.value)).collect();
        Some(pairs.join("; "))
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Cookie>> {
        // A poisoned lock only means another thread panicked between two list operations; the list is intact.
        self.cookies.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The list without the cookies that have expired by now.
    fn unexpired(&self) -> MutexGuard<'_, Vec<Cookie>> {
        let now = now();
        let mut cookies = self.lock();
        cookies.retain(|cookie| !cookie.expired(now));
        cookies
    }
}

/// Cookies set by the final response (its `Set-Cookie` headers), with domain and path defaulted from
/// `response.sent.url`. Feeds the Cookies tab of the response panel.
///
/// Expired cookies (a server deleting a cookie) are listed too; headers the jar ignores are not.
/// `expires_unix` of a `Max-Age` cookie is counted from the moment of this call.
pub fn response_cookies(response: &HttpResponse) -> Vec<Cookie> {
    let Ok(url) = Url::parse(&response.sent.url) else { return Vec::new() };
    let now = now();
    response
        .headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
        .filter_map(|(_, value)| parse_set_cookie(value, &url, now))
        .collect()
}

/// Current time as Unix seconds.
fn now() -> i64 {
    // A clock before 1970 counts as 1970: nothing has expired yet.
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs() as i64)
}

/// Parses one `Set-Cookie` header received from `url` (RFC 6265 §5.2, §5.3). `None` when the header has no
/// `name=value` pair or its `Domain` does not cover the host of `url`.
///
/// There is no public suffix list: a `Domain` of one label (`com`, `localhost`) is accepted only when it is
/// the host itself.
fn parse_set_cookie(header: &str, url: &Url, now: i64) -> Option<Cookie> {
    let host = url.host_str()?;
    let mut pieces = header.split(';');
    let (name, value) = pieces.next()?.split_once('=')?;
    let (name, value) = (name.trim(), value.trim());
    if name.is_empty() {
        return None;
    }

    let (mut domain, mut path, mut expires, mut max_age) = (None, None, None, None);
    let (mut secure, mut http_only) = (false, false);
    for piece in pieces {
        let (attribute, text) = piece.split_once('=').unwrap_or((piece, ""));
        let text = text.trim();
        match attribute.trim().to_ascii_lowercase().as_str() {
            // An unparseable date or number leaves the attribute out; the last occurrence wins.
            "expires" => expires = parse_date(text).or(expires),
            "max-age" => max_age = parse_max_age(text).or(max_age),
            "domain" if !text.is_empty() => domain = Some(text.strip_prefix('.').unwrap_or(text).to_ascii_lowercase()),
            "path" => path = text.starts_with('/').then(|| text.to_string()),
            "secure" => secure = true,
            "httponly" => http_only = true,
            _ => {}
        }
    }

    let domain = match domain {
        None => host.to_string(),
        // An IP address or a one-label host has no subdomains: the cookie stays with the host.
        Some(domain) if domain == host && (url.domain().is_none() || !host.contains('.')) => host.to_string(),
        Some(domain) if !domain.contains('.') => return None,
        Some(domain) if domain == host || (url.domain().is_some() && host.ends_with(&format!(".{domain}"))) => {
            format!(".{domain}")
        }
        Some(_) => return None,
    };
    // `Max-Age` wins over `Expires`; zero or negative means "already expired".
    let expires_unix = match max_age {
        Some(seconds) if seconds <= 0 => Some(0),
        Some(seconds) => Some(now.saturating_add(seconds)),
        None => expires,
    };
    Some(Cookie {
        name: name.to_string(),
        value: value.to_string(),
        domain,
        path: path.unwrap_or_else(|| default_path(url.path())),
        expires_unix,
        secure,
        http_only,
    })
}

/// Value of `Max-Age`: digits with an optional minus; a number too long for `i64` is clamped.
fn parse_max_age(text: &str) -> Option<i64> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(text.parse().unwrap_or(if text.starts_with('-') { i64::MIN } else { i64::MAX }))
}

/// Default path of a cookie: the request path up to its last `/` (RFC 6265 §5.1.4).
fn default_path(request_path: &str) -> String {
    match request_path.rfind('/') {
        Some(last) if last > 0 && request_path.starts_with('/') => request_path[..last].to_string(),
        _ => "/".to_string(),
    }
}

/// Whether a request path is inside a cookie path (RFC 6265 §5.1.4).
fn path_matches(request_path: &str, cookie_path: &str) -> bool {
    match request_path.strip_prefix(cookie_path) {
        Some(rest) => rest.is_empty() || rest.starts_with('/') || cookie_path.ends_with('/'),
        None => false,
    }
}

/// Parses a cookie date into Unix seconds by the tolerant algorithm of RFC 6265 §5.1.1: the time, the day,
/// the month and the year are found among the tokens in any order.
fn parse_date(text: &str) -> Option<i64> {
    const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let is_delimiter =
        |c: char| matches!(c, '\t' | ' '..='/' | ';'..='@' | '['..='`' | '{'..='~');
    let (mut time, mut day, mut month, mut year) = (None, None, None, None);
    for token in text.split(is_delimiter).filter(|token| !token.is_empty()) {
        if time.is_none() {
            time = parse_time(token);
            if time.is_some() {
                continue;
            }
        }
        if day.is_none() {
            day = leading_number(token, 1, 2);
            if day.is_some() {
                continue;
            }
        }
        if month.is_none() {
            let prefix = token.get(..3).map(str::to_ascii_lowercase);
            month = prefix.and_then(|prefix| MONTHS.iter().position(|month| *month == prefix)).map(|index| index as i64 + 1);
            if month.is_some() {
                continue;
            }
        }
        if year.is_none() {
            year = leading_number(token, 2, 4);
        }
    }
    let ((hour, minute, second), day, month, year) = (time?, day?, month?, year?);
    let year = match year {
        70..=99 => year + 1900,
        0..=69 => year + 2000,
        _ => year,
    };
    if !(1..=31).contains(&day) || year < 1601 || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// `hh:mm:ss` with one or two digits each; characters after the seconds are ignored.
fn parse_time(token: &str) -> Option<(i64, i64, i64)> {
    let mut fields = token.splitn(3, ':');
    let (hour, minute, second) = (fields.next()?, fields.next()?, fields.next()?);
    let whole = |field: &str| leading_number(field, 1, 2).filter(|_| field.len() <= 2);
    Some((whole(hour)?, whole(minute)?, leading_number(second, 1, 2)?))
}

/// The number a token starts with, when it has from `min` to `max` digits.
fn leading_number(token: &str, min: usize, max: usize) -> Option<i64> {
    let digits = token.bytes().take_while(u8::is_ascii_digit).count();
    if !(min..=max).contains(&digits) {
        return None;
    }
    token[..digits].parse().ok()
}

/// Days from 1970-01-01 to a date of the Gregorian calendar.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn dates_are_parsed_in_the_usual_formats() {
        assert_eq!(parse_date("Wed, 21 Oct 2015 07:28:00 GMT"), Some(1_445_412_480));
        assert_eq!(parse_date("Wednesday, 21-Oct-15 07:28:00 GMT"), Some(1_445_412_480));
        assert_eq!(parse_date("Wed Oct 21 07:28:00 2015"), Some(1_445_412_480));
        assert_eq!(parse_date("Thu, 01 Jan 1970 00:00:00 GMT"), Some(0));
        assert_eq!(parse_date("Tue, 29 Feb 2000 12:00:00 GMT"), Some(951_825_600));
        assert_eq!(parse_date("soon"), None);
        assert_eq!(parse_date("21 Oct 2015 25:00:00"), None);
    }

    #[test]
    fn domain_path_secure_and_expiry_rules() {
        let from = url("https://app.example.com/a/b/page");
        let cookie = parse_set_cookie("k=v; Domain=.Example.com; Secure; HttpOnly", &from, 100).unwrap();
        assert_eq!((cookie.domain.as_str(), cookie.path.as_str()), (".example.com", "/a/b"));
        assert!(cookie.secure && cookie.http_only && cookie.expires_unix.is_none());
        assert!(cookie.matches(&url("https://example.com/a/b")));
        assert!(cookie.matches(&url("https://x.example.com/a/b/c")));
        assert!(!cookie.matches(&url("https://badexample.com/a/b")), "another domain");
        assert!(!cookie.matches(&url("http://example.com/a/b")), "Secure needs https");
        assert!(!cookie.matches(&url("https://example.com/a/bc")), "path prefix without a slash");

        let host_only = parse_set_cookie("k=v", &from, 100).unwrap();
        assert_eq!(host_only.domain, "app.example.com");
        assert!(!host_only.matches(&url("https://x.app.example.com/a/b")));

        assert!(parse_set_cookie("k=v; Domain=other.test", &from, 100).is_none(), "foreign domain");
        assert!(parse_set_cookie("k=v; Domain=com", &from, 100).is_none(), "top-level domain");
        assert!(parse_set_cookie("novalue", &from, 100).is_none());
        assert_eq!(parse_set_cookie("k=v; Max-Age=50; Expires=Thu, 01 Jan 1970 00:00:00 GMT", &from, 100).unwrap().expires_unix, Some(150));
        assert_eq!(parse_set_cookie("k=v; Max-Age=-1", &from, 100).unwrap().expires_unix, Some(0));
    }

    #[test]
    fn expired_cookie_removes_the_stored_one_and_replacement_keeps_the_place() {
        let jar = CookieJar::new();
        let from = url("http://127.0.0.1:8080/");
        jar.store(&from, "a=1");
        jar.store(&from, "b=1; Path=/deep");
        jar.store(&from, "a=2");
        assert_eq!(jar.header_for(&url("http://127.0.0.1:9090/deep/x")).unwrap(), "b=1; a=2", "longer path first; port ignored");
        jar.store(&from, "a=; Expires=Thu, 01 Jan 1970 00:00:00 GMT");
        assert_eq!(jar.list().iter().map(|cookie| cookie.name.as_str()).collect::<Vec<_>>(), vec!["b"]);
        assert_eq!(jar.header_for(&from), None);
    }
}
