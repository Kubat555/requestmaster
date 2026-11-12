//! The HTTP engine: sends one request through `reqwest` and stores the decoded response body on disk.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Mutex, Once, PoisonError};
use std::time::{Duration, Instant};

use reqwest::header::{HeaderName, HeaderValue, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE};
use reqwest::{multipart, Body, Client, Method, Url};
use tokio_util::sync::CancellationToken;

use crate::decode::{self, Piece};
use crate::headers;
use crate::types::{
    HttpBody, HttpError, HttpRequest, HttpResponse, MultipartPart, PartValue, ProxySetting, SendOptions, SentRequest,
    Timings,
};

/// Something that can send an HTTP request. `rqm-runner` depends on this trait, tests replace the engine.
pub trait HttpEngine: Send + Sync {
    /// Sends `request` and waits for the whole response. `cancel` aborts both the wait for the headers
    /// and the reading of the body; no body file is left behind then.
    fn send<'a>(
        &'a self,
        request: HttpRequest,
        cancel: CancellationToken,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, HttpError>> + Send + 'a>>;
}

/// Settings of an engine, taken from the application.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Directory for response bodies: every response is stored there as `<uuid>.body`.
    pub body_dir: PathBuf,
    /// Value of the automatic `User-Agent` header, `RequestMaster/<version>`.
    pub user_agent: String,
}

/// [`HttpEngine`] on `reqwest` with `rustls`/`ring`.
///
/// `reqwest`'s own redirects and decompression are off: the engine does both itself to report the chain and
/// the on-the-wire size. Not `Clone`: it owns its client cache; share it behind an `Arc`.
#[derive(Debug)]
pub struct ReqwestEngine {
    config: EngineConfig,
    /// Clients by the settings that need a separate client: (`verify_tls`, `proxy`).
    clients: Mutex<HashMap<(bool, ProxySetting), Client>>,
}

impl ReqwestEngine {
    /// Creates an engine and its body directory. Fails with [`HttpError::Io`] when the directory
    /// cannot be created.
    pub fn new(config: EngineConfig) -> Result<Self, HttpError> {
        static PROVIDER: Once = Once::new();
        // `reqwest` is built with `rustls-no-provider` and uses the process default provider. The call fails
        // only when a provider is already installed, which is just as good.
        PROVIDER.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
        std::fs::create_dir_all(&config.body_dir)
            .map_err(|e| HttpError::Io(format!("{}: {e}", config.body_dir.display())))?;
        Ok(ReqwestEngine { config, clients: Mutex::new(HashMap::new()) })
    }

    /// The client for these options, built on first use.
    fn client(&self, options: &SendOptions) -> Result<Client, HttpError> {
        // A poisoned lock only means another thread panicked between two map operations; the map is intact.
        let mut clients = self.clients.lock().unwrap_or_else(PoisonError::into_inner);
        let key = (options.verify_tls, options.proxy.clone());
        if let Some(client) = clients.get(&key) {
            return Ok(client.clone());
        }
        let mut builder = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .tls_danger_accept_invalid_certs(!options.verify_tls);
        builder = match &options.proxy {
            // The `system-proxy` feature makes this the default behaviour of the client.
            ProxySetting::System => builder,
            ProxySetting::Off => builder.no_proxy(),
            ProxySetting::Manual(proxy) => builder
                .proxy(reqwest::Proxy::all(proxy.as_str()).map_err(|e| HttpError::InvalidUrl(format!("proxy: {}", describe(&e))))?),
        };
        let client = builder.build().map_err(classify)?;
        clients.insert(key, client.clone());
        Ok(client)
    }

    async fn run(&self, request: HttpRequest, cancel: CancellationToken) -> Result<HttpResponse, HttpError> {
        let HttpRequest { method, url, headers: user_headers, body, options } = request;
        let method = Method::from_bytes(method.as_bytes()).map_err(|_| HttpError::Other("invalid method".to_string()))?;
        let url = parse_url(&url)?;
        let client = self.client(&options)?;
        let (request, sent) =
            build_request(&client, method, url, user_headers, body, &options, &self.config.user_agent).await?;

        let deadline = deadline(options.timeout);
        tokio::pin!(deadline);
        let started = Instant::now();
        let mut response = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(HttpError::Cancelled),
            _ = &mut deadline => return Err(HttpError::Timeout),
            response = client.execute(request) => response.map_err(classify)?,
        };
        let ttfb = started.elapsed();

