mod common;
use common::*;
use rqm_http::*;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

#[tokio::test] async fn cookies_are_stored_sent_listed_persisted_and_cleared() {
    let s = MockServer::start().await;
    Mock::given(path("/login")).respond_with(ResponseTemplate::new(200).append_header("set-cookie", "sid=abc; Path=/").append_header("set-cookie", "old=1; Max-Age=0")).mount(&s).await;
    Mock::given(path("/me")).respond_with(ResponseTemplate::new(200)).mount(&s).await;
    let jar = Arc::new(CookieJar::new());
    let (e, _d) = engine_with_jar(jar.clone());
    let first = e.send(get(&format!("{}/login", s.uri())), CancellationToken::new()).await.unwrap();
    let set: Vec<_> = response_cookies(&first).into_iter().map(|c| (c.name, c.path, c.domain)).collect();
    assert_eq!(set, vec![("sid".to_string(), "/".to_string(), "127.0.0.1".to_string()), ("old".to_string(), "/".to_string(), "127.0.0.1".to_string())]);
    assert_eq!(jar.list().iter().map(|c| (c.name.as_str(), c.value.as_str())).collect::<Vec<_>>(), vec![("sid", "abc")]);
    e.send(get(&format!("{}/me", s.uri())), CancellationToken::new()).await.unwrap();
    assert_eq!(s.received_requests().await.unwrap()[1].headers.get("cookie").unwrap(), "sid=abc");

    let restored = Arc::new(CookieJar::from_json(&jar.to_json()).unwrap());   // session cookie survives a restart
    let (e2, _d2) = engine_with_jar(restored.clone());
    let mut req = get(&format!("{}/me", s.uri())); req.headers = vec![("Cookie".into(), "mine=1".into())];
    e2.send(req, CancellationToken::new()).await.unwrap();
    assert_eq!(s.received_requests().await.unwrap()[2].headers.get("cookie").unwrap(), "mine=1; sid=abc");

    restored.clear();
    assert!(restored.list().is_empty());
    assert!(CookieJar::from_json("garbage").is_err());
}
#[tokio::test] async fn cookie_set_on_a_redirect_is_sent_to_the_next_hop() {
    let s = MockServer::start().await;
    Mock::given(path("/start")).respond_with(ResponseTemplate::new(302).insert_header("location", "/end").insert_header("set-cookie", "hop=1; Path=/")).mount(&s).await;
    Mock::given(path("/end")).respond_with(ResponseTemplate::new(200)).mount(&s).await;
    let (e, _d) = engine();
    e.send(get(&format!("{}/start", s.uri())), CancellationToken::new()).await.unwrap();
    assert_eq!(s.received_requests().await.unwrap()[1].headers.get("cookie").unwrap(), "hop=1");
}
#[tokio::test] async fn cookies_do_not_leak_to_another_host_or_path() {
    let a = MockServer::start().await;
    Mock::given(path("/set")).respond_with(ResponseTemplate::new(200).insert_header("set-cookie", "p=1; Path=/only")).mount(&a).await;
    Mock::given(any()).respond_with(ResponseTemplate::new(200)).mount(&a).await;
    let (e, _d) = engine();
    e.send(get(&format!("{}/set", a.uri())), CancellationToken::new()).await.unwrap();
    e.send(get(&format!("{}/other", a.uri())), CancellationToken::new()).await.unwrap();
    e.send(get(&format!("{}/only/x", a.uri())), CancellationToken::new()).await.unwrap();
    let got = a.received_requests().await.unwrap();
    assert!(got[1].headers.get("cookie").is_none());
    assert_eq!(got[2].headers.get("cookie").unwrap(), "p=1");
}
