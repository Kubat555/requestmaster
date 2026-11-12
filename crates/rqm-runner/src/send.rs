//! Sending: [`prepare`] with the system dynamic values, then the HTTP engine.

use rqm_http::{HttpEngine, HttpError, HttpResponse};
use tokio_util::sync::CancellationToken;

use crate::prepare::{prepare, PrepareError, Prepared, SendWarning};
use crate::SendInput;

/// Why a send produced no response.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SendError {
    /// The request could not be built; nothing was sent.
    #[error(transparent)]
    Prepare(#[from] PrepareError),
    /// The request was built but the exchange failed.
    #[error(transparent)]
    Http(#[from] HttpError),
}

/// Result of [`send`].
#[derive(Debug, Clone)]
pub struct SendOutcome {
    /// The response, or why there is none.
    pub result: Result<HttpResponse, SendError>,
    /// Warnings of [`prepare`]; present on a network error too, empty when `prepare` itself failed.
    pub warnings: Vec<SendWarning>,
}

/// Builds the HTTP request from `input` (dynamic values from the system clock and random generator) and
/// sends it with `engine`. `cancel` aborts the exchange.
pub async fn send(engine: &dyn HttpEngine, input: &SendInput, cancel: CancellationToken) -> SendOutcome {
    let prepared = prepare(input, &mut |name| rqm_vars::system_dynamic(name));
    let Prepared { http, warnings } = match prepared {
        Ok(prepared) => prepared,
        Err(error) => return SendOutcome { result: Err(error.into()), warnings: Vec::new() },
    };
    let result = engine.send(http, cancel).await.map_err(SendError::from);
    SendOutcome { result, warnings }
}
