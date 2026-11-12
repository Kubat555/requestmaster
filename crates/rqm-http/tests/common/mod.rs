//! Helpers shared by the integration tests of `rqm-http`.
#![allow(dead_code)]

use std::io::Write;
use std::time::Duration;

use rqm_http::{EngineConfig, HttpBody, HttpRequest, HttpResponse, ProxySetting, ReqwestEngine, SendOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// An engine that stores response bodies in a fresh temporary directory.
pub fn engine() -> (ReqwestEngine, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let config = EngineConfig { body_dir: dir.path().to_path_buf(), user_agent: "RequestMaster/test".into() };
    (ReqwestEngine::new(config).unwrap(), dir)
}

/// `GET` without headers and body; the proxy is off so the tests do not depend on the machine's settings.
pub fn get(url: &str) -> HttpRequest {
    HttpRequest {
        method: "GET".into(),
        url: url.into(),
        headers: Vec::new(),
        body: HttpBody::None,
        options: SendOptions { proxy: ProxySetting::Off, ..SendOptions::default() },
    }
}

/// The decoded response body read back from disk.
pub fn body(response: &HttpResponse) -> Vec<u8> {
    std::fs::read(&response.body_path).unwrap()
}

pub fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

/// HTTP `deflate`: a zlib stream.
pub fn deflate(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

pub fn brotli(bytes: &[u8]) -> Vec<u8> {
    let mut packed = Vec::new();
    brotli::CompressorWriter::new(&mut packed, 4096, 5, 22).write_all(bytes).unwrap();
    packed
}

pub fn zstd(bytes: &[u8]) -> Vec<u8> {
    zstd::encode_all(bytes, 3).unwrap()
}

/// Raw TCP server on `127.0.0.1`: answers every connection with the headers at once and the body
/// `hello` after `delay`. Returns its address.
pub async fn slow_body_server(delay: Duration) -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                // Read the request up to the blank line first: closing a socket with unread data
                // makes Windows reset the connection.
                let mut request = Vec::new();
                let mut chunk = [0u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    match socket.read(&mut chunk).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => request.extend_from_slice(&chunk[..n]),
                    }
                }
                let head = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\n";
                if socket.write_all(head).await.is_err() {
                    return;
                }
                tokio::time::sleep(delay).await;
                let _ = socket.write_all(b"hello").await;
                let _ = socket.shutdown().await;
            });
        }
    });
    addr
}
