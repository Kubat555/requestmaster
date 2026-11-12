//! Stage 0 spike: can reqwest 0.13 + rustls/ring cover the product's HTTP needs.
//! Throwaway code — not copied into the product.

use std::collections::HashMap;
use std::error::Error as _;
use std::io::Read;
use std::sync::{Mutex, Once, OnceLock};
use std::time::{Duration, Instant};

use reqwest::{header, Client, Method, Url};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub struct SendOptions {
    pub follow_redirects: bool,
    pub max_redirects: u32,
    pub verify_tls: bool,
    pub timeout: Duration,
}

/// A redirect response that was followed: its status and the URL that returned it.
#[derive(Debug)]
pub struct Hop {
    pub status: u16,
    pub url: String,
}

#[derive(Debug)]
pub struct Outcome {
    pub status: u16,
    pub hops: Vec<Hop>,
    /// From the start of `send` to the final response's headers.
    pub ttfb: Duration,
    /// From the start of `send` to the end of the final response's body.
    pub total: Duration,
    /// Body size as received, before content decoding.
    pub wire_bytes: u64,
    /// Decoded body.
    pub body: Vec<u8>,
}

#[derive(Debug)]
pub enum SendError {
    Cancelled,
    Timeout,
    Tls(String),
    Other(String),
}

pub async fn send(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: Option<Vec<u8>>,
    opts: &SendOptions,
    cancel: CancellationToken,
) -> Result<Outcome, SendError> {
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(SendError::Cancelled),
        r = tokio::time::timeout(opts.timeout, run(method, url, headers, body, opts)) => {
            r.unwrap_or(Err(SendError::Timeout))
        }
    }
}

/// Proxy the OS would use for an HTTPS request (env vars, then Windows Internet Settings).
/// reqwest applies the same matcher internally (`system-proxy` feature) but does not expose it.
pub fn system_proxy() -> Option<String> {
    let dst: http::Uri = "https://example.com/".parse().unwrap();
    hyper_util::client::proxy::matcher::Matcher::from_system()
        .intercept(&dst)
        .map(|i| i.uri().to_string())
}

async fn run(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: Option<Vec<u8>>,
    opts: &SendOptions,
) -> Result<Outcome, SendError> {
    let client = client(opts.verify_tls)?;
    let started = Instant::now();
    let mut method = Method::from_bytes(method.as_bytes()).map_err(other)?;
    let mut url = Url::parse(url).map_err(other)?;
    let mut body = body;
    let mut hops = Vec::new();
    let user_sets_encoding = headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("accept-encoding"));

    loop {
        let mut req = client.request(method.clone(), url.clone());
        if !user_sets_encoding {
            // Auto-decompression is off, so reqwest no longer advertises encodings itself.
            req = req.header(header::ACCEPT_ENCODING, "gzip, deflate, br, zstd");
        }
        for (k, v) in headers {
            req = req.header(k, v);
        }
        if let Some(b) = &body {
            req = req.body(b.clone());
        }
        let resp = req.send().await.map_err(classify)?;
        let status = resp.status();

        let location = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok());
        if let (true, true, Some(location)) = (opts.follow_redirects, status.is_redirection(), location) {
            if hops.len() as u32 >= opts.max_redirects {
                return Err(SendError::Other(format!("too many redirects (max {})", opts.max_redirects)));
            }
            let next = url.join(location).map_err(other)?;
            hops.push(Hop { status: status.as_u16(), url: url.to_string() });
            let to_get = match status.as_u16() {
                303 => method != Method::HEAD,
                301 | 302 => method == Method::POST,
                _ => false,
            };
            if to_get {
                method = Method::GET;
                body = None;
            }
            url = next;
            continue;
        }

        let ttfb = started.elapsed();
        let encoding = resp
            .headers()
            .get(header::CONTENT_ENCODING)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_ascii_lowercase());
        let raw = resp.bytes().await.map_err(classify)?;
        let total = started.elapsed();
        let wire_bytes = raw.len() as u64;
        let body = decode(encoding.as_deref(), &raw)?;
        return Ok(Outcome { status: status.as_u16(), hops, ttfb, total, wire_bytes, body });
    }
}

/// Clients are cached by the settings that need a separate client; today that is only `verify_tls`
/// (timeout is applied per request, redirects are always handled by `run`).
fn client(verify_tls: bool) -> Result<Client, SendError> {
    static PROVIDER: Once = Once::new();
    static CLIENTS: OnceLock<Mutex<HashMap<bool, Client>>> = OnceLock::new();

    // `rustls-no-provider`: reqwest uses whatever provider is the process default.
    PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });

    let mut clients = CLIENTS.get_or_init(Default::default).lock().unwrap();
    if let Some(c) = clients.get(&verify_tls) {
        return Ok(c.clone());
    }
    let c = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        // Auto-decompression strips Content-Encoding/Content-Length and hands back only the decoded
        // body, so the on-the-wire size is lost. Decode ourselves in `decode`.
        .gzip(false)
        .brotli(false)
        .deflate(false)
        .zstd(false)
        .tls_danger_accept_invalid_certs(!verify_tls)
        .build()
        .map_err(classify)?;
    clients.insert(verify_tls, c.clone());
    Ok(c)
}

fn decode(encoding: Option<&str>, raw: &[u8]) -> Result<Vec<u8>, SendError> {
    let mut out = Vec::new();
    let r = match encoding {
        Some("gzip") | Some("x-gzip") => flate2::read::MultiGzDecoder::new(raw).read_to_end(&mut out),
        // HTTP "deflate" is meant to be zlib-wrapped; some servers send raw deflate.
        Some("deflate") => flate2::read::ZlibDecoder::new(raw).read_to_end(&mut out).or_else(|_| {
            out.clear();
            flate2::read::DeflateDecoder::new(raw).read_to_end(&mut out)
        }),
        Some("br") => brotli::Decompressor::new(raw, 4096).read_to_end(&mut out),
        Some("zstd") => zstd::stream::read::Decoder::new(raw).and_then(|mut d| d.read_to_end(&mut out)),
        // identity, none, or something not handled in the spike (e.g. stacked encodings): as is.
        _ => return Ok(raw.to_vec()),
    };
    r.map_err(|e| SendError::Other(format!("decode {}: {e}", encoding.unwrap_or_default())))?;
    Ok(out)
}

fn classify(e: reqwest::Error) -> SendError {
    if e.is_timeout() {
        return SendError::Timeout;
    }
    // rustls errors reach us as io::Error(Other, io::Error(InvalidData, rustls::Error)), and
    // io::Error's `source()` skips the wrapped error itself, so unwrap io layers via `get_ref`.
    let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(&e);
    while let Some(err) = cur {
        let mut layer = err;
        loop {
            if let Some(t) = layer.downcast_ref::<rustls::Error>() {
                return SendError::Tls(t.to_string());
            }
            match layer.downcast_ref::<std::io::Error>().and_then(|io| io.get_ref()) {
                Some(inner) => layer = inner,
                None => break,
            }
        }
        cur = err.source();
    }
    let mut msg = e.to_string();
    let mut src = e.source();
    while let Some(s) = src {
        msg.push_str(": ");
        msg.push_str(&s.to_string());
        src = s.source();
    }
    SendError::Other(msg)
}

fn other(e: impl std::fmt::Display) -> SendError {
    SendError::Other(e.to_string())
}
