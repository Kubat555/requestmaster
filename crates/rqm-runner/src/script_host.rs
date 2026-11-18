//! `pm.sendRequest`: the host of the script engine over a channel. The thread of the scripts only asks; the
//! requests are sent by the asynchronous side of the send, through the same HTTP engine as the main request.

use std::io::Read;
use std::path::Path;

use rqm_http::{HttpBody, HttpEngine, HttpRequest, HttpResponse, MultipartPart, PartValue, SendOptions};
use rqm_script::{Host, Job, OutgoingBody, OutgoingRequest, Report, ResponseBody, ScriptResponse, BODY_LIMIT};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_util::sync::CancellationToken;

use crate::prepare::form_encode;

/// What the blocking side of a phase of scripts tells the asynchronous one.
enum HostCall {
    /// A script waits for the answer to its request.
    Request(OutgoingRequest, std::sync::mpsc::Sender<Result<ScriptResponse, String>>),
    /// The scripts of the phase are over.
    Done(Report),
}

/// The host the script thread owns: nothing but the sending end of the channel.
struct ChannelHost {
    calls: UnboundedSender<HostCall>,
}

impl Host for ChannelHost {
    fn send_request(&mut self, request: OutgoingRequest) -> Result<ScriptResponse, String> {
        let cancelled = || "cancelled".to_string();
        let (reply, answer) = std::sync::mpsc::channel();
        // Nobody listens any more: the phase is over and this is a thread the watchdog gave up on.
        self.calls.send(HostCall::Request(request, reply)).map_err(|_| cancelled())?;
        answer.recv().unwrap_or_else(|_| Err(cancelled()))
    }
}

/// Runs the scripts of `job` on a blocking thread and sends their requests through `engine` with `options`.
/// `None` — the blocking task died without a report.
pub(crate) async fn run_job(
    engine: &dyn HttpEngine,
    job: Job,
    options: &SendOptions,
    cancel: &CancellationToken,
) -> Option<Report> {
    let (calls, mut inbox) = unbounded_channel();
    let host = Box::new(ChannelHost { calls: calls.clone() });
    let token = cancel.clone();
    tokio::task::spawn_blocking(move || {
        let report = rqm_script::run_with_host(job, host, &token);
        let _ = calls.send(HostCall::Done(report));
    });
    // Until `Done`, not until the channel closes: the host lives in the script thread, and a thread the
    // watchdog gave up on never drops it.
    loop {
        match inbox.recv().await? {
            HostCall::Request(request, reply) => {
                let _ = reply.send(exchange(engine, request, options, cancel).await);
            }
            HostCall::Done(report) => return Some(report),
        }
    }
}

/// Sends a request of a script and reads the answer into memory; the body file of the engine is removed.
async fn exchange(
    engine: &dyn HttpEngine,
    request: OutgoingRequest,
    options: &SendOptions,
    cancel: &CancellationToken,
) -> Result<ScriptResponse, String> {
    let http = http_request(request, options.clone());
    let response = engine.send(http, cancel.clone()).await.map_err(|error| error.to_string())?;
    let path = response.body_path.clone();
    let body = tokio::task::spawn_blocking(move || read_and_remove(&path)).await;
    let body = body.map_err(|error| format!("response body reader failed: {error}"))?;
    let body = body.map_err(|error| format!("file error: {}: {error}", response.body_path.display()))?;
    Ok(script_response(&response, ResponseBody::Bytes(body)))
}

/// At most `BODY_LIMIT + 1` bytes of the file — enough for the engine to see a body over the limit. The file
/// is removed whether it was read or not.
fn read_and_remove(path: &Path) -> std::io::Result<Vec<u8>> {
    let read = std::fs::File::open(path).and_then(|file| {
        let mut bytes = Vec::new();
        file.take(BODY_LIMIT.saturating_add(1)).read_to_end(&mut bytes)?;
        Ok(bytes)
    });
    let _ = std::fs::remove_file(path);
    read
}

/// The response as scripts see it: `pm.response` of the main request and the answer of `pm.sendRequest`.
pub(crate) fn script_response(response: &HttpResponse, body: ResponseBody) -> ScriptResponse {
    ScriptResponse {
        code: response.status,
        status: response.status_text.clone(),
        headers: response.headers.clone(),
        time_ms: response.timings.total.as_secs_f64() * 1000.0,
        size: response.decoded_size,
        body,
    }
}

/// The request of a script as the engine takes it: nothing is substituted, no auth of the tree is added.
fn http_request(request: OutgoingRequest, options: SendOptions) -> HttpRequest {
    let bytes = |text: String, content_type: &str| HttpBody::Bytes {
        data: text.into_bytes(),
        default_content_type: Some(content_type.to_string()),
    };
    let body = match request.body {
        OutgoingBody::None => HttpBody::None,
        OutgoingBody::Raw(text) => bytes(text, "text/plain"),
        OutgoingBody::UrlEncoded(fields) => {
            let pairs: Vec<String> =
                fields.iter().map(|(key, value)| format!("{}={}", form_encode(key), form_encode(value))).collect();
            bytes(pairs.join("&"), "application/x-www-form-urlencoded")
        }
        OutgoingBody::FormData(fields) => {
            let part = |(name, value)| MultipartPart { name, value: PartValue::Text(value), content_type: None };
            HttpBody::Multipart(fields.into_iter().map(part).collect())
        }
    };
    // A `://` inside the query is not a scheme.
    let before_query = request.url.split('?').next().unwrap_or(&request.url);
    let url = match before_query.contains("://") {
        true => request.url,
        false => format!("http://{}", request.url),
    };
    HttpRequest { method: request.method, url, headers: request.headers, body, options }
}
