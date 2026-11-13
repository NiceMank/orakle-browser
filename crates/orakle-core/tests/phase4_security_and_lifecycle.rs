use orakle_core::{
    compile_to_wk_content_rule_list, platform_capability_matrix,
    sync_rules_file_for_webkit_extension, validate_navigation_url, DownloadRecord, NoteEntry,
    PrivateSession, ProfileContext, ProxyConfig, SecureLoopbackProxy, SessionRecord, ShieldEngine,
    Space, StorageEngine, TabGroup, TabRecord,
};
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[test]
fn test_navigation_url_security_policy() {
    // Schémas dangereux strictement rejetés
    assert!(validate_navigation_url("javascript:alert(1)", false).is_err());
    assert!(validate_navigation_url("file:///etc/passwd", false).is_err());
    assert!(validate_navigation_url("data:text/html,<h1>x</h1>", false).is_err());
    assert!(validate_navigation_url("ftp://example.com/a", false).is_err());
    assert!(validate_navigation_url("about:config", false).is_err());

    // Cibles SSRF / IP privées / métadonnées cloud rejetées par défaut
    assert!(validate_navigation_url("http://127.0.0.1:8080/", false).is_err());
    assert!(validate_navigation_url("http://[::1]:8080/", false).is_err());
    assert!(validate_navigation_url("http://169.254.169.254/latest/meta-data/", false).is_err());
    assert!(validate_navigation_url("http://10.0.0.1/", false).is_err());
    assert!(validate_navigation_url("http://192.168.1.10/", false).is_err());
    assert!(validate_navigation_url("http://localhost:3000/", false).is_err());

    // Mode test local explicite : autorise loopback uniquement, garde le blocage cloud/LAN
    assert!(validate_navigation_url("http://127.0.0.1:8080/home", true).is_ok());
    assert!(validate_navigation_url("http://169.254.169.254/latest/meta-data/", true).is_err());
    assert!(validate_navigation_url("http://192.168.1.1/", true).is_err());

    // Navigation HTTPS publique autorisée
    assert!(validate_navigation_url("https://doc.rust-lang.org/book/", false).is_ok());
}

#[test]
fn test_shield_dynamic_toggle_exceptions_and_webkit_rules_sync() {
    let shield = ShieldEngine::new_with_rules("||doubleclick.net^\n/blocked-tracker.js\n");
    let d1 = shield
        .check_network_request(
            "https://stats.doubleclick.net/pixel.js",
            "https://example.com/",
            "script",
            "GET",
        )
        .unwrap();
    assert!(d1.blocked);

    // Exception dynamique par domaine
    shield.add_exception_domain("example.com");
    let d2 = shield
        .check_network_request(
            "https://stats.doubleclick.net/pixel.js",
            "https://example.com/",
            "script",
            "GET",
        )
        .unwrap();
    assert!(!d2.blocked);

    // Désactivation globale puis réactivation + export fichier extension
    shield.set_enabled(false);
    let tmp = tempdir().unwrap();
    let rules_file = tmp.path().join("shield_rules.txt");
    sync_rules_file_for_webkit_extension(&shield, &rules_file).unwrap();
    let disabled_txt = fs::read_to_string(&rules_file).unwrap();
    assert!(disabled_txt.starts_with("! DISABLED"));

    shield.set_enabled(true);
    shield.remove_exception_domain("example.com");
    sync_rules_file_for_webkit_extension(&shield, &rules_file).unwrap();
    let enabled_txt = fs::read_to_string(&rules_file).unwrap();
    assert!(enabled_txt.contains("||doubleclick.net^"));
    assert!(!enabled_txt.contains("@@||example.com^$document"));

    let d3 = shield
        .check_network_request(
            "https://stats.doubleclick.net/pixel.js",
            "https://example.com/",
            "script",
            "GET",
        )
        .unwrap();
    assert!(d3.blocked);
}

#[test]
fn test_profile_purge_and_private_session_auto_cleanup() {
    let tmp = tempdir().unwrap();
    let p = ProfileContext::open_persistent(tmp.path(), "work", "Travail").unwrap();
    p.storage
        .insert_space(&Space {
            id: "sp-1".into(),
            name: "Espace 1".into(),
            color: "#0969da".into(),
            created_at: 100,
        })
        .unwrap();
    p.storage
        .upsert_tab(&TabRecord {
            id: "tab-1".into(),
            space_id: "sp-1".into(),
            group_id: None,
            url: "https://example.org/".into(),
            title: "Example".into(),
            is_pinned: true,
            is_suspended: true,
            is_incognito: false,
            position: 0,
            scroll_y: 0,
            last_active_at: 100,
        })
        .unwrap();
    p.storage
        .insert_history_entry(&orakle_core::HistoryEntry {
            id: None,
            url: "https://example.org/".into(),
            title: "Example".into(),
            visited_at: 100,
        })
        .unwrap();
    fs::write(p.webview_data_dir.join("cache.bin"), b"data").unwrap();
    assert!(p.webview_data_dir.join("cache.bin").exists());

    p.purge_all_local_data().unwrap();
    assert!(!p.webview_data_dir.join("cache.bin").exists());
    assert!(p.webview_data_dir.exists());
    assert!(p.storage.get_history(10).unwrap().is_empty());

    // Vérification nettoyage strict à la destruction d'une session privée
    let ephem_path;
    {
        let priv_sess = PrivateSession::start().unwrap();
        ephem_path = priv_sess.temp_path.clone();
        fs::write(ephem_path.join("cookie.txt"), b"ephemeral").unwrap();
        assert!(ephem_path.exists());
    }
    assert!(!ephem_path.exists());
}

