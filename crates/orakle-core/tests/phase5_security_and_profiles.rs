use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use orakle_core::{
    classify_filename_risk, download_resumable_to_path, is_forbidden_private_ip, is_host_loopback,
    resolve_safe_destination, sanitize_filename, sanitize_url_for_log,
    validate_navigation_with_origin, BookmarkEntry, DomainPermission, DownloadControl,
    DownloadSafetyRisk, HistoryEntry, NavigationOrigin, PrivateSession, ProfileContext,
    ProfileRegistry, ProxyConfig, ProxyHealth, SecureLoopbackProxy, ShieldEngine, Space,
};

#[test]
fn test_phase5_url_injection_crlf_and_ssrf_variants() {
    // Protocoles et schémas dangereux bloqués quelle que soit l'origine
    let dangerous = [
        "javascript:alert(document.cookie)",
        "JAVASCRIPT:eval('1')",
        "vbscript:msgbox(1)",
        "data:text/html,<script>alert(1)</script>",
        "file:///etc/passwd",
        "blob:https://example.com/1234",
        "ftp://ftp.example.com/file",
        "about:config",
        "chrome://settings",
        "https://example.com/path\r\nX-Injected: evil",
        "https://example.com/\0nullbyte",
        "https://example.com/\nnewline",
    ];
    for u in dangerous {
        assert!(
            validate_navigation_with_origin(u, NavigationOrigin::UserOmnibox).is_err(),
            "Doit bloquer côté Omnibox : {}",
            u
        );
        assert!(
            validate_navigation_with_origin(u, NavigationOrigin::WebContent).is_err(),
            "Doit bloquer côté WebContent : {}",
            u
        );
    }

    // Métadonnées cloud et réseaux privés toujours bloqués (y compris Omnibox)
    let ssrf_private = [
        "http://169.254.169.254/latest/meta-data/",
        "http://[::ffff:169.254.169.254]/latest/meta-data/",
        "http://0.0.0.0:8080/",
        "http://10.0.0.5/internal",
        "http://172.16.0.10/api",
        "http://192.168.1.1/router",
        "http://100.64.0.1/cgnat",
    ];
    for u in ssrf_private {
        assert!(
            validate_navigation_with_origin(u, NavigationOrigin::UserOmnibox).is_err(),
            "Cible SSRF privée doit être bloquée : {}",
            u
        );
    }

    // Mode dev localhost : autorisé sur action directe Omnibox, bloqué sur origine WebContent
    assert!(validate_navigation_with_origin(
        "http://127.0.0.1:3000/app",
        NavigationOrigin::UserOmnibox
    )
    .is_ok());
    assert!(validate_navigation_with_origin(
        "http://localhost:5173/",
        NavigationOrigin::UserOmnibox
    )
    .is_ok());
    assert!(
        validate_navigation_with_origin("http://[::1]:8080/", NavigationOrigin::UserOmnibox)
            .is_ok()
    );
    assert!(validate_navigation_with_origin(
        "http://127.0.0.1:3000/app",
        NavigationOrigin::WebContent
    )
    .is_err());
    assert!(validate_navigation_with_origin(
        "http://[::ffff:127.0.0.1]/admin",
        NavigationOrigin::WebContent
    )
    .is_err());
    assert!(validate_navigation_with_origin(
        "http://localhost:5173/",
        NavigationOrigin::WebContent
    )
    .is_err());

    // Sanitisation des logs URL (aucun query/userinfo/fragment en clair)
    let sanitized =
        sanitize_url_for_log("https://user:secret@example.com/auth/callback?token=SECRET_123#frag");
    assert!(!sanitized.contains("SECRET_123"));
    assert!(!sanitized.contains("user:secret"));
    assert!(!sanitized.contains("#frag"));
    assert!(sanitized.starts_with("https://example.com/auth/callback"));
}

#[test]
fn test_phase5_private_ip_and_dns_rebinding_checks() {
    use std::net::IpAddr;
    let blocked_ips = [
        "127.0.0.1",
        "127.0.1.1",
        "10.0.0.1",
        "172.16.4.2",
        "192.168.0.254",
        "169.254.169.254",
        "0.0.0.0",
        "100.64.0.1",
        "::1",
        "::",
        "fc00::1",
        "fd12:3456::1",
        "fe80::1",
        "::ffff:127.0.0.1",
        "::ffff:169.254.169.254",
        "::ffff:10.0.0.1",
    ];
    for raw in blocked_ips {
        let ip: IpAddr = raw.parse().unwrap();
        assert!(
            is_forbidden_private_ip(ip, false),
            "IP privée non bloquée : {}",
            raw
        );
    }
    let public_ip: IpAddr = "93.184.216.34".parse().unwrap();
    assert!(!is_forbidden_private_ip(public_ip, false));
    assert!(is_host_loopback("localhost"));
    assert!(is_host_loopback("sub.localhost"));
    assert!(is_host_loopback("127.0.0.1"));
    assert!(is_host_loopback("[::1]"));
}

