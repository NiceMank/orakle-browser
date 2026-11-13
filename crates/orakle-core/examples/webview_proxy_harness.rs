use orakle_core::proxy::{ProxyConfig, SecureLoopbackProxy};
use orakle_core::shield::ShieldEngine;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

const HARNESS_RULES: &str = "
||doubleclick.net^
/tracker-pixel.js
/ads/*
@@/ads/sponsor-allowed.js
";

async fn spawn_origin_a(
    port_b_holder: Arc<Mutex<u16>>,
    hits_a: Arc<Mutex<Vec<String>>>,
) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = listener.accept().await {
                let port_b = *port_b_holder.lock().unwrap();
                let hits = hits_a.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let n = stream.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let first_line = req.lines().next().unwrap_or("");
                    let path = first_line.split_whitespace().nth(1).unwrap_or("/");
                    hits.lock().unwrap().push(path.to_string());

                    if req.to_ascii_lowercase().contains("upgrade: websocket") && path == "/ws" {
                        // Calcul standard Sec-WebSocket-Accept simplifié non requis pour test ou SHA1 standard
                        let key = req
                            .lines()
                            .find_map(|l| {
                                let (k, v) = l.split_once(':')?;
                                if k.trim().eq_ignore_ascii_case("sec-websocket-key") {
                                    Some(v.trim().to_string())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_default();
                        let accept = compute_ws_accept(&key);
                        let resp = format!(
                            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
                            accept
                        );
                        let _ = stream.write_all(resp.as_bytes()).await;
                        // Envoie un frame WebSocket texte "ORAKLE_WS_OK" puis attend la fermeture propre du client
                        let msg = b"ORAKLE_WS_OK";
                        let mut frame = vec![0x81u8, msg.len() as u8];
                        frame.extend_from_slice(msg);
                        let _ = stream.write_all(&frame).await;
                        let mut drain = [0u8; 256];
                        let _ = tokio::time::timeout(
                            std::time::Duration::from_millis(1000),
                            stream.read(&mut drain),
                        )
                        .await;
                        return;
                    }

                    let html_body = format!(
                        r#"<!doctype html>
<html>
<head><meta charset="utf-8"><title>Orakle WebView Proxy Test</title></head>
<body>
  <h1 id="title">Orakle Proxy E2E</h1>
  <img id="logo" src="/logo.svg" alt="logo" />
  <div id="status">loading</div>
  <script src="/app.js"></script>
  <script src="/vendor.js"></script>
  <script src="http://127.0.0.1:{b}/third-party.js"></script>
  <script src="http://127.0.0.1:{b}/tracker-pixel.js"></script>
  <script src="http://127.0.0.1:{b}/ads/banner.js"></script>
  <script src="http://127.0.0.1:{b}/ads/sponsor-allowed.js"></script>
  <script>
    (async () => {{
      const results = {{ app: !!window.orakleApp, vendor: !!window.orakleVendor, thirdParty: !!window.thirdPartyLoaded, trackerBlocked: !window.trackerLoaded, adBannerBlocked: !window.adBannerLoaded, exceptionAllowed: !!window.sponsorExceptionLoaded }};
      try {{
        const apiRes = await fetch('/api/data');
        results.fetchJson = await apiRes.json();
      }} catch (e) {{ results.fetchError = String(e); }}
      try {{
        const redirRes = await fetch('/redirect-test');
        results.redirectJson = await redirRes.json();
      }} catch (e) {{ results.redirectError = String(e); }}
      results.ws = await new Promise((resolve) => {{
        try {{
          const ws = new WebSocket('ws://' + location.host + '/ws');
          ws.onmessage = (ev) => {{ resolve(ev.data); ws.close(); }};
          ws.onerror = () => resolve('ws_error');
          setTimeout(() => resolve('ws_timeout'), 1500);
        }} catch (e) {{ resolve('ws_exc:' + e); }}
      }});
      document.getElementById('status').textContent = JSON.stringify(results);
      document.title = 'DONE:' + JSON.stringify(results);
    }})();
  </script>
</body>
</html>"#,
                        b = port_b
                    );

                    let (status, extra, ctype, body): (&str, &str, &str, String) = match path {
                        "/index.html" | "/" => ("200 OK", "", "text/html; charset=utf-8", html_body),
                        "/app.js" => ("200 OK", "", "application/javascript", "window.orakleApp = true;".into()),
                        "/vendor.js" => ("200 OK", "", "application/javascript", "window.orakleVendor = true;".into()),
                        "/logo.svg" => (
                            "200 OK",
                            "",
                            "image/svg+xml",
                            "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><circle cx='12' cy='12' r='10'/></svg>".into(),
                        ),
                        "/api/data" => ("200 OK", "", "application/json", "{\"ok\":true,\"source\":\"origin-a\"}".into()),
                        "/redirect-test" => ("302 Found", "Location: /redirect-target\r\n", "text/plain", "Redirect".into()),
                        "/redirect-target" => ("200 OK", "", "application/json", "{\"redirected\":true}".into()),
                        _ => ("404 Not Found", "", "text/plain", "Not Found".into()),
                    };

                    let resp = format!(
                        "HTTP/1.1 {}\r\n{}Content-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        status,
                        extra,
                        ctype,
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                });
            }
        }
    });

    addr
}

async fn spawn_origin_b(hits_b: Arc<Mutex<Vec<String>>>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = listener.accept().await {
                let hits = hits_b.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let n = stream.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let first_line = req.lines().next().unwrap_or("");
                    let path = first_line.split_whitespace().nth(1).unwrap_or("/");
                    hits.lock().unwrap().push(path.to_string());

                    let (status, ctype, body) = match path {
                        "/third-party.js" => (
                            "200 OK",
                            "application/javascript",
                            "window.thirdPartyLoaded = true;",
                        ),
                        "/tracker-pixel.js" => (
                            "200 OK",
                            "application/javascript",
                            "window.trackerLoaded = true;",
                        ),
                        "/ads/banner.js" => (
                            "200 OK",
                            "application/javascript",
                            "window.adBannerLoaded = true;",
                        ),
                        "/ads/sponsor-allowed.js" => (
                            "200 OK",
                            "application/javascript",
                            "window.sponsorExceptionLoaded = true;",
                        ),
                        _ => ("404 Not Found", "text/plain", "Not Found"),
                    };

                    let resp = format!(
                        "HTTP/1.1 {}\r\nContent-Type: {}\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        status,
                        ctype,
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                });
            }
        }
    });

    addr
}

/// SHA-1 + Base64 minimal RFC 6455 pour handshake WebSocket réel avec WebKitGTK / Chromium
fn compute_ws_accept(key: &str) -> String {
    let input = format!("{}258EAFA5-E914-47DA-95CA-C5AB0DC85B11", key.trim());
    let digest = sha1_bytes(input.as_bytes());
    base64_encode(&digest)
}

fn sha1_bytes(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let bit_len = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (i, val) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&val.to_be_bytes());
    }
    out
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[tokio::main]
async fn main() {
    let hits_a = Arc::new(Mutex::new(Vec::new()));
    let hits_b = Arc::new(Mutex::new(Vec::new()));

    let addr_b = spawn_origin_b(hits_b.clone()).await;
    let port_b_holder = Arc::new(Mutex::new(addr_b.port()));
    let addr_a = spawn_origin_a(port_b_holder, hits_a.clone()).await;

    let shield = Arc::new(ShieldEngine::new_with_rules(HARNESS_RULES));
    let mut config = ProxyConfig::default();
    config.allowed_ports = vec![80, 443, 8080, addr_a.port(), addr_b.port()];
    config.allow_loopback_upstream = true;

    let (proxy, shutdown_rx) = SecureLoopbackProxy::new(config, shield);
    let proxy_addr = proxy.start(shutdown_rx).await.expect("start proxy");

    println!(
        "HARNESS_READY proxy={} origin_a={} origin_b={}",
        proxy_addr, addr_a, addr_b
    );
    let mut stdin_reader = BufReader::new(tokio::io::stdin()).lines();
    let _ = stdin_reader.next_line().await;

    let tel = proxy.telemetry_snapshot();
    println!(
        "PROXY_TELEMETRY total={} allowed={} blocked_shield={} blocked_ssrf={} blocked_port={}",
        tel.total_requests,
        tel.allowed_requests,
        tel.blocked_by_shield,
        tel.blocked_ssrf,
        tel.blocked_port
    );
    for ev in &tel.events {
        let label = if ev.blocked {
            format!("BLOCKED({}, {:?})", ev.detail, ev.rule)
        } else {
            "ALLOWED".to_string()
        };
        println!(
            "EVENT method={} dest={} decision={}",
            ev.method, ev.destination, label
        );
    }
    println!("ORIGIN_A_HITS={:?}", *hits_a.lock().unwrap());
    println!("ORIGIN_B_HITS={:?}", *hits_b.lock().unwrap());

    proxy.shutdown();
}
