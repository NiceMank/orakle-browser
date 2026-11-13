use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use wry::{ProxyConfig as WryProxyConfig, ProxyEndpoint, Rect, WebContext, WebView, WebViewBuilder};

#[cfg(target_os = "linux")]
use gtk::prelude::*;
#[cfg(target_os = "linux")]
use tao::platform::unix::WindowExtUnix;
#[cfg(target_os = "linux")]
use wry::WebViewBuilderExtUnix;

use orakle_core::proxy::{ProxyConfig, SecureLoopbackProxy};
use orakle_core::shield::ShieldEngine;
use orakle_core::storage::{Space, StorageEngine, TabRecord};

const CHROME_HTML: &str = include_str!("chrome.html");
const TOOLBAR_HEIGHT: i32 = 56;

const DEFAULT_RULES: &str = "
||doubleclick.net^
||google-analytics.com^
||pixel.facebook.com^
/blocked-tracker.js
";

#[derive(Debug, Clone)]
enum UserEvent {
    IpcCommand(String),
    VerifyStep(u8),
}

#[derive(Debug, Deserialize)]
struct IpcPayload {
    cmd: String,
    payload: String,
}

#[derive(Debug, Clone)]
struct ProcessTreeRss {
    host_rss_kb: u64,
    web_processes_count: usize,
    web_processes_rss_kb: u64,
    total_rss_kb: u64,
}

/// Mesure réelle de la mémoire RSS (`VmRSS` dans `/proc/<pid>/status`) du processus hôte
/// et de tous les processus enfants WebKit (`WebKitWebProcess` / `WebKitNetworkProcess`).
fn measure_process_tree_rss() -> ProcessTreeRss {
    let my_pid = std::process::id();
    let mut host_rss_kb = 0u64;
    let mut web_processes_count = 0usize;
    let mut web_processes_rss_kb = 0u64;

    if let Ok(entries) = fs::read_dir("/proc") {
        // Collecte des couples (pid, ppid, comm, rss_kb)
        let mut procs: Vec<(u32, u32, String, u64)> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            if let Ok(pid) = name.to_string_lossy().parse::<u32>() {
                let status_path = format!("/proc/{}/status", pid);
                if let Ok(status) = fs::read_to_string(&status_path) {
                    let mut ppid = 0u32;
                    let mut rss = 0u64;
                    let mut comm = String::new();
                    for line in status.lines() {
                        if let Some(rest) = line.strip_prefix("Name:") {
                            comm = rest.trim().to_string();
                        } else if let Some(rest) = line.strip_prefix("PPid:") {
                            ppid = rest.trim().parse().unwrap_or(0);
                        } else if let Some(rest) = line.strip_prefix("VmRSS:") {
                            let num_str = rest.split_whitespace().next().unwrap_or("0");
                            rss = num_str.parse().unwrap_or(0);
                        }
                    }
                    procs.push((pid, ppid, comm, rss));
                }
            }
        }

        // Recherche des descendants de my_pid (profondeur 2 pour WebKit processes)
        let mut tree_pids = vec![my_pid];
        let mut changed = true;
        while changed {
            changed = false;
            for (pid, ppid, _, _) in &procs {
                if tree_pids.contains(ppid) && !tree_pids.contains(pid) {
                    tree_pids.push(*pid);
                    changed = true;
                }
            }
        }

        for (pid, _, comm, rss) in procs {
            if pid == my_pid {
                host_rss_kb = rss;
            } else if tree_pids.contains(&pid) {
                if comm.contains("WebKitWeb") {
                    web_processes_count += 1;
                }
                web_processes_rss_kb += rss;
            }
        }
    }

    ProcessTreeRss {
        host_rss_kb,
        web_processes_count,
        web_processes_rss_kb,
        total_rss_kb: host_rss_kb + web_processes_rss_kb,
    }
}

