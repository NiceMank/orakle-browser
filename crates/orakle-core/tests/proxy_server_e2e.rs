use orakle_core::proxy::{validate_navigation_url, ProxyConfig, SecureLoopbackProxy};
use orakle_core::shield::ShieldEngine;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const TEST_RULES: &str = "
||tracker.analytics.local^
||ad.doubleclick.local^
/blocked-tracker.js
/ads/*
@@/ads/allowed-sponsor-notice.js
";

async fn read_http_response(stream: &mut tokio::net::TcpStream) -> String {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 2048];
    while let Ok(n) = stream.read(&mut tmp).await {
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        // Pour les tests simples Connection: close ou upgrade 101
        let text = String::from_utf8_lossy(&buf);
        if text.contains("101 Switching Protocols") && text.contains("\r\n\r\n") {
            break;
        }
    }
    String::from_utf8_lossy(&buf).to_string()
}

/// Serveur Web de test local multi-ressources (HTML, JS, image SVG, Fetch JSON, redirection 302, WebSocket)
async fn spawn_mock_web_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let n = stream.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    let req_str = String::from_utf8_lossy(&buf[..n]);
                    let first_line = req_str.lines().next().unwrap_or("");

                    // Support WebSocket Handshake
                    if req_str.to_ascii_lowercase().contains("upgrade: websocket")
                        && first_line.contains("/ws")
                    {
                        let resp = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
                        let _ = stream.write_all(resp.as_bytes()).await;
                        // Echo simple de la charge WebSocket reçue
                        let mut ws_buf = [0u8; 512];
                        if let Ok(wn) = stream.read(&mut ws_buf).await {
                            if wn > 0 {
                                let _ = stream.write_all(&ws_buf[..wn]).await;
                            }
                        }
                        return;
                    }

                    let (status, extra_headers, content_type, body) = if first_line
                        .contains("/index.html")
                    {
                        (
                            "200 OK",
                            "",
                            "text/html",
                            "<html><body><h1>Orakle Test</h1><script src='/app.js'></script><img src='/logo.svg'/></body></html>",
                        )
                    } else if first_line.contains("/app.js") {
                        (
                            "200 OK",
                            "",
                            "application/javascript",
                            "console.log('App Loaded');",
                        )
                    } else if first_line.contains("/vendor.js") {
                        (
                            "200 OK",
                            "",
                            "application/javascript",
                            "window.vendorReady = true;",
                        )
                    } else if first_line.contains("/ads/allowed-sponsor-notice.js") {
                        (
                            "200 OK",
                            "",
                            "application/javascript",
                            "console.log('Exception JS Loaded');",
                        )
                    } else if first_line.contains("/logo.svg") {
                        (
                            "200 OK",
                            "",
                            "image/svg+xml",
                            "<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'></svg>",
                        )
                    } else if first_line.contains("/redirect") {
                        (
                            "302 Found",
                            "Location: /index.html\r\n",
                            "text/plain",
                            "Redirecting...",
                        )
                    } else if first_line.contains("/api/data") {
                        (
                            "200 OK",
                            "",
                            "application/json",
                            "{\"status\":\"ok\",\"engine\":\"orakle-core\"}",
                        )
                    } else {
                        ("404 Not Found", "", "text/plain", "Not Found")
                    };

                    let response = format!(
                        "HTTP/1.1 {}\r\n{}Content-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        status, extra_headers, content_type, body.len(), body
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        }
    });

    addr
}

#[tokio::test]
async fn test_proxy_e2e_routing_blocking_exceptions_and_fail_closed() {
    println!("\n=== [TEST INTÉGRATION E2E PROXY RUST ORAKLE] ===");

    let mock_addr = spawn_mock_web_server().await;
    let shield = Arc::new(ShieldEngine::new_with_rules(TEST_RULES));

    // Config de test autorisant uniquement le port du mock pour valider le routage HTTP réel
    let test_config = ProxyConfig {
        allowed_ports: vec![80, 443, 8080, mock_addr.port()],
        allow_loopback_upstream: true,
        ..Default::default()
    };

    let (proxy, shutdown_rx) = SecureLoopbackProxy::new(test_config, shield.clone());
    let proxy_addr = proxy.start(shutdown_rx).await.expect("Démarrage du proxy");
    println!(
        "  [1] Proxy de routage démarré sur : {} (mock sur {})",
        proxy_addr, mock_addr
    );

    // 1. HTML + Multiples JS + Image + Fetch + Redirection 302 via le Proxy
    let allowed_paths = [
        ("/index.html", "200 OK", "<h1>Orakle Test</h1>"),
        ("/app.js", "200 OK", "App Loaded"),
        ("/vendor.js", "200 OK", "vendorReady"),
        ("/logo.svg", "200 OK", "<svg"),
        ("/api/data", "200 OK", "\"engine\":\"orakle-core\""),
        ("/redirect", "302 Found", "Location: /index.html"),
    ];

    for (path, expected_status, expected_substr) in allowed_paths {
        let mut client = tokio::net::TcpStream::connect(proxy_addr).await.unwrap();
        let req = format!(
            "GET http://{}{} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            mock_addr, path, mock_addr
        );
        client.write_all(req.as_bytes()).await.unwrap();
        let resp = read_http_response(&mut client).await;
        assert!(resp.contains(expected_status), "Échec {} : {}", path, resp);
        assert!(
            resp.contains(expected_substr),
            "Contenu manquant {} : {}",
            path,
            resp
        );
    }
    println!(
        "  [2] Ressources HTML, JS multiples, Image SVG, API JSON et 302 routées avec succès."
    );

    // 2. Vérification des règles de blocage HTTP & des exceptions @@
    {
        // Bloqué par règle /blocked-tracker.js
        let mut client = tokio::net::TcpStream::connect(proxy_addr).await.unwrap();
        let req = format!(
            "GET http://{}/blocked-tracker.js HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            mock_addr, mock_addr
        );
        client.write_all(req.as_bytes()).await.unwrap();
        let resp = read_http_response(&mut client).await;
        assert!(
            resp.contains("403 Forbidden"),
            "Le traqueur JS doit être bloqué"
        );

        // Bloqué par règle /ads/*
        let mut client_ads = tokio::net::TcpStream::connect(proxy_addr).await.unwrap();
        let req_ads = format!(
            "GET http://{}/ads/banner.js HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            mock_addr, mock_addr
        );
        client_ads.write_all(req_ads.as_bytes()).await.unwrap();
        let resp_ads = read_http_response(&mut client_ads).await;
        assert!(
            resp_ads.contains("403 Forbidden"),
            "/ads/banner.js doit être bloqué"
        );

        // Exception @@/ads/allowed-sponsor-notice.js doit passer !
        let mut client_exc = tokio::net::TcpStream::connect(proxy_addr).await.unwrap();
        let req_exc = format!(
            "GET http://{}/ads/allowed-sponsor-notice.js HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            mock_addr, mock_addr
        );
        client_exc.write_all(req_exc.as_bytes()).await.unwrap();
        let resp_exc = read_http_response(&mut client_exc).await;
        assert!(
            resp_exc.contains("200 OK"),
            "L'exception @@ doit être respectée"
        );
        assert!(resp_exc.contains("Exception JS Loaded"));
    }
    println!("  [3] Blocage sous-ressources et respect des exceptions @@ validés.");

    // 3. Test WebSocket via le Proxy HTTP (Upgrade 101 + Echo)
    {
        let mut ws_client = tokio::net::TcpStream::connect(proxy_addr).await.unwrap();
        let ws_req = format!(
            "GET http://{}/ws HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
            mock_addr, mock_addr
        );
        ws_client.write_all(ws_req.as_bytes()).await.unwrap();
        let handshake_resp = read_http_response(&mut ws_client).await;
        assert!(
            handshake_resp.contains("101 Switching Protocols"),
            "Handshake WebSocket via proxy échoué : {}",
            handshake_resp
        );
        ws_client.write_all(b"PING_ORAKLE").await.unwrap();
        let mut echo_buf = [0u8; 64];
        let en = ws_client.read(&mut echo_buf).await.unwrap();
        assert_eq!(&echo_buf[..en], b"PING_ORAKLE");
    }
    println!("  [4] Tunnel WebSocket (101 Switching Protocols + echo) validé via le proxy.");

    // 4. Vérification Fail-Closed : un port fermé autorisé retourne 502 Bad Gateway (zéro fuite silencieuse)
    {
        let dead_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dead_port = dead_listener.local_addr().unwrap().port();
        drop(dead_listener);

        let cfg_dead = ProxyConfig {
            allowed_ports: vec![dead_port],
            allow_loopback_upstream: true,
            ..Default::default()
        };
        let (dead_proxy, dead_rx) = SecureLoopbackProxy::new(cfg_dead, shield.clone());
        let dead_proxy_addr = dead_proxy.start(dead_rx).await.unwrap();

        let mut c = tokio::net::TcpStream::connect(dead_proxy_addr)
            .await
            .unwrap();
        let req = format!(
            "CONNECT 127.0.0.1:{} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
            dead_port, dead_port
        );
        c.write_all(req.as_bytes()).await.unwrap();
        let resp = read_http_response(&mut c).await;
        assert!(
            resp.contains("502 Bad Gateway"),
            "Doit échouer proprement en 502 : {}",
            resp
        );
        dead_proxy.shutdown();
    }
    println!("  [5] Comportement fail-closed (502 Bad Gateway sur erreur amont) validé.");

    // 5. Validation de la télémétrie Proxy : destinations bien reçues
    let tel = proxy.telemetry_snapshot();
    assert_eq!(tel.allowed_requests, 8); // 6 ressources + 1 exception + 1 WS
    assert_eq!(tel.blocked_by_shield, 2); // /blocked-tracker.js + /ads/banner.js
    assert!(tel
        .events
        .iter()
        .any(|e| e.destination.contains("/index.html") && !e.blocked));
    assert!(tel
        .events
        .iter()
        .any(|e| e.destination.contains("/blocked-tracker.js") && e.blocked));
    println!(
        "  [6] Télémétrie Proxy vérifiée : {} requêtes routées, {} bloquées par Shield.",
        tel.allowed_requests, tel.blocked_by_shield
    );

    // 5b. Tunnel WebSocket ws:// via SOCKS5 (IPv4) + IPv6 ::1 + arrêt proxy fail-closed
    {
        let mut s5 = tokio::net::TcpStream::connect(proxy_addr).await.unwrap();
        s5.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
        let mut g = [0u8; 2];
        s5.read_exact(&mut g).await.unwrap();
        assert_eq!(g, [0x05, 0x00]);

        let ip_bytes = match mock_addr.ip() {
            std::net::IpAddr::V4(v4) => v4.octets(),
            _ => panic!("expected ipv4"),
        };
        let mut req = vec![0x05, 0x01, 0x00, 0x01];
        req.extend_from_slice(&ip_bytes);
        req.extend_from_slice(&mock_addr.port().to_be_bytes());
        s5.write_all(&req).await.unwrap();
        let mut rep = [0u8; 10];
        s5.read_exact(&mut rep).await.unwrap();
        assert_eq!(rep[1], 0x00, "SOCKS5 CONNECT vers mock web doit réussir");

        let ws_upgrade = format!(
            "GET /ws HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
            mock_addr
        );
        s5.write_all(ws_upgrade.as_bytes()).await.unwrap();
        let ws_resp = read_http_response(&mut s5).await;
        assert!(ws_resp.contains("101 Switching Protocols"));
        s5.write_all(b"SOCKS5_WS_OK").await.unwrap();
        let mut echo = [0u8; 32];
        let en = s5.read(&mut echo).await.unwrap();
        assert_eq!(&echo[..en], b"SOCKS5_WS_OK");

        // Vérification IPv6 [::1] si socket dual-stack bindable localement
        if let Ok(v6_listener) = TcpListener::bind("[::1]:0").await {
            let v6_addr = v6_listener.local_addr().unwrap();
            tokio::spawn(async move {
                if let Ok((mut s, _)) = v6_listener.accept().await {
                    let mut b = [0u8; 128];
                    let n = s.read(&mut b).await.unwrap_or(0);
                    let _ = s.write_all(&b[..n]).await;
                }
            });

            let cfg_v6 = ProxyConfig {
                allowed_ports: vec![80, 443, v6_addr.port()],
                allow_loopback_upstream: true,
                ..Default::default()
            };
            let (p6, rx6) = SecureLoopbackProxy::new(cfg_v6, shield.clone());
            let p6_addr = p6.start(rx6).await.unwrap();

            let mut s5_v6 = tokio::net::TcpStream::connect(p6_addr).await.unwrap();
            s5_v6.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
            let mut g6 = [0u8; 2];
            s5_v6.read_exact(&mut g6).await.unwrap();
            let mut r6 = vec![0x05, 0x01, 0x00, 0x04];
            r6.extend_from_slice(&std::net::Ipv6Addr::LOCALHOST.octets());
            r6.extend_from_slice(&v6_addr.port().to_be_bytes());
            s5_v6.write_all(&r6).await.unwrap();
            let mut rep6 = [0u8; 10];
            s5_v6.read_exact(&mut rep6).await.unwrap();
            assert_eq!(rep6[1], 0x00);

            // Simule une préface HTTP/2 dans le tunnel SOCKS5 IPv6
            s5_v6
                .write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
                .await
                .unwrap();
            let mut echo6 = [0u8; 64];
            let en6 = s5_v6.read(&mut echo6).await.unwrap();
            assert_eq!(&echo6[..en6], b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");
            p6.shutdown();
        }
    }
    println!("  [6b] SOCKS5 (ws:// echo + IPv6 ::1 + préface HTTP/2) validé.");

    proxy.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(
        tokio::net::TcpStream::connect(proxy_addr).await.is_err(),
        "Après shutdown, le port proxy doit être fermé (fail-closed, zéro écoute résiduelle)"
    );

    // 6. Test Anti-SSRF strict en configuration PRODUCTION par défaut (allow_loopback_upstream = false)
    {
        let (prod_proxy, prod_rx) = SecureLoopbackProxy::new(ProxyConfig::default(), shield);
        let prod_addr = prod_proxy.start(prod_rx).await.unwrap();

        for target in [
            "tracker.analytics.local:443",
            "192.168.1.50:443",
            "169.254.169.254:80",
            "127.0.0.1:80",
            "localhost:80", // Test anti-DNS-rebinding sur localhost !
            "[::1]:443",
            "example.com:22",
        ] {
            let mut client = tokio::net::TcpStream::connect(prod_addr).await.unwrap();
            let req = format!("CONNECT {} HTTP/1.1\r\nHost: {}\r\n\r\n", target, target);
            client.write_all(req.as_bytes()).await.unwrap();
            let resp = read_http_response(&mut client).await;
            assert!(
                resp.contains("403 Forbidden"),
                "Production proxy doit bloquer {} : {}",
                target,
                resp
            );
        }

        // Vérification DNS inexistant (.invalid) -> 502 Bad Gateway fail-closed
        let mut dns_fail = tokio::net::TcpStream::connect(prod_addr).await.unwrap();
        dns_fail
            .write_all(b"CONNECT does-not-exist.invalid:443 HTTP/1.1\r\nHost: does-not-exist.invalid:443\r\n\r\n")
            .await
            .unwrap();
        let dns_resp = read_http_response(&mut dns_fail).await;
        assert!(dns_resp.contains("502 Bad Gateway"));

        // Redirection vers IP privée ou file:// rejetée
        assert!(validate_navigation_url("http://169.254.169.254/latest/meta-data", false).is_err());
        assert!(validate_navigation_url("file:///etc/shadow", false).is_err());

        prod_proxy.shutdown();
    }
    println!("  [7] Durcissement Anti-SSRF production (RFC1918, metadata, IPv6 ::1, localhost DNS, DNS fail, port 22, shutdown) validé.");
}