        let status = response.status();
        let http_version = format!("{:?}", response.version());
        let headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(name, value)| (name.to_string(), String::from_utf8_lossy(value.as_bytes()).into_owned()))
            .collect();
        let codings =
            decode::codings(response.headers().get_all(CONTENT_ENCODING).iter().filter_map(|value| value.to_str().ok()));

        // The body goes to a blocking thread that decodes and writes it; the bounded channel keeps memory flat.
        let body_path = self.config.body_dir.join(format!("{}.body", uuid::Uuid::new_v4()));
        let (pieces, receiver) = tokio::sync::mpsc::channel(16);
        let writer = tokio::task::spawn_blocking({
            let body_path = body_path.clone();
            move || decode::write_body(receiver, &body_path, &codings)
        });
        let received = {
            let receive = async {
                while let Some(chunk) = response.chunk().await.map_err(classify)? {
                    if pieces.send(Piece::Data(chunk)).await.is_err() {
                        // The writer stopped on a file error and reports it itself.
                        return Ok(());
                    }
                }
                let _ = pieces.send(Piece::End).await;
                Ok(())
            };
            tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(HttpError::Cancelled),
                _ = &mut deadline => Err(HttpError::Timeout),
                received = receive => received,
            }
        };
        // Closing the channel without `Piece::End` tells the writer to remove what it wrote. Wait for it
        // either way, so that no file is left when the error is returned.
        drop(pieces);
        let written = writer.await.map_err(|e| HttpError::Other(format!("response body writer failed: {e}")))?;
        received?;
        let written = written
            .map_err(|e| HttpError::Io(format!("{}: {e}", body_path.display())))?
            .ok_or_else(|| HttpError::Other("response body was not stored".to_string()))?;
        let total = started.elapsed();

        Ok(HttpResponse {
            status: status.as_u16(),
            status_text: status.canonical_reason().unwrap_or_default().to_string(),
            http_version,
            headers,
            body_path,
            wire_size: written.wire_size,
            decoded_size: written.decoded_size,
            decode_error: written.decode_error,
            timings: Timings { ttfb, total },
            redirects: Vec::new(),
            sent,
        })
    }
}

impl HttpEngine for ReqwestEngine {
    fn send<'a>(
        &'a self,
        request: HttpRequest,
        cancel: CancellationToken,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, HttpError>> + Send + 'a>> {
        Box::pin(self.run(request, cancel))
    }
}

/// Completes when the timeout runs out; never, when there is none.
async fn deadline(timeout: Option<Duration>) {
    match timeout {
        Some(timeout) => tokio::time::sleep(timeout).await,
        None => std::future::pending().await,
    }
}

/// An absolute `http`/`https` URL, or [`HttpError::InvalidUrl`]. `host:8080/x` parses as scheme `host`,
/// so the scheme is checked explicitly.
fn parse_url(url: &str) -> Result<Url, HttpError> {
    let parsed = Url::parse(url).map_err(|e| HttpError::InvalidUrl(e.to_string()))?;
    match parsed.scheme() {
        "http" | "https" if parsed.has_host() => Ok(parsed),
        "http" | "https" => Err(HttpError::InvalidUrl("no host".to_string())),
        scheme => Err(HttpError::InvalidUrl(format!("unsupported scheme `{scheme}`"))),
    }
}