async fn spawn_local_demo_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 2048];
                    let n = stream.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let path = req.lines().next().unwrap_or("").split_whitespace().nth(1).unwrap_or("/");

                    let (status, ctype, body) = match path {
                        "/left" => (
                            "200 OK",
                            "text/html; charset=utf-8",
                            r#"<!doctype html><html><body style="font-family:sans-serif;background:#0d1117;color:#f0f6fc;padding:32px">
                            <h1>Volet Gauche — Orakle Split View</h1>
                            <p>Rendu isolé via Proxy Loopback Rust.</p>
                            <script src="/blocked-tracker.js"></script>
                            <script>window.__leftAllocated = new Array(250000).fill('orakle-left-dom');</script>
                            </body></html>"#,
                        ),
                        "/right" => (
                            "200 OK",
                            "text/html; charset=utf-8",
                            r#"<!doctype html><html><body style="font-family:sans-serif;background:#161b22;color:#f0f6fc;padding:32px">
                            <h1>Volet Droit — Contexte Éphémère</h1>
                            <p>WebKitWebProcess indépendant suspendable vers SQLite WAL.</p>
                            <script>window.__rightAllocated = new Array(500000).fill('orakle-right-heavy-tab-data');</script>
                            </body></html>"#,
                        ),
                        _ => ("404 Not Found", "text/plain", "Not Found"),
                    };

                    let resp = format!(
                        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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

fn make_rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
    Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(w.max(1), h.max(1)).into(),
    }
}

fn layout_webviews(
    width: i32,
    height: i32,
    split_active: bool,
    chrome_wv: &WebView,
    left_wv: &WebView,
    right_wv: Option<&WebView>,
) {
    let content_h = (height - TOOLBAR_HEIGHT).max(100);
    let _ = chrome_wv.set_bounds(make_rect(0, 0, width, TOOLBAR_HEIGHT));

    if split_active && right_wv.is_some() {
        let half = width / 2;
        let _ = left_wv.set_bounds(make_rect(0, TOOLBAR_HEIGHT, half, content_h));
        if let Some(rw) = right_wv {
            let _ = rw.set_bounds(make_rect(half, TOOLBAR_HEIGHT, width - half, content_h));
        }
    } else {
        let _ = left_wv.set_bounds(make_rect(0, TOOLBAR_HEIGHT, width, content_h));
    }
}

#[allow(clippy::too_many_arguments)]
fn build_content_webview(
    url: &str,
    bounds: Rect,
    proxy_port: u16,
    incognito: bool,
    web_ctx: &mut WebContext,
    #[cfg(target_os = "linux")] fixed_container: &gtk::Fixed,
    #[cfg(not(target_os = "linux"))] window: &tao::window::Window,
) -> WebView {
    let builder = WebViewBuilder::new_with_web_context(web_ctx)
        .with_url(url)
        .with_bounds(bounds)
        .with_incognito(incognito)
        .with_proxy_config(WryProxyConfig::Http(ProxyEndpoint {
            host: "127.0.0.1".to_string(),
            port: proxy_port.to_string(),
        }));

    #[cfg(target_os = "linux")]
    {
        builder.build_gtk(fixed_container).expect("build_gtk content webview")
    }
    #[cfg(not(target_os = "linux"))]
    {
        builder.build_as_child(window).expect("build_as_child content webview")
    }
}

