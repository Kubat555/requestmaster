//! Step 6 of the spike: one real HTTPS request with verification on, plus the detected system proxy.
//! Usage: `cargo run -- https://example.com`

use std::time::Duration;

use http_probe::{send, system_proxy, SendOptions};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    let url = std::env::args().nth(1).expect("usage: http-probe <url>");
    let opts = SendOptions { follow_redirects: true, max_redirects: 10, verify_tls: true, timeout: Duration::from_secs(30) };
    match send("GET", &url, &[], None, &opts, CancellationToken::new()).await {
        Ok(o) => println!(
            "status={} hops={} ttfb={:?} total={:?} wire_bytes={} body_bytes={}",
            o.status,
            o.hops.len(),
            o.ttfb,
            o.total,
            o.wire_bytes,
            o.body.len()
        ),
        Err(e) => println!("error: {e:?}"),
    }
    println!("system_proxy={:?}", system_proxy());
}
