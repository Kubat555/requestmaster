//! RequestMaster HTTP engine: sends one request and stores the decoded response body on disk.
//!
//! The crate knows nothing about collections or variables: `rqm-runner` builds an [`HttpRequest`] and
//! calls an [`HttpEngine`].

mod cookies;
mod decode;
mod engine;
mod headers;
mod proxy;
mod redirect;
mod types;

pub use cookies::{response_cookies, Cookie, CookieJar};
pub use engine::{EngineConfig, HttpEngine, ReqwestEngine};
pub use headers::auto_headers;
pub use proxy::resolve_proxy;
pub use types::{
    HttpBody, HttpError, HttpRequest, HttpResponse, MultipartPart, PartValue, ProxySetting, RedirectHop, SendOptions,
    SentRequest, Timings,
};