#[test]
fn test_suspension_and_lifecycle_entities_survive_wal_reopen() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("orakle_lifecycle.sqlite");
    {
        let st = StorageEngine::open(&db_path).unwrap();
        st.insert_space(&Space {
            id: "sp-main".into(),
            name: "Principal".into(),
            color: "#1f6feb".into(),
            created_at: 10,
        })
        .unwrap();
        st.upsert_tab_group(&TabGroup {
            id: "grp-1".into(),
            space_id: "sp-main".into(),
            name: "Docs".into(),
            color: "#2da44e".into(),
            collapsed: false,
        })
        .unwrap();
        st.upsert_tab(&TabRecord {
            id: "t-susp".into(),
            space_id: "sp-main".into(),
            group_id: Some("grp-1".into()),
            url: "https://doc.rust-lang.org/std/".into(),
            title: "Rust Std".into(),
            is_pinned: false,
            is_suspended: true,
            is_incognito: false,
            position: 0,
            scroll_y: 420,
            last_active_at: 20,
        })
        .unwrap();
        st.save_session(&SessionRecord {
            id: "sess-1".into(),
            name: "Session Audit".into(),
            space_id: "sp-main".into(),
            snapshot_json: r#"[{"url":"https://doc.rust-lang.org/std/"}]"#.into(),
            created_at: 21,
        })
        .unwrap();
        st.upsert_download(&DownloadRecord {
            id: "dl-1".into(),
            url: "https://example.com/archive.tar.gz".into(),
            filename: "archive.tar.gz".into(),
            save_path: "/tmp/archive.tar.gz".into(),
            status: "Terminé".into(),
            bytes_received: 1024,
            total_bytes: 1024,
            started_at: 22,
        })
        .unwrap();
        st.upsert_note(&NoteEntry {
            id: "note-1".into(),
            title: "Note rapide".into(),
            content: "Contenu vérifié".into(),
            url: Some("https://doc.rust-lang.org/std/".into()),
            updated_at: 23,
        })
        .unwrap();
        st.checkpoint_wal().unwrap();
    }

    let reopened = StorageEngine::open(&db_path).unwrap();
    assert!(reopened.check_integrity().unwrap());
    let tabs = reopened.get_tabs_for_space("sp-main").unwrap();
    assert_eq!(tabs.len(), 1);
    assert!(tabs[0].is_suspended);
    assert_eq!(tabs[0].group_id.as_deref(), Some("grp-1"));
    assert_eq!(reopened.get_sessions().unwrap().len(), 1);
    assert_eq!(reopened.get_downloads().unwrap().len(), 1);
    assert_eq!(reopened.get_notes().unwrap().len(), 1);
}

#[test]
fn test_platform_matrix_honesty_and_wk_compiler_limits() {
    let matrix = platform_capability_matrix();
    assert_eq!(matrix.len(), 4);
    assert!(matrix.iter().any(|m| m.platform.contains("Linux")
        && m.supports_https_subresource_url
        && m.status == "Implémenté et testé sous Linux"));
    assert!(matrix.iter().any(|m| m.platform.contains("Windows")
        && m.status.contains("non compilé ni testé sous Windows")));

    let compiled = compile_to_wk_content_rule_list(&[
        "||doubleclick.net^".into(),
        "example.com##.ad-banner".into(),
    ]);
    assert_eq!(compiled.compiled_count, 1);
    assert_eq!(
        compiled.unsupported_rules,
        vec!["example.com##.ad-banner".to_string()]
    );
    assert!(compiled.json.contains("doubleclick"));
}

#[tokio::test]
async fn test_proxy_shutdown_is_strict_fail_closed() {
    let shield = Arc::new(ShieldEngine::new_with_rules("||blocked.test^\n"));
    let (proxy, rx) = SecureLoopbackProxy::new(ProxyConfig::default(), shield);
    let addr = proxy.start(rx).await.unwrap();

    // Requête avant shutdown bloquée par SSRF/Shield (403)
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(b"GET http://127.0.0.1:80/ HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .await
        .unwrap();
    let mut buf = [0u8; 256];
    let n = s.read(&mut buf).await.unwrap();
    let head = String::from_utf8_lossy(&buf[..n]);
    assert!(head.contains("403"), "Attendu 403 SSRF, reçu: {}", head);

    // Shutdown -> plus de nouvelle connexion acceptée
    proxy.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
    assert!(TcpStream::connect(addr).await.is_err());
}
