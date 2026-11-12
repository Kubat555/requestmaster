mod common;
use common::*;
use rqm_http::*;
use tokio_util::sync::CancellationToken;
use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

#[tokio::test] async fn get_returns_status_headers_body_and_sizes() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/a")).respond_with(ResponseTemplate::new(201).insert_header("x-test", "1").set_body_string("привет")).mount(&server).await;
    let (e, _dir) = engine();
    let r = e.send(get(&format!("{}/a", server.uri())), CancellationToken::new()).await.unwrap();
    assert_eq!(r.status, 201);
    assert!(r.headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("x-test") && v == "1"));
    assert_eq!(body(&r), "привет".as_bytes());
    assert_eq!((r.wire_size, r.decoded_size), (12, 12));
    assert!(r.redirects.is_empty() && r.decode_error.is_none());
    assert!(r.timings.ttfb <= r.timings.total);
}
#[tokio::test] async fn automatic_headers_are_sent_and_reported() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(header("user-agent", "RequestMaster/test")).and(header("accept", "*/*"))
        .and(headers("accept-encoding", vec!["gzip", "deflate", "br", "zstd"])).and(header("content-type", "application/json"))
        .and(header("content-length", "2")).and(body_string("{}")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let (e, _dir) = engine();
    let mut req = get(&server.uri()); req.method = "POST".into();
    req.body = HttpBody::Bytes { data: b"{}".to_vec(), default_content_type: Some("application/json".into()) };
    let names: Vec<_> = auto_headers(&req, "RequestMaster/test").into_iter().map(|(k, _)| k).collect();
    assert_eq!(names, vec!["Host", "User-Agent", "Accept", "Accept-Encoding", "Content-Type", "Content-Length"]);
    let r = e.send(req, CancellationToken::new()).await.unwrap();
    assert_eq!(r.status, 200);
    assert!(r.sent.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("user-agent")));
}
#[tokio::test] async fn user_header_overrides_and_disabled_auto_header_is_not_sent() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let (e, _dir) = engine();
    let mut req = get(&server.uri());
    req.headers = vec![("User-Agent".into(), "mine".into()), ("X-Dup".into(), "1".into()), ("X-Dup".into(), "2".into())];
    req.options.disabled_auto_headers = vec!["accept".into(), "accept-encoding".into(), "host".into()];
    let names: Vec<_> = auto_headers(&req, "RequestMaster/test").into_iter().map(|(k, _)| k).collect();
    assert_eq!(names, vec!["Host", "Accept"]);          // these two cannot be disabled; no body = no Content-*
    e.send(req, CancellationToken::new()).await.unwrap();
    let got = &server.received_requests().await.unwrap()[0];
    assert_eq!(got.headers.get("user-agent").unwrap(), "mine");
    assert_eq!(got.headers.get("accept").unwrap(), "*/*");
    assert!(got.headers.get("accept-encoding").is_none());
    assert_eq!(got.headers.get_all("x-dup").iter().count(), 2);
}
#[tokio::test] async fn every_encoding_is_decoded_and_wire_size_is_reported() {
    let plain = "x".repeat(50_000).into_bytes();
    for (name, packed) in [("gzip", gzip(&plain)), ("deflate", deflate(&plain)), ("br", brotli(&plain)), ("zstd", zstd(&plain))] {
        let server = MockServer::start().await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).insert_header("content-encoding", name).set_body_bytes(packed.clone())).mount(&server).await;
        let (e, _dir) = engine();
        let r = e.send(get(&server.uri()), CancellationToken::new()).await.unwrap();
        assert_eq!(body(&r), plain, "{name}");
        assert_eq!((r.wire_size, r.decoded_size), (packed.len() as u64, 50_000), "{name}");
        assert!(r.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("content-encoding")), "{name}: header is kept");
    }
}
#[tokio::test] async fn empty_encoded_bodies_are_not_an_error() {
    for (m, status) in [("HEAD", 200), ("GET", 204), ("GET", 304)] {
        let server = MockServer::start().await;
        Mock::given(method(m)).respond_with(ResponseTemplate::new(status).insert_header("content-encoding", "gzip")).mount(&server).await;
        let (e, _dir) = engine();
        let mut req = get(&server.uri()); req.method = m.into();
        let r = e.send(req, CancellationToken::new()).await.unwrap();
        assert_eq!((r.status, r.decoded_size, r.decode_error.is_none()), (status, 0, true), "{m} {status}");
    }
}
#[tokio::test] async fn broken_and_unknown_encodings_keep_the_raw_body() {
    for (enc, expect_error) in [("gzip", true), ("made-up", false)] {
        let server = MockServer::start().await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).insert_header("content-encoding", enc).set_body_string("not compressed")).mount(&server).await;
        let (e, _dir) = engine();
        let r = e.send(get(&server.uri()), CancellationToken::new()).await.unwrap();
        assert_eq!(body(&r), b"not compressed");
        assert_eq!(r.decode_error.is_some(), expect_error, "{enc}");
    }
}
#[tokio::test] async fn large_body_goes_to_disk() {
    let plain = vec![b'a'; 30 * 1024 * 1024];
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).insert_header("content-encoding", "gzip").set_body_bytes(gzip(&plain))).mount(&server).await;
    let (e, dir) = engine();
    let r = e.send(get(&server.uri()), CancellationToken::new()).await.unwrap();
    assert_eq!(r.decoded_size, 30 * 1024 * 1024);
    assert_eq!(std::fs::metadata(&r.body_path).unwrap().len(), 30 * 1024 * 1024);
    assert!(r.body_path.starts_with(dir.path()));
}
#[tokio::test] async fn ttfb_measures_headers_not_body() {
    // raw TCP server: headers at once, body after 1500 ms
    let addr = slow_body_server(std::time::Duration::from_millis(1500)).await;
    let (e, _dir) = engine();
    let r = e.send(get(&format!("http://{addr}/")), CancellationToken::new()).await.unwrap();
    assert!(r.timings.ttfb < std::time::Duration::from_millis(1000), "ttfb {:?}", r.timings.ttfb);
    assert!(r.timings.total >= std::time::Duration::from_millis(1400), "total {:?}", r.timings.total);
}
#[tokio::test] async fn cancel_and_timeout() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(5))).mount(&server).await;
    let (e, dir) = engine();
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move { tokio::time::sleep(std::time::Duration::from_millis(100)).await; c2.cancel(); });
    let started = std::time::Instant::now();
    assert!(matches!(e.send(get(&server.uri()), cancel).await, Err(HttpError::Cancelled)));
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    let mut req = get(&server.uri()); req.options.timeout = Some(std::time::Duration::from_millis(150));
    assert!(matches!(e.send(req, CancellationToken::new()).await, Err(HttpError::Timeout)));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0, "no body files are left behind");
}
#[tokio::test] async fn cancel_during_body_removes_the_partial_file() {
    let addr = slow_body_server(std::time::Duration::from_secs(5)).await;
    let (e, dir) = engine();
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move { tokio::time::sleep(std::time::Duration::from_millis(300)).await; c2.cancel(); });
    assert!(matches!(e.send(get(&format!("http://{addr}/")), cancel).await, Err(HttpError::Cancelled)));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
