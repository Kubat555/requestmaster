use std::io::{Read, Write};
use std::net::TcpListener as StdTcpListener;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use http_probe::{send, system_proxy, SendError, SendOptions};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------- fixtures ----------

/// One wiremock server for the whole test binary. wiremock serves from its own thread, so it outlives
/// the per-test runtimes; mocks are mounted from a throwaway runtime on a helper thread.
fn mock_server() -> &'static MockServer {
    static SERVER: OnceLock<MockServer> = OnceLock::new();
    SERVER.get_or_init(|| {
        std::thread::spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            rt.block_on(async {
                let s = MockServer::start().await;
                let redirect = |code: u16, to: &str| ResponseTemplate::new(code).insert_header("Location", to);
                Mock::given(method("GET")).and(path("/a")).respond_with(redirect(302, "/b")).mount(&s).await;
                Mock::given(method("GET")).and(path("/b")).respond_with(redirect(301, "/c")).mount(&s).await;
                Mock::given(method("GET"))
                    .and(path("/c"))
                    .respond_with(ResponseTemplate::new(200).set_body_string("end"))
                    .mount(&s)
                    .await;
                Mock::given(method("GET"))
                    .and(path("/gz"))
                    .respond_with(
                        ResponseTemplate::new(200)
                            .insert_header("Content-Encoding", "gzip")
                            .set_body_bytes(gzip(&vec![0u8; 100 * 1024])),
                    )
                    .mount(&s)
                    .await;
                Mock::given(method("GET"))
                    .and(path("/slow"))
                    .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
                    .mount(&s)
                    .await;
                s
            })
        })
        .join()
        .unwrap()
    })
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(data).unwrap();
    enc.finish().unwrap()
}

/// wiremock cannot delay the body after the headers, so `/drip` is a raw HTTP/1.1 server:
/// headers go out immediately, the 4-byte body 300 ms later.
fn drip_server() -> &'static str {
    static BASE: OnceLock<String> = OnceLock::new();
    BASE.get_or_init(|| {
        let listener = StdTcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                std::thread::spawn(move || {
                    read_request_head(&mut s);
                    let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n");
                    let _ = s.flush();
                    std::thread::sleep(Duration::from_millis(300));
                    let _ = s.write_all(b"drip");
                });
            }
        });
        base
    })
}

fn read_request_head(s: &mut impl Read) {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while !buf.ends_with(b"\r\n\r\n") {
        match s.read(&mut byte) {
            Ok(1) => buf.push(byte[0]),
            _ => return,
        }
    }
}

fn url(p: &str) -> String {
    if p == "/drip" {
        return format!("{}{}", drip_server(), p);
    }
    format!("{}{}", mock_server().uri(), p)
}

/// Minimal TLS server with an rcgen self-signed certificate, 127.0.0.1 on an ephemeral port.
/// Answers every request with `200 ok`.
fn tls_url(p: &str) -> String {
    static BASE: OnceLock<String> = OnceLock::new();
    let base = BASE.get_or_init(|| {
        let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string(), "127.0.0.1".to_string()]).unwrap();
        let cert = ck.cert.der().clone();
        let key = rustls::pki_types::PrivateKeyDer::Pkcs8(ck.signing_key.serialize_der().into());
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .unwrap();
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));

        let listener = StdTcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("https://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            rt.block_on(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                loop {
                    let Ok((tcp, _)) = listener.accept().await else { continue };
                    let acceptor = acceptor.clone();
                    tokio::spawn(async move {
                        // A client that rejects the certificate aborts the handshake: just drop it.
                        let Ok(mut tls) = acceptor.accept(tcp).await else { return };
                        let mut buf = Vec::new();
                        let mut chunk = [0u8; 1024];
                        while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                            match tls.read(&mut chunk).await {
                                Ok(0) | Err(_) => return,
                                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                            }
                        }
                        let _ = tls
                            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                            .await;
                        let _ = tls.shutdown().await;
                    });
                }
            });
        });
        base
    });
    format!("{base}{p}")
}

fn opts(follow: bool) -> SendOptions {
    SendOptions { follow_redirects: follow, max_redirects: 10, verify_tls: true, timeout: Duration::from_secs(30) }
}

fn cancel_after(ms: u64) -> CancellationToken {
    let token = CancellationToken::new();
    let t = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        t.cancel();
    });
    token
}

// ---------- tests (bodies verbatim from the task brief) ----------

#[tokio::test] async fn redirect_chain_is_captured() {
    // /a -> 302 /b -> 301 /c -> 200 "end"
    let out = send("GET", &url("/a"), &[], None, &opts(true), CancellationToken::new()).await.unwrap();
    assert_eq!(out.status, 200);
    assert_eq!(out.hops.iter().map(|h| h.status).collect::<Vec<_>>(), vec![302, 301]);
}
#[tokio::test] async fn redirects_not_followed_when_disabled() {
    let out = send("GET", &url("/a"), &[], None, &opts(false), CancellationToken::new()).await.unwrap();
    assert_eq!(out.status, 302);
    assert!(out.hops.is_empty());
}
#[tokio::test] async fn gzip_body_is_decoded_and_wire_size_reported() {
    // сервер отдаёт 100 КБ нулей, сжатых gzip
    let out = send("GET", &url("/gz"), &[], None, &opts(true), CancellationToken::new()).await.unwrap();
    assert_eq!(out.body.len(), 100 * 1024);
    assert!(out.wire_bytes < 10 * 1024);
}
#[tokio::test] async fn cancel_stops_slow_request() {
    // сервер отвечает через 5 с; отмена через 100 мс
    let t = Instant::now();
    let r = send("GET", &url("/slow"), &[], None, &opts(true), cancel_after(100)).await;
    assert!(matches!(r, Err(SendError::Cancelled)));
    assert!(t.elapsed() < Duration::from_secs(1));
}
#[tokio::test] async fn ttfb_is_less_than_total_for_delayed_body() {
    let out = send("GET", &url("/drip"), &[], None, &opts(true), CancellationToken::new()).await.unwrap();
    assert!(out.ttfb < out.total);
}
#[tokio::test] async fn self_signed_fails_by_default() {
    let r = send("GET", &tls_url("/"), &[], None, &opts(true), CancellationToken::new()).await;
    assert!(matches!(r, Err(SendError::Tls(_))));
}
#[tokio::test] async fn self_signed_passes_when_verification_disabled() {
    let o = SendOptions { verify_tls: false, ..opts(true) };
    assert_eq!(send("GET", &tls_url("/"), &[], None, &o, CancellationToken::new()).await.unwrap().status, 200);
}
#[test] fn system_proxy_is_detected() {
    // не утверждение о значении: функция не паникует и возвращает то же, что настройки Windows
    let _ = system_proxy();
}
