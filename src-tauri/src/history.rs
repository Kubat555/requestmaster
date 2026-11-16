//! The history of sends of [`AppCore`]: writing an entry after every send, the list, one entry, clearing.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use rqm_runner::{SendInput, SendOutcome};
use rqm_store::{HistoryRow, Store, StoreError};

use crate::core::{message, AppCore};
use crate::dto::{HistoryDetail, HistoryEntry, HistoryItem, HistorySummary};
use crate::send::{error_dto, milliseconds, warning_dto};

/// The most of a request or response body an entry keeps: 1 MiB.
const BODY_LIMIT: usize = 1_048_576;

/// The most entries one `history_list` call returns.
const LIST_LIMIT: usize = 200;

impl AppCore {
    /// History entries, newest first: at most `limit` of them (kept within 1..=200), and with `before_id`
    /// only the ones older than that entry.
    pub fn history_list(&self, limit: usize, before_id: Option<i64>) -> Result<Vec<HistoryItem>, String> {
        let rows = self.store()?.history_list(limit.clamp(1, LIST_LIMIT), before_id).map_err(message)?;
        rows.into_iter().map(history_item).collect()
    }

    /// The whole entry; `node_exists` tells whether the request it was sent from is still in the tree.
    pub fn history_get(&self, id: i64) -> Result<HistoryEntry, String> {
        let store = self.store()?;
        let detail = store.history_detail(id).map_err(message)?;
        let detail: HistoryDetail = serde_json::from_str(&detail).map_err(|error| unreadable(id, error))?;
        // The store reads a summary only as part of the list: the newest entry below `id + 1` is the entry itself.
        let rows = store.history_list(1, id.checked_add(1)).map_err(message)?;
        let row = rows.into_iter().find(|row| row.id == id).ok_or_else(|| message(StoreError::NotFound(id.to_string())))?;
        let item = history_item(row)?;
        let node_exists = match &item.summary.node_id {
            Some(node_id) => node_exists(&store, node_id)?,
            None => false,
        };
        Ok(HistoryEntry { item, detail, node_exists })
    }

    /// Deletes every history entry.
    pub fn history_clear(&self) -> Result<(), String> {
        self.store()?.history_clear().map_err(message)
    }

    /// Writes the entry of one send, whatever its outcome was. Must be called while the file of the response
    /// body is still in place. A failure is ignored: the history must never cost the user the response.
    pub(crate) fn record_history(&self, request_id: &str, input: &SendInput, sent_at: &str, outcome: &SendOutcome) {
        let (summary, detail) = history_record(request_id, input, outcome);
        let (Ok(summary), Ok(detail)) = (serde_json::to_string(&summary), serde_json::to_string(&detail)) else {
            return;
        };
        if let Ok(mut store) = self.store() {
            let _ = store.history_add(sent_at, &summary, &detail, self.history_keep);
        }
    }
}

/// What the history keeps of one send.
///
/// The entry shows the request the user sent. With a response that came without redirects, the method, the
/// address and the headers are the ones that went to the network, automatic headers included. After redirects
/// `response.sent` is the last request of the chain — another method and address, and not the body that was
/// sent — so the method, the address and the headers are the prepared ones, like the body. Without a response
/// but with a prepared request (network error, timeout, cancellation) they are the prepared ones as well:
/// variables substituted. When the request could not be prepared, the method and the address are the ones
/// typed and there are no headers and no body.
fn history_record(request_id: &str, input: &SendInput, outcome: &SendOutcome) -> (HistorySummary, HistoryDetail) {
    let prepared = outcome.request.as_ref();
    let (method, url, request_headers) = match (&outcome.result, prepared) {
        (Ok(response), Some(request)) if !response.redirects.is_empty() => {
            (&request.method, &request.url, request.headers.clone())
        }
        (Ok(response), _) => (&response.sent.method, &response.sent.url, response.sent.headers.clone()),
        (Err(_), Some(request)) => (&request.method, &request.url, request.headers.clone()),
        (Err(_), None) => (&input.request.method, &input.request.url, Vec::new()),
    };
    let (request_body, request_body_truncated) = match prepared.and_then(|request| request.body.as_deref()) {
        Some(body) => (Some(text_prefix(body).to_string()), body.len() > BODY_LIMIT),
        None => (None, false),
    };
    let mut summary = HistorySummary {
        node_id: Some(request_id.to_string()),
        name: input.request_name.clone(),
        method: method.clone(),
        url: url.clone(),
        status: None,
        error_kind: None,
        total_ms: None,
        size: None,
    };
    let mut detail = HistoryDetail {
        request_headers,
        request_body,
        request_body_truncated,
        response_headers: Vec::new(),
        response_body: None,
        response_body_truncated: false,
        response_body_binary: false,
        error_message: None,
        warnings: outcome.warnings.iter().cloned().map(warning_dto).collect(),
    };
    match &outcome.result {
        Ok(response) => {
            summary.status = Some(response.status);
            summary.total_ms = Some(milliseconds(response.timings.total));
            summary.size = Some(response.decoded_size);
            detail.response_headers = response.headers.clone();
            // A body file that cannot be read leaves the entry without a body; the rest is still worth keeping.
            if let Ok(beginning) = read_beginning(&response.body_path) {
                detail.response_body = text_of_beginning(beginning);
                detail.response_body_binary = detail.response_body.is_none();
            }
            detail.response_body_truncated = response.decoded_size > BODY_LIMIT as u64;
        }
        Err(error) => {
            let error = error_dto(error.clone());
            summary.error_kind = Some(error.kind);
            detail.error_message = Some(error.message);
        }
    }
    (summary, detail)
}

/// The first [`BODY_LIMIT`] bytes of the file, or all of it when it is shorter.
fn read_beginning(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(BODY_LIMIT as u64).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// The beginning of a body as text; `None` when it is not UTF-8 text. An incomplete character at the very
/// end is not a sign of that: it is the cut at [`BODY_LIMIT`] falling inside a character, and it is dropped.
/// An empty body is an empty text.
fn text_of_beginning(bytes: Vec<u8>) -> Option<String> {
    match String::from_utf8(bytes) {
        Ok(text) => Some(text),
        Err(error) => {
            let utf8_error = error.utf8_error();
            if utf8_error.error_len().is_some() {
                return None;
            }
            let mut bytes = error.into_bytes();
            bytes.truncate(utf8_error.valid_up_to());
            String::from_utf8(bytes).ok()
        }
    }
}

/// At most [`BODY_LIMIT`] bytes of the text from its start, ending on a character boundary.
fn text_prefix(text: &str) -> &str {
    let mut end = text.len().min(BODY_LIMIT);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// A row of the store as the list shows it.
fn history_item(row: HistoryRow) -> Result<HistoryItem, String> {
    let summary = serde_json::from_str(&row.summary).map_err(|error| unreadable(row.id, error))?;
    Ok(HistoryItem { id: row.id, sent_at: row.sent_at, summary })
}

/// Whether `store` has a node with this id.
fn node_exists(store: &Store, id: &str) -> Result<bool, String> {
    match store.node(id) {
        Ok(_) => Ok(true),
        Err(StoreError::NotFound(_)) => Ok(false),
        Err(error) => Err(message(error)),
    }
}

/// The error of an entry whose stored JSON does not read back.
fn unreadable(id: i64, error: serde_json::Error) -> String {
    format!("history entry {id} cannot be read: {error}")
}