#[tokio::test] async fn multipart_and_file_bodies() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let (e, dir) = engine();
    let file = dir.path().join("up.bin");
    std::fs::write(&file, b"FILEDATA").unwrap();
    let mut req = get(&server.uri()); req.method = "POST".into();
    req.body = HttpBody::Multipart(vec![
        MultipartPart { name: "text".into(), value: PartValue::Text("значение".into()), content_type: None },
        MultipartPart { name: "f".into(), value: PartValue::File(file.clone()), content_type: Some("application/x-test".into()) }]);
    e.send(req, CancellationToken::new()).await.unwrap();
    let mut req = get(&server.uri()); req.method = "POST".into();
    req.body = HttpBody::File { path: file, default_content_type: Some("application/octet-stream".into()) };
    e.send(req, CancellationToken::new()).await.unwrap();
    let got = server.received_requests().await.unwrap();
    assert!(got[0].headers.get("content-type").unwrap().to_str().unwrap().starts_with("multipart/form-data; boundary="));
    let multipart = String::from_utf8_lossy(&got[0].body);
    assert!(multipart.contains("name=\"text\"") && multipart.contains("значение"));
    assert!(multipart.contains("filename=\"up.bin\"") && multipart.contains("application/x-test") && multipart.contains("FILEDATA"));
    assert_eq!(got[1].body, b"FILEDATA");
    assert_eq!(got[1].headers.get("content-type").unwrap(), "application/octet-stream");
}
#[tokio::test] async fn errors_are_classified() {
    let (e, _dir) = engine();
    assert!(matches!(e.send(get("no-scheme.test/x"), CancellationToken::new()).await, Err(HttpError::InvalidUrl(_))));
    assert!(matches!(e.send(get("http://127.0.0.1:1/"), CancellationToken::new()).await, Err(HttpError::Connect(_))));
    let mut req = get("http://127.0.0.1:1/"); req.method = "BAD METHOD".into();
    assert!(matches!(e.send(req, CancellationToken::new()).await, Err(HttpError::Other(_))));
    let mut req = get("http://127.0.0.1:1/"); req.body = HttpBody::File { path: "Z:/definitely/missing.bin".into(), default_content_type: None };
    assert!(matches!(e.send(req, CancellationToken::new()).await, Err(HttpError::Io(_))));
}
#[tokio::test] async fn custom_method_is_sent_as_is() {
    let server = MockServer::start().await;
    Mock::given(method("PURGE")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let (e, _dir) = engine();
    let mut req = get(&server.uri()); req.method = "PURGE".into();
    assert_eq!(e.send(req, CancellationToken::new()).await.unwrap().status, 200);
}
