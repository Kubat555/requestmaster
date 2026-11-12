mod common;
use common::*;
use rqm_http::*;
use tokio_util::sync::CancellationToken;
use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

async fn redirecting(status: u16, to: &str) -> MockServer {
    let s = MockServer::start().await;
    Mock::given(path("/start")).respond_with(ResponseTemplate::new(status).insert_header("location", to)).mount(&s).await;
    Mock::given(path("/end")).respond_with(ResponseTemplate::new(200).set_body_string("done")).mount(&s).await;
    s
}
fn post(url: &str) -> HttpRequest {
    let mut r = get(url); r.method = "POST".into();
    r.headers = vec![("Authorization".into(), "Bearer secret".into()), ("Cookie".into(), "a=1".into()), ("X-Keep".into(), "1".into())];
    r.body = HttpBody::Bytes { data: b"payload".to_vec(), default_content_type: Some("text/plain".into()) };
    r
}

#[tokio::test] async fn chain_is_followed_and_reported() {
    let s = redirecting(302, "/end").await;
    let (e, _d) = engine();
    let r = e.send(get(&format!("{}/start", s.uri())), CancellationToken::new()).await.unwrap();
    assert_eq!((r.status, body(&r)), (200, b"done".to_vec()));
    assert_eq!(r.redirects.len(), 1);
    assert_eq!((r.redirects[0].status, r.redirects[0].location.as_str()), (302, "/end"));
    assert!(r.sent.url.ends_with("/end"));
}
#[tokio::test] async fn method_and_body_rules_per_status() {
    for (status, method_after, body_after) in [(301, "GET", ""), (302, "GET", ""), (303, "GET", ""), (307, "POST", "payload"), (308, "POST", "payload")] {
        let s = redirecting(status, "/end").await;
        let (e, _d) = engine();
        e.send(post(&format!("{}/start", s.uri())), CancellationToken::new()).await.unwrap();
        let got = s.received_requests().await.unwrap();
        let last = got.iter().find(|r| r.url.path() == "/end").unwrap();
        assert_eq!(last.method.as_str(), method_after, "{status}");
        assert_eq!(String::from_utf8_lossy(&last.body), body_after, "{status}");
        assert_eq!(last.headers.get("content-type").is_some(), !body_after.is_empty(), "{status}: body headers");
        assert_eq!(last.headers.get("x-keep").unwrap(), "1", "{status}");
        assert_eq!(last.headers.get("authorization").unwrap(), "Bearer secret", "{status}: same origin keeps auth");
    }
}
#[tokio::test] async fn head_stays_head_on_303() {
    let s = redirecting(303, "/end").await;
    let (e, _d) = engine();
    let mut req = get(&format!("{}/start", s.uri())); req.method = "HEAD".into();
    e.send(req, CancellationToken::new()).await.unwrap();
    assert_eq!(s.received_requests().await.unwrap().last().unwrap().method.as_str(), "HEAD");
}
#[tokio::test] async fn cross_origin_strips_credentials() {
    let other = MockServer::start().await;
    Mock::given(path("/end")).respond_with(ResponseTemplate::new(200)).mount(&other).await;
    let s = redirecting(307, &format!("{}/end", other.uri())).await;   // another port = another origin
    let (e, _d) = engine();
    e.send(post(&format!("{}/start", s.uri())), CancellationToken::new()).await.unwrap();
    let got = &other.received_requests().await.unwrap()[0];
    assert!(got.headers.get("authorization").is_none());
    assert!(got.headers.get("cookie").is_none());
    assert_eq!(got.headers.get("x-keep").unwrap(), "1");
    assert_eq!(got.body, b"payload");
}
#[tokio::test] async fn only_real_redirect_statuses_are_followed() {
    for status in [300, 304, 305, 306] {
        let s = redirecting(status, "/end").await;
        let (e, _d) = engine();
        let r = e.send(get(&format!("{}/start", s.uri())), CancellationToken::new()).await.unwrap();
        assert_eq!((r.status, r.redirects.len()), (status, 0));
    }
    let s = MockServer::start().await;
    Mock::given(path("/start")).respond_with(ResponseTemplate::new(302)).mount(&s).await;   // no Location
    let (e, _d) = engine();
    assert_eq!(e.send(get(&format!("{}/start", s.uri())), CancellationToken::new()).await.unwrap().status, 302);
    let s = redirecting(302, "ftp://example.test/x").await;
    let (e, _d) = engine();
    assert_eq!(e.send(get(&format!("{}/start", s.uri())), CancellationToken::new()).await.unwrap().status, 302);
}
#[tokio::test] async fn follow_off_and_limit() {
    let s = redirecting(302, "/start").await;                       // endless loop
    let (e, _d) = engine();
    let mut req = get(&format!("{}/start", s.uri())); req.options.follow_redirects = false;
    assert_eq!(e.send(req, CancellationToken::new()).await.unwrap().status, 302);
    let mut req = get(&format!("{}/start", s.uri())); req.options.max_redirects = 3;
    assert!(matches!(e.send(req, CancellationToken::new()).await, Err(HttpError::TooManyRedirects(h)) if h.len() == 3));
    let mut req = get(&format!("{}/start", s.uri())); req.options.max_redirects = 0;
    assert!(matches!(e.send(req, CancellationToken::new()).await, Err(HttpError::TooManyRedirects(_))));
}
#[tokio::test] async fn relative_location_is_resolved() {
    let s = MockServer::start().await;
    Mock::given(path("/a/b/start")).respond_with(ResponseTemplate::new(302).insert_header("location", "../end?x=1")).mount(&s).await;
    Mock::given(path("/a/end")).and(query_param("x", "1")).respond_with(ResponseTemplate::new(200)).mount(&s).await;
    let (e, _d) = engine();
    assert_eq!(e.send(get(&format!("{}/a/b/start", s.uri())), CancellationToken::new()).await.unwrap().status, 200);
}