/// Builds the `reqwest` request and the record of what is sent. Body files are opened here, before any
/// connection is made.
async fn build_request(
    client: &Client,
    method: Method,
    url: Url,
    user_headers: Vec<(String, String)>,
    body: HttpBody,
    options: &SendOptions,
    user_agent: &str,
) -> Result<(reqwest::Request, SentRequest), HttpError> {
    let (body, content_type, content_length) = match body {
        HttpBody::None => (None, None, None),
        HttpBody::Bytes { data, default_content_type } => {
            let length = data.len() as u64;
            (Some(Body::from(data)), default_content_type, Some(length))
        }
        HttpBody::File { path, default_content_type } => {
            let (file, length) = open(&path).await?;
            (Some(Body::from(file)), default_content_type, Some(length))
        }
        HttpBody::Multipart(parts) => {
            // `reqwest` picks the boundary and computes the length; take both from the request it builds.
            let mut built = client
                .request(method.clone(), url.clone())
                .multipart(multipart_form(parts).await?)
                .build()
                .map_err(classify)?;
            let content_type = header_text(&built, &CONTENT_TYPE);
            let content_length = header_text(&built, &CONTENT_LENGTH).and_then(|length| length.parse().ok());
            (built.body_mut().take(), content_type, content_length)
        }
    };

    let mut sent_headers = headers::compose(
        &user_headers,
        &options.disabled_auto_headers,
        user_agent,
        headers::host_header(&url),
        content_type,
        content_length,
    );
    sent_headers.extend(user_headers);

    let sent = SentRequest { method: method.to_string(), url: url.to_string(), headers: sent_headers };
    let mut request = reqwest::Request::new(method, url);
    for (name, value) in &sent.headers {
        let header_name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| HttpError::Other(format!("invalid header name `{name}`")))?;
        // `from_bytes` rather than `from_str`: a value in UTF-8 is sent as typed.
        let header_value = HeaderValue::from_bytes(value.as_bytes())
            .map_err(|_| HttpError::Other(format!("invalid value of header `{name}`")))?;
        request.headers_mut().append(header_name, header_value);
    }
    *request.body_mut() = body;
    Ok((request, sent))
}

/// Opens a body file and reads its size.
async fn open(path: &Path) -> Result<(tokio::fs::File, u64), HttpError> {
    let io_error = |e: std::io::Error| HttpError::Io(format!("{}: {e}", path.display()));
    let file = tokio::fs::File::open(path).await.map_err(io_error)?;
    let length = file.metadata().await.map_err(io_error)?.len();
    Ok((file, length))
}

async fn multipart_form(parts: Vec<MultipartPart>) -> Result<multipart::Form, HttpError> {
    let mut form = multipart::Form::new();
    for MultipartPart { name, value, content_type } in parts {
        let mut part = match value {
            PartValue::Text(text) => multipart::Part::text(text),
            PartValue::File(path) => {
                let (file, length) = open(&path).await?;
                let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                multipart::Part::stream_with_length(file, length).file_name(file_name)
            }
        };
        if let Some(content_type) = content_type {
            part = part
                .mime_str(&content_type)
                .map_err(|_| HttpError::Other(format!("invalid content type `{content_type}` of part `{name}`")))?;
        }
        form = form.part(name, part);
    }
    Ok(form)
}

fn header_text(request: &reqwest::Request, name: &HeaderName) -> Option<String> {
    request.headers().get(name).and_then(|value| value.to_str().ok()).map(str::to_string)
}

/// Sorts a `reqwest` failure into the engine's error kinds.
fn classify(error: reqwest::Error) -> HttpError {
    // The caller knows the URL; keeping it out of the text keeps query-string secrets out of messages.
    let error = error.without_url();
    if error.is_timeout() {
        return HttpError::Timeout;
    }
    // A rustls error arrives as io::Error(Other, io::Error(InvalidData, rustls::Error)), and `source()` of an
    // io::Error skips the error it wraps, so io layers are unwrapped through `get_ref`.
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(&error);
    while let Some(outer) = current {
        let mut layer = outer;
        loop {
            if let Some(tls) = layer.downcast_ref::<rustls::Error>() {
                return HttpError::Tls(tls.to_string());
            }
            match layer.downcast_ref::<std::io::Error>().and_then(|io| io.get_ref()) {
                Some(inner) => layer = inner,
                None => break,
            }
        }
        current = outer.source();
    }
    if error.is_connect() {
        HttpError::Connect(describe(&error))
    } else {
        HttpError::Other(describe(&error))
    }
}

/// The error with its chain of causes in one line.
fn describe(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}