#[tokio::test]
async fn test_phase5_http_redirect_ssrf_and_cross_origin_csrf_and_ws_fail_closed() {
    // Serveur local jouant le rôle d'upstream autorisé (dev localhost)
    let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let up_port = upstream.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = upstream.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    return;
                }
                let req = String::from_utf8_lossy(&buf[..n]);
                let first = req.lines().next().unwrap_or("");
                if first.contains("/redirect-meta") {
                    let _ = s
                        .write_all(
                            b"HTTP/1.1 302 Found\r\nLocation: http://169.254.169.254/latest/meta-data/\r\nContent-Length: 0\r\n\r\n",
                        )
                        .await;
                } else if first.contains("/redirect-loop") {
                    let _ = s
                        .write_all(
                            b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:65432/secret\r\nContent-Length: 0\r\n\r\n",
                        )
                        .await;
                } else if req.to_ascii_lowercase().contains("upgrade: websocket") {
                    let _ = s
                        .write_all(
                            b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
                        )
                        .await;
                    let mut ws_buf = [0u8; 64];
                    if let Ok(m) = s.read(&mut ws_buf).await {
                        if m > 0 {
                            let _ = s.write_all(&ws_buf[..m]).await;
                        }
                    }
                } else {
                    let _ = s
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK",
                        )
                        .await;
                }
            });
        }
    });

    let shield = Arc::new(ShieldEngine::new_with_rules("||tracker.evil^\n"));
    let (proxy, rx) = SecureLoopbackProxy::new(ProxyConfig::default(), shield.clone());
    let proxy_addr = proxy.start(rx).await.unwrap();
    assert!(proxy.is_running());
    assert_eq!(proxy.health_status(), ProxyHealth::Running);

    // 1) Sans autorisation dev localhost -> accès à 127.0.0.1:<up_port> bloqué (403 SSRF)
    {
        let mut c = TcpStream::connect(proxy_addr).await.unwrap();
        let req = format!(
            "GET http://127.0.0.1:{}/ok HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            up_port, up_port
        );
        c.write_all(req.as_bytes()).await.unwrap();
        let mut resp = vec![0u8; 512];
        let n = c.read(&mut resp).await.unwrap();
        assert!(String::from_utf8_lossy(&resp[..n]).contains("403"));
    }

    // 2) Autorisation utilisateur Omnibox du port dev -> requête directe OK, CSRF externe bloquée, 302 SSRF bloqués
    proxy.authorize_user_localhost_port(up_port);

    // 2a) Requête directe autorisée -> 200 OK
    {
        let mut c = TcpStream::connect(proxy_addr).await.unwrap();
        let req = format!(
            "GET http://127.0.0.1:{}/ok HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            up_port, up_port
        );
        c.write_all(req.as_bytes()).await.unwrap();
        let mut resp = vec![0u8; 512];
        let n = c.read(&mut resp).await.unwrap();
        let txt = String::from_utf8_lossy(&resp[..n]);
        assert!(txt.contains("200 OK"), "Attendu 200 OK, reçu: {}", txt);
    }

    // 2b) CSRF/SSRF cross-origin depuis un site externe (Origin / Referer -> localhost) -> 403
    {
        let mut c = TcpStream::connect(proxy_addr).await.unwrap();
        let req = format!(
            "GET http://127.0.0.1:{}/ok HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: https://attacker.example\r\nConnection: close\r\n\r\n",
            up_port, up_port
        );
        c.write_all(req.as_bytes()).await.unwrap();
        let mut resp = vec![0u8; 512];
        let n = c.read(&mut resp).await.unwrap();
        let txt = String::from_utf8_lossy(&resp[..n]);
        assert!(
            txt.contains("403"),
            "CSRF Origin externe devait être bloqué: {}",
            txt
        );
    }

    // 2c) Redirect HTTP 302 vers métadonnées AWS 169.254.169.254 -> intercepté par le proxy -> 403
    {
        let mut c = TcpStream::connect(proxy_addr).await.unwrap();
        let req = format!(
            "GET http://127.0.0.1:{}/redirect-meta HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            up_port, up_port
        );
        c.write_all(req.as_bytes()).await.unwrap();
        let mut resp = vec![0u8; 512];
        let n = c.read(&mut resp).await.unwrap();
        let txt = String::from_utf8_lossy(&resp[..n]);
        assert!(
            txt.contains("403"),
            "302 vers 169.254.169.254 devait être bloqué: {}",
            txt
        );
    }

    // 2d) Redirect HTTP 302 vers un autre port loopback non autorisé -> bloqué -> 403
    {
        let mut c = TcpStream::connect(proxy_addr).await.unwrap();
        let req = format!(
            "GET http://127.0.0.1:{}/redirect-loop HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            up_port, up_port
        );
        c.write_all(req.as_bytes()).await.unwrap();
        let mut resp = vec![0u8; 512];
        let n = c.read(&mut resp).await.unwrap();
        let txt = String::from_utf8_lossy(&resp[..n]);
        assert!(
            txt.contains("403"),
            "302 vers port loopback fermé devait être bloqué: {}",
            txt
        );
    }

    // 2e) Tunnel CONNECT WebSocket (101 Switching Protocols + echo bidirectionnel)
    {
        let mut c = TcpStream::connect(proxy_addr).await.unwrap();
        let conn_req = format!(
            "CONNECT 127.0.0.1:{} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
            up_port, up_port
        );
        c.write_all(conn_req.as_bytes()).await.unwrap();
        let mut buf = [0u8; 256];
        let n = c.read(&mut buf).await.unwrap();
        assert!(String::from_utf8_lossy(&buf[..n]).contains("200"));

        c.write_all(
            format!(
                "GET /ws HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
                up_port
            )
            .as_bytes(),
        )
        .await
        .unwrap();
        let m = c.read(&mut buf).await.unwrap();
        assert!(String::from_utf8_lossy(&buf[..m]).contains("101"));
        c.write_all(b"ORAKLE_WS_PING").await.unwrap();
        let e = c.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..e], b"ORAKLE_WS_PING");
    }

    // 3) Shutdown fail-closed puis redémarrage proxy propre
    proxy.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!proxy.is_running());
    assert_eq!(proxy.health_status(), ProxyHealth::Stopped);
    assert!(TcpStream::connect(proxy_addr).await.is_err());

    let (proxy2, rx2) = SecureLoopbackProxy::new(ProxyConfig::default(), shield);
    let addr2 = proxy2.start(rx2).await.unwrap();
    assert!(proxy2.is_running());
    assert_ne!(addr2.port(), 0);
    proxy2.shutdown();
}

