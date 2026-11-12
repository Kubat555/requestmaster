mod common;
use common::*;
use rqm_http::*;
use tokio_util::sync::CancellationToken;

#[tokio::test] async fn self_signed_certificate_needs_verify_off() {
    let addr = https_server().await;
    let (e, _d) = engine();
    let url = format!("https://127.0.0.1:{}/", addr.port());
    assert!(matches!(e.send(get(&url), CancellationToken::new()).await, Err(HttpError::Tls(_))));
    let mut req = get(&url); req.options.verify_tls = false;
    assert_eq!(e.send(req, CancellationToken::new()).await.unwrap().status, 200);
    // the strict client is still strict after a relaxed one was used
    assert!(matches!(e.send(get(&url), CancellationToken::new()).await, Err(HttpError::Tls(_))));
}
#[tokio::test] async fn manual_proxy_receives_the_request() {
    let proxy = wiremock::MockServer::start().await;                 // acts as a plain-HTTP proxy
    wiremock::Mock::given(wiremock::matchers::any()).respond_with(wiremock::ResponseTemplate::new(299)).mount(&proxy).await;
    let (e, _d) = engine();
    let mut req = get("http://target.invalid/x"); req.options.proxy = ProxySetting::Manual(proxy.uri());
    assert_eq!(e.send(req, CancellationToken::new()).await.unwrap().status, 299);
    let mut req = get("http://target.invalid/x"); req.options.proxy = ProxySetting::Manual("socks9://nope".into());
    assert!(matches!(e.send(req, CancellationToken::new()).await, Err(HttpError::InvalidUrl(_))));
}
#[test] fn proxy_resolution_rules() {
    assert_eq!(resolve_proxy(&ProxySetting::Off, Some("http://sys:1".into())).unwrap(), None);
    assert_eq!(resolve_proxy(&ProxySetting::System, Some("http://sys:1".into())).unwrap(), Some("http://sys:1".into()));
    assert_eq!(resolve_proxy(&ProxySetting::System, None).unwrap(), None);
    assert_eq!(resolve_proxy(&ProxySetting::Manual("http://m:2".into()), Some("http://sys:1".into())).unwrap(), Some("http://m:2".into()));
    assert!(resolve_proxy(&ProxySetting::Manual("ftp://m".into()), None).is_err());
}