fn main() {
    let verify_mode = std::env::args().any(|a| a == "--verify");

    let rt = Arc::new(tokio::runtime::Runtime::new().expect("tokio runtime"));
    let shield = Arc::new(ShieldEngine::new_with_rules(DEFAULT_RULES));

    let temp_dir = tempfile::tempdir().expect("tempdir");
    let db_path = temp_dir.path().join("orakle_app.sqlite");
    let storage = StorageEngine::open(&db_path).expect("sqlite storage");
    storage
        .insert_space(&Space {
            id: "default".into(),
            name: "Travail".into(),
            color: "#0969da".into(),
            created_at: 1700000000,
        })
        .expect("create default space");

    let (demo_addr, proxy_addr, proxy) = rt.block_on(async {
        let demo = spawn_local_demo_server().await;
        let mut cfg = ProxyConfig::default();
        if verify_mode {
            cfg.allowed_ports = vec![80, 443, 8080, demo.port()];
            cfg.allow_loopback_upstream = true;
        }
        let (proxy, rx) = SecureLoopbackProxy::new(cfg, shield.clone());
        let addr = proxy.start(rx).await.expect("start loopback proxy");
        (demo, addr, proxy)
    });

    let left_initial_url = format!("http://{}/left", demo_addr);
    let right_initial_url = format!("http://{}/right", demo_addr);

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let event_proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Orakle Browser V2 — Hôte Rust + WRY")
        .with_inner_size(LogicalSize::new(1280.0, 800.0))
        .build(&event_loop)
        .expect("create main window");

    #[cfg(target_os = "linux")]
    let fixed_container = {
        let vbox = window.default_vbox().expect("GTK default vbox");
        let fixed = gtk::Fixed::new();
        vbox.pack_start(&fixed, true, true, 0);
        fixed.show_all();
        fixed
    };

    let mut normal_ctx = WebContext::new(Some(temp_dir.path().join("profile-normal")));
    let mut incognito_ctx = WebContext::new(None);

    let ipc_proxy = event_proxy.clone();
    let chrome_builder = WebViewBuilder::new()
        .with_html(CHROME_HTML)
        .with_bounds(make_rect(0, 0, 1280, TOOLBAR_HEIGHT))
        .with_ipc_handler(move |req| {
            let _ = ipc_proxy.send_event(UserEvent::IpcCommand(req.body().to_string()));
        });

    #[cfg(target_os = "linux")]
    let chrome_wv = chrome_builder.build_gtk(&fixed_container).expect("build chrome webview");
    #[cfg(not(target_os = "linux"))]
    let chrome_wv = chrome_builder.build_as_child(&window).expect("build chrome webview");

    let left_wv = build_content_webview(
        &left_initial_url,
        make_rect(0, TOOLBAR_HEIGHT, 640, 800 - TOOLBAR_HEIGHT),
        proxy_addr.port(),
        false,
        &mut normal_ctx,
        #[cfg(target_os = "linux")]
        &fixed_container,
        #[cfg(not(target_os = "linux"))]
        &window,
    );

    let mut right_wv: Option<WebView> = Some(build_content_webview(
        &right_initial_url,
        make_rect(640, TOOLBAR_HEIGHT, 640, 800 - TOOLBAR_HEIGHT),
        proxy_addr.port(),
        true,
        &mut incognito_ctx,
        #[cfg(target_os = "linux")]
        &fixed_container,
        #[cfg(not(target_os = "linux"))]
        &window,
    ));

    let mut split_active = true;
    let mut win_w = 1280i32;
    let mut win_h = 800i32;
    let mut right_url_saved = right_initial_url.clone();

    if verify_mode {
        let verify_proxy = event_proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(2500));
            let _ = verify_proxy.send_event(UserEvent::VerifyStep(1));
            std::thread::sleep(Duration::from_millis(800));
            let _ = verify_proxy.send_event(UserEvent::VerifyStep(2));
            std::thread::sleep(Duration::from_millis(1500));
            let _ = verify_proxy.send_event(UserEvent::VerifyStep(3));
            std::thread::sleep(Duration::from_millis(1500));
            let _ = verify_proxy.send_event(UserEvent::VerifyStep(4));
        });
    }

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                proxy.shutdown();
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(new_size),
                ..
            } => {
                let scale = window.scale_factor();
                let logical = new_size.to_logical::<i32>(scale);
                win_w = logical.width;
                win_h = logical.height;
                layout_webviews(win_w, win_h, split_active, &chrome_wv, &left_wv, right_wv.as_ref());
            }
            Event::UserEvent(UserEvent::IpcCommand(raw)) => {
                if let Ok(msg) = serde_json::from_str::<IpcPayload>(&raw) {
                    match msg.cmd.as_str() {
                        "navigate" => {
                            let target = if msg.payload.starts_with("http") {
                                msg.payload
                            } else {
                                format!("https://{}", msg.payload)
                            };
                            let _ = left_wv.load_url(&target);
                        }
                        "toggle_split" => {
                            split_active = !split_active;
                            if split_active && right_wv.is_none() {
                                right_wv = Some(build_content_webview(
                                    &right_url_saved,
                                    make_rect(win_w / 2, TOOLBAR_HEIGHT, win_w - win_w / 2, win_h - TOOLBAR_HEIGHT),
                                    proxy_addr.port(),
                                    true,
                                    &mut incognito_ctx,
                                    #[cfg(target_os = "linux")]
                                    &fixed_container,
                                    #[cfg(not(target_os = "linux"))]
                                    &window,
                                ));
                            }
                            layout_webviews(win_w, win_h, split_active, &chrome_wv, &left_wv, right_wv.as_ref());
                        }
                        "discard_right" => {
                            if right_wv.is_some() {
                                let _ = storage.upsert_tab(&TabRecord {
                                    id: "tab-right".into(),
                                    space_id: "default".into(),
                                    url: right_url_saved.clone(),
                                    title: "Volet Droit".into(),
                                    is_pinned: false,
                                    is_suspended: true,
                                    position: 1,
                                    scroll_y: 420,
                                    last_active_at: 1700000100,
                                });
                                right_wv = None;
                                split_active = false;
                            } else {
                                if let Ok(tabs) = storage.get_tabs_for_space("default") {
                                    if let Some(tab) = tabs.into_iter().find(|t| t.id == "tab-right") {
                                        right_url_saved = tab.url;
                                    }
                                }
                                split_active = true;
                                right_wv = Some(build_content_webview(
                                    &right_url_saved,
                                    make_rect(win_w / 2, TOOLBAR_HEIGHT, win_w - win_w / 2, win_h - TOOLBAR_HEIGHT),
                                    proxy_addr.port(),
                                    true,
                                    &mut incognito_ctx,
                                    #[cfg(target_os = "linux")]
                                    &fixed_container,
                                    #[cfg(not(target_os = "linux"))]
                                    &window,
                                ));
                            }
                            layout_webviews(win_w, win_h, split_active, &chrome_wv, &left_wv, right_wv.as_ref());
                            let tel = proxy.telemetry_snapshot();
                            let _ = chrome_wv.evaluate_script(&format!(
                                "window.updateOrakleStats({}, {})",
                                tel.blocked_by_shield,
                                right_wv.is_none()
                            ));
                        }
                        _ => {}
                    }
                }
            }
            Event::UserEvent(UserEvent::VerifyStep(step)) => match step {
                1 => {
                    let rss = measure_process_tree_rss();
                    println!(
                        "[VERIFY STEP 1 — 3 WebViews actives (Split 50/50)] host_rss={} KB | web_procs={} | web_rss={} KB | total_rss={} KB ({:.1} MiB)",
                        rss.host_rss_kb,
                        rss.web_processes_count,
                        rss.web_processes_rss_kb,
                        rss.total_rss_kb,
                        rss.total_rss_kb as f64 / 1024.0
                    );
                    window.set_inner_size(LogicalSize::new(1024.0, 700.0));
                }
                2 => {
                    println!("[VERIFY STEP 2 — Redimensionnement Split View 1024x700 & Suspension Volet Droit vers SQLite]");
                    storage
                        .upsert_tab(&TabRecord {
                            id: "tab-right".into(),
                            space_id: "default".into(),
                            url: right_url_saved.clone(),
                            title: "Volet Droit Suspendu".into(),
                            is_pinned: false,
                            is_suspended: true,
                            position: 1,
                            scroll_y: 350,
                            last_active_at: 1700000200,
                        })
                        .expect("save discarded tab");
                    right_wv = None;
                    split_active = false;
                    layout_webviews(win_w, win_h, split_active, &chrome_wv, &left_wv, None);
                }
                3 => {
                    let rss = measure_process_tree_rss();
                    println!(
                        "[VERIFY STEP 3 — Après Tab Discarding (2 WebViews actives)] host_rss={} KB | web_procs={} | web_rss={} KB | total_rss={} KB ({:.1} MiB)",
                        rss.host_rss_kb,
                        rss.web_processes_count,
                        rss.web_processes_rss_kb,
                        rss.total_rss_kb,
                        rss.total_rss_kb as f64 / 1024.0
                    );
                    let tabs = storage.get_tabs_for_space("default").expect("list tabs");
                    let restored = tabs.into_iter().find(|t| t.id == "tab-right").expect("tab persisted");
                    right_url_saved = restored.url;
                    split_active = true;
                    right_wv = Some(build_content_webview(
                        &right_url_saved,
                        make_rect(win_w / 2, TOOLBAR_HEIGHT, win_w - win_w / 2, win_h - TOOLBAR_HEIGHT),
                        proxy_addr.port(),
                        true,
                        &mut incognito_ctx,
                        #[cfg(target_os = "linux")]
                        &fixed_container,
                        #[cfg(not(target_os = "linux"))]
                        &window,
                    ));
                    layout_webviews(win_w, win_h, split_active, &chrome_wv, &left_wv, right_wv.as_ref());
                }
                4 => {
                    let rss = measure_process_tree_rss();
                    let tel = proxy.telemetry_snapshot();
                    println!(
                        "[VERIFY STEP 4 — Après Restauration Split View] host_rss={} KB | web_procs={} | web_rss={} KB | total_rss={} KB ({:.1} MiB)",
                        rss.host_rss_kb,
                        rss.web_processes_count,
                        rss.web_processes_rss_kb,
                        rss.total_rss_kb,
                        rss.total_rss_kb as f64 / 1024.0
                    );
                    println!(
                        "[VERIFY PROXY TELEMETRY] total={} allowed={} blocked_shield={}",
                        tel.total_requests, tel.allowed_requests, tel.blocked_by_shield
                    );
                    proxy.shutdown();
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            },
            _ => {}
        }
    });
}