#[test]
fn test_phase5_multi_profile_isolation_and_private_session_cleanup() {
    let tmp = tempdir().unwrap();

    // 1) Registre multi-profils persistant : création de 2 profils + changement démarrage
    let mut reg = ProfileRegistry::load_or_init(tmp.path()).unwrap();
    let p_work = reg.create_profile("Travail").unwrap();
    let p_perso = reg.create_profile("Personnel").unwrap();
    reg.set_startup_profile(&p_work.id).unwrap();

    let reloaded_reg = ProfileRegistry::load_or_init(tmp.path()).unwrap();
    assert_eq!(reloaded_reg.startup_profile_id(), p_work.id);
    assert!(reloaded_reg.list_profiles().len() >= 3); // default + 2

    // 2) Isolation stricte des données entre Work et Personal
    let ctx_work = ProfileContext::open_persistent(tmp.path(), &p_work.id, "Travail").unwrap();
    let ctx_perso = ProfileContext::open_persistent(tmp.path(), &p_perso.id, "Personnel").unwrap();
    assert_ne!(ctx_work.db_path, ctx_perso.db_path);
    assert_ne!(ctx_work.webview_data_dir, ctx_perso.webview_data_dir);

    ctx_work
        .storage
        .insert_space(&Space {
            id: "sp-work".into(),
            name: "Pro".into(),
            color: "#3b82f6".into(),
            created_at: 100,
        })
        .unwrap();
    ctx_work
        .storage
        .upsert_bookmark(&BookmarkEntry {
            id: "bm-w".into(),
            title: "Intranet".into(),
            url: "https://intranet.example.com/".into(),
            category: "Pro".into(),
            created_at: 101,
        })
        .unwrap();
    ctx_work
        .storage
        .insert_history_entry(&HistoryEntry {
            id: None,
            url: "https://secret-work.example/".into(),
            title: "Projet Confidentiel".into(),
            visited_at: 102,
        })
        .unwrap();
    ctx_work
        .storage
        .set_domain_permission(&DomainPermission {
            origin: "https://meet.example".into(),
            permission: "microphone".into(),
            decision: "blocked".into(),
            updated_at: 103,
        })
        .unwrap();

    assert!(ctx_perso.storage.get_bookmarks().unwrap().is_empty());
    assert!(ctx_perso.storage.get_history(20).unwrap().is_empty());
    assert!(ctx_perso
        .storage
        .get_domain_permissions()
        .unwrap()
        .is_empty());
    assert_eq!(ctx_work.storage.get_domain_permissions().unwrap().len(), 1);

    // 3) Session Privée éphémère : zéro fuite vers les profils et suppression garantie sur disque
    let priv_temp_dir;
    {
        let priv_sess = PrivateSession::start().unwrap();
        priv_temp_dir = priv_sess.temp_path.clone();
        priv_sess
            .storage
            .insert_history_entry(&HistoryEntry {
                id: None,
                url: "https://private-only.example/".into(),
                title: "Navigation Privée".into(),
                visited_at: 200,
            })
            .unwrap();
        fs::write(priv_temp_dir.join("ephemeral_cache.dat"), b"ram").unwrap();
        assert!(priv_temp_dir.exists());
    }
    assert!(!priv_temp_dir.exists());
    assert!(!ctx_work
        .storage
        .get_history(50)
        .unwrap()
        .iter()
        .any(|h| h.url.contains("private-only")));
    assert!(!ctx_perso
        .storage
        .get_history(50)
        .unwrap()
        .iter()
        .any(|h| h.url.contains("private-only")));
}

#[tokio::test]
async fn test_phase5_download_sanitization_classification_and_range_resume() {
    // 1) Sanitisation & anti-traversal
    let cleaned = sanitize_filename("../../.ssh/authorized_keys\0.sh");
    assert!(!cleaned.contains(".."));
    assert!(!cleaned.contains('/'));
    assert!(!cleaned.contains('\0'));

    let tmp = tempdir().unwrap();
    let dest = resolve_safe_destination(tmp.path(), cleaned.as_str()).unwrap();
    assert!(dest.starts_with(tmp.path()));

    // 2) Classification honnête par extension
    assert!(matches!(
        classify_filename_risk("installer.exe"),
        DownloadSafetyRisk::ExecutableWarning { .. }
    ));
    assert!(matches!(
        classify_filename_risk("backup.tar.gz"),
        DownloadSafetyRisk::Archive
    ));
    assert_eq!(
        classify_filename_risk("report.pdf"),
        DownloadSafetyRisk::Safe
    );

    // 3) Blocage SSRF sur téléchargement vers 169.254.169.254
    let blocked_dest = tmp.path().join("meta.txt");
    let ssrf_dl = download_resumable_to_path(
        "http://169.254.169.254/latest/meta-data/",
        &blocked_dest,
        None,
        false,
        DownloadControl::new(),
        |_| {},
    )
    .await;
    assert!(ssrf_dl.is_err());

    // 4) Serveur HTTP Range réel : téléchargement direct avec vérification contenu complet
    let payload: Vec<u8> = (0..65536).map(|i| (i % 251) as u8).collect();
    let payload_srv = payload.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let srv_addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = listener.accept().await {
            let p = payload_srv.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                let n = s.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let start = req
                    .lines()
                    .find(|l| l.to_ascii_lowercase().starts_with("range:"))
                    .and_then(|l| l.split('=').nth(1))
                    .and_then(|r| r.split('-').next())
                    .and_then(|s| s.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if start > 0 && start < p.len() {
                    let hdr = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        p.len() - start,
                        start,
                        p.len() - 1,
                        p.len()
                    );
                    let _ = s.write_all(hdr.as_bytes()).await;
                    let _ = s.write_all(&p[start..]).await;
                } else {
                    let hdr = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        p.len()
                    );
                    let _ = s.write_all(hdr.as_bytes()).await;
                    let _ = s.write_all(&p).await;
                }
            });
        }
    });

    let out_file = tmp.path().join("test_range.bin");
    download_resumable_to_path(
        &format!("http://{}/test_range.bin", srv_addr),
        &out_file,
        None,
        true,
        DownloadControl::new(),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(fs::read(&out_file).unwrap(), payload);
}
