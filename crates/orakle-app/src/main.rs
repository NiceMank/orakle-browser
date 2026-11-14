mod bench;
mod metrics;
mod pages;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use wry::{
    NewWindowResponse, PageLoadEvent, ProxyConfig as WryProxyConfig, ProxyEndpoint, Rect,
    WebContext, WebView, WebViewBuilder,
};

#[cfg(target_os = "linux")]
use gtk::prelude::*;
#[cfg(target_os = "linux")]
use tao::platform::unix::WindowExtUnix;
#[cfg(target_os = "linux")]
use webkit2gtk::{
    CacheModel, DownloadExt, PermissionRequestExt, URIRequestExt, URIResponseExt, UserMessageExt,
    WebContextExt, WebViewExt as WebKitWebViewExt,
};
#[cfg(target_os = "linux")]
use wry::{WebViewBuilderExtUnix, WebViewExtUnix};

use orakle_core::{
    classify_filename_risk, download_resumable_to_path, platform_capability_matrix,
    resolve_safe_destination, sanitize_filename, sync_rules_file_for_webkit_extension,
    validate_navigation_with_origin, BookmarkEntry, DomainPermission, DownloadControl,
    DownloadRecord, DownloadSafetyRisk, HistoryEntry, NavigationOrigin, NetworkPulse, NoteEntry,
    PrivateSession, ProfileContext, ProfileRegistry, ProxyConfig, SecureLoopbackProxy,
    SessionRecord, ShieldEngine, Space, TabRecord, DL_STATE_CANCELLED, DL_STATE_COMPLETED,
    DL_STATE_ERROR, DL_STATE_PAUSED,
};
use pages::{
    build_omnibox_suggestions, classify_webkit_error, normalize_search_engine,
    render_error_page_html, render_start_page_html, resolve_omnibox_input, search_engine_label,
    ErrorPageKind,
};

const CHROME_HTML: &str = include_str!("chrome.html");
const PANEL_HTML: &str = include_str!("panel.html");

const SPLIT_BAR_HEIGHT: i32 = 28;
const PROXY_BANNER_HEIGHT: i32 = 36;
const PANEL_WIDTH: i32 = 360;

const DEFAULT_RULES: &str = "
||doubleclick.net^
||googleads.g.doubleclick.net^
||google-analytics.com^
||pixel.facebook.com^
||criteo.com^
/blocked-tracker.js
/ads/*
@@||doc.rust-lang.org^$document
";

#[derive(Debug, Clone)]
enum UserEvent {
    IpcCommand(String),
    NewTabFromWeb(String),
    WebTitleChanged {
        tab_id: String,
        title: String,
    },
    WebUriChanged {
        tab_id: String,
        uri: String,
    },
    WebLoadingChanged {
        tab_id: String,
        loading: bool,
    },
    WebAudioChanged {
        tab_id: String,
        playing: bool,
    },
    NativeAdapterBlocked {
        url: String,
        resource_type: String,
        rule: Option<String>,
    },
    WebPermissionDenied {
        origin: String,
        permission: String,
    },
    DownloadStarted {
        id: String,
        url: String,
        filename: String,
        save_path: String,
    },
    DownloadCompleted {
        url: String,
        success: bool,
    },
    DownloadProgress {
        id: String,
        url: String,
        filename: String,
        save_path: String,
        received_bytes: u64,
        total_bytes: u64,
        speed_bps: u64,
        status: String,
        error_message: Option<String>,
    },
    PulseCompleted(PulseViewData),
    SyncScrollApply {
        tab_id: String,
        y: f64,
    },
    VerifyStep(u8),
    VerifyPhase5Step(u8),
    VerifyUxStep(u8),
    HostResized(i32, i32),
}

#[derive(Debug, Clone, Serialize)]
struct LiveDownloadMeta {
    resumable: bool,
    speed_bps: u64,
    exec_warning: bool,
    error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PulseViewData {
    url: String,
    samples_requested: usize,
    samples_succeeded: usize,
    dns_p50: f64,
    dns_p95: f64,
    tcp_p50: f64,
    tcp_p95: f64,
    tls_p50: f64,
    tls_p95: f64,
    ttfb_p50: f64,
    ttfb_p95: f64,
}

#[derive(Debug, Deserialize)]
struct IpcMessage {
    cmd: String,
    payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
struct TabViewState {
    id: String,
    space_id: String,
    url: String,
    title: String,
    is_pinned: bool,
    is_suspended: bool,
    is_incognito: bool,
    is_loading: bool,
    is_playing_audio: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ChromeStateSnapshot {
    tabs: Vec<TabViewState>,
    spaces: Vec<Space>,
    active_space_id: String,
    left_tab_id: Option<String>,
    right_tab_id: Option<String>,
    focused_pane: String,
    split_active: bool,
    split_ratio: f64,
    sync_scroll: bool,
    panel_open: bool,
    panel_section: String,
    theme: String,
    accent: String,
    density: String,
    shield_enabled: bool,
    shield_blocked: u64,
    proxy_running: bool,
    can_go_back: bool,
    can_go_forward: bool,
    closed_count: usize,
    downloads_count: usize,
    profile_name: String,
    toast: Option<String>,
    search_engine: String,
    needs_search_onboarding: bool,
    tips_on_startup: bool,
}

struct TabItem {
    id: String,
    space_id: String,
    group_id: Option<String>,
    url: String,
    title: String,
    is_pinned: bool,
    is_suspended: bool,
    is_incognito: bool,
    is_loading: bool,
    is_playing_audio: bool,
    wv: Option<WebView>,
}

#[derive(Clone, Serialize, Deserialize)]
struct ClosedTab {
    space_id: String,
    url: String,
    title: String,
    is_incognito: bool,
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn make_rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
    Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(w.max(1), h.max(1)).into(),
    }
}

fn find_webkit_extension_dir(work_root: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("liborakle_webkit_ext.so"));
        }
    }
    candidates.push(PathBuf::from("target/release/liborakle_webkit_ext.so"));
    candidates.push(PathBuf::from("target/debug/liborakle_webkit_ext.so"));

    for so_path in candidates {
        if so_path.exists() {
            let ext_dir = work_root.join("webkit_extensions");
            let _ = fs::create_dir_all(&ext_dir);
            let target = ext_dir.join("liborakle_webkit_ext.so");
            if fs::copy(&so_path, &target).is_ok() {
                return Some(ext_dir);
            }
        }
    }
    None
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
                    let path = req
                        .lines()
                        .next()
                        .unwrap_or("")
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or("/");

                    let (status, ctype, body) = match path {
                        "/" | "/home" => (
                            "200 OK",
                            "text/html; charset=utf-8",
                            r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Accueil Orakle V2</title>
                            <style>body{font-family:-apple-system,sans-serif;background:#0d1117;color:#f0f6fc;max-width:760px;margin:48px auto;padding:0 24px;line-height:1.6}
                            .card{background:#161b22;border:1px solid rgba(240,246,252,.1);border-radius:10px;padding:18px;margin-top:16px}
                            a{color:#58a6ff;text-decoration:none;margin-right:16px}code{font-family:monospace;background:#1c222b;padding:2px 6px;border-radius:4px}</style></head>
                            <body><h1>Orakle Browser V2</h1><p>Navigateur natif indépendant piloté en Rust + WRY.</p>
                            <div class="card"><strong>Test Orakle Shield (Proxy Niveau 1 + WebKitWebExtension Niveau 2)</strong>
                            <p style="color:#8b949e;font-size:13px;">Cette page tente de charger <code>/blocked-tracker.js</code> : bloqué avant émission réseau.</p>
                            <script src="/blocked-tracker.js"></script><script src="/ads/banner.js"></script>
                            <p><a href="/left">Ouvrir Volet Gauche</a><a href="/right">Ouvrir Volet Droit</a><a href="https://doc.rust-lang.org/">Documentation Rust (HTTPS)</a></p></div></body></html>"#,
                        ),
                        "/left" => (
                            "200 OK",
                            "text/html; charset=utf-8",
                            r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Volet Gauche — Documentation</title></head>
                            <body style="font-family:sans-serif;background:#0d1117;color:#f0f6fc;padding:32px;line-height:1.6">
                            <h1>Volet Gauche — Orakle Split View</h1>
                            <p>WebView principale connectée au proxy Loopback Rust et à l'extension WebKitWebProcess.</p>
                            <script src="/blocked-tracker.js"></script>
                            <script>window.__leftAlloc = new Array(200000).fill('orakle-left');</script>
                            </body></html>"#,
                        ),
                        "/right" => (
                            "200 OK",
                            "text/html; charset=utf-8",
                            r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Volet Droit — Contexte Privé</title></head>
                            <body style="font-family:sans-serif;background:#161b22;color:#f0f6fc;padding:32px;line-height:1.6">
                            <h1>Volet Droit — Split View Indépendant</h1>
                            <p>Processus WebKitWebProcess distinct isolé (suspension SQLite WAL + restauration complète).</p>
                            <script src="/ads/popup.js"></script>
                            <script>window.__rightAlloc = new Array(400000).fill('orakle-right');</script>
                            </body></html>"#,
                        ),
                        "/page2" => (
                            "200 OK",
                            "text/html; charset=utf-8",
                            r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Page 2 — Navigation Locale</title></head>
                            <body style="font-family:sans-serif;background:#0d1117;color:#f0f6fc;padding:32px;">
                            <h1>Seconde Page Orakle</h1><p>Navigation Précédent / Suivant vérifiée.</p></body></html>"#,
                        ),
                        "/redirect-ssrf" => {
                            let resp = "HTTP/1.1 302 Found\r\nLocation: http://169.254.169.254/latest/meta-data\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                            let _ = stream.write_all(resp.as_bytes()).await;
                            return;
                        }
                        "/download-test.bin" => {
                            let total = 262_144usize;
                            let payload: Vec<u8> = (0..total).map(|i| (i % 251) as u8).collect();
                            let range_start = req
                                .lines()
                                .find(|l| l.to_ascii_lowercase().starts_with("range: bytes="))
                                .and_then(|l| l.split('=').nth(1))
                                .and_then(|r| r.split('-').next())
                                .and_then(|n| n.trim().parse::<usize>().ok())
                                .unwrap_or(0)
                                .min(total);
                            let slice = &payload[range_start..];
                            let hdr = if range_start > 0 {
                                format!(
                                    "HTTP/1.1 206 Partial Content\r\nContent-Type: application/octet-stream\r\nAccept-Ranges: bytes\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    range_start,
                                    total.saturating_sub(1),
                                    total,
                                    slice.len()
                                )
                            } else {
                                format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nAccept-Ranges: bytes\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    total
                                )
                            };
                            let _ = stream.write_all(hdr.as_bytes()).await;
                            let _ = stream.write_all(slice).await;
                            return;
                        }
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

fn resolve_effective_theme(pref: &str) -> String {
    if pref == "light" {
        return "light".to_string();
    }
    if pref == "system" {
        if let Ok(out) = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
            if s.contains("default") && !s.contains("prefer-dark") {
                return "light".to_string();
            }
        }
    }
    "dark".to_string()
}

fn pick_adjacent_tab_id(remaining_space_ids: &[String], removed_pos: usize) -> Option<String> {
    if remaining_space_ids.is_empty() {
        return None;
    }
    let idx = removed_pos.min(remaining_space_ids.len() - 1);
    Some(remaining_space_ids[idx].clone())
}

fn origin_key_from_url(raw: &str) -> Option<String> {
    let u = url::Url::parse(raw).ok()?;
    let host = u.host_str()?;
    let port = u.port_or_known_default()?;
    Some(format!(
        "{}://{}:{}",
        u.scheme(),
        host.to_ascii_lowercase(),
        port
    ))
}

struct BrowserHost {
    rt: Arc<tokio::runtime::Runtime>,
    registry: ProfileRegistry,
    profile: ProfileContext,
    private_session: Option<PrivateSession>,
    shield: Arc<ShieldEngine>,
    proxy: Arc<SecureLoopbackProxy>,
    proxy_cfg: ProxyConfig,
    proxy_port: u16,
    proxy_running: bool,
    proxy_running_flag: Arc<AtomicBool>,
    rules_file_path: PathBuf,
    ext_dir_opt: Option<PathBuf>,
    download_dir: PathBuf,
    allow_loopback_nav: bool,
    demo_base_url: Option<String>,
    window: tao::window::Window,
    #[cfg(target_os = "linux")]
    scrolled_container: gtk::ScrolledWindow,
    #[cfg(target_os = "linux")]
    fixed_container: gtk::Fixed,
    event_proxy: EventLoopProxy<UserEvent>,
    normal_ctx: WebContext,
    chrome_wv: WebView,
    panel_wv: WebView,
    tabs: Vec<TabItem>,
    closed_tabs: Vec<ClosedTab>,
    spaces: Vec<Space>,
    active_space_id: String,
    left_tab_id: Option<String>,
    right_tab_id: Option<String>,
    focused_pane: String,
    split_active: bool,
    split_ratio: f64,
    sync_scroll: bool,
    panel_open: bool,
    panel_section: String,
    chrome_overlay_h: i32,
    theme_pref: String,
    accent: String,
    density: String,
    toast: Option<String>,
    search_engine: String,
    needs_search_onboarding: bool,
    tips_on_startup: bool,
    last_pulse: Option<PulseViewData>,
    live_downloads: HashMap<String, LiveDownloadMeta>,
    resumable_controls: HashMap<String, Arc<DownloadControl>>,
    #[cfg(target_os = "linux")]
    webkit_downloads: Rc<RefCell<HashMap<String, webkit2gtk::Download>>>,
    download_ctx_hooked: HashSet<usize>,
    authorized_localhost_origins: Arc<Mutex<HashMap<String, HashSet<String>>>>,
    win_w: i32,
    win_h: i32,
    next_id: usize,
}

impl BrowserHost {
    fn gen_id(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{}-{}-{}", prefix, now_ts(), self.next_id)
    }

    fn set_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some(msg.into());
    }

    fn focused_tab_id(&self) -> Option<String> {
        if self.split_active && self.focused_pane == "right" {
            self.right_tab_id.clone()
        } else {
            self.left_tab_id.clone()
        }
    }

    fn content_area(&self) -> (i32, i32, i32) {
        let base_h = if self.density == "compact" { 74 } else { 86 };
        let top_h = base_h
            + if self.split_active {
                SPLIT_BAR_HEIGHT
            } else {
                0
            }
            + if !self.proxy_running {
                PROXY_BANNER_HEIGHT
            } else {
                0
            }
            + self.chrome_overlay_h;
        let content_w = (self.win_w - if self.panel_open { PANEL_WIDTH } else { 0 }).max(240);
        let content_h = (self.win_h - top_h).max(160);
        (top_h, content_w, content_h)
    }

    fn position_webview(&self, wv: &WebView, x: i32, y: i32, w: i32, h: i32) {
        let w = w.max(1);
        let h = h.max(1);
        #[cfg(target_os = "linux")]
        {
            let g = wv.webview();
            g.set_size_request(w, h);
            self.fixed_container.move_(&g, x, y);
        }
        let _ = wv.set_bounds(make_rect(x, y, w, h));
    }

    fn relayout(&self) {
        let (top_h, content_w, content_h) = self.content_area();
        self.position_webview(&self.chrome_wv, 0, 0, self.win_w, top_h);

        if self.panel_open {
            let _ = self.panel_wv.set_visible(true);
            self.position_webview(&self.panel_wv, content_w, top_h, PANEL_WIDTH, content_h);
        } else {
            let _ = self.panel_wv.set_visible(false);
        }

        let left_w = if self.split_active && self.right_tab_id.is_some() {
            ((content_w as f64) * self.split_ratio.clamp(0.20, 0.80)).round() as i32
        } else {
            content_w
        };
        let right_w = (content_w - left_w).max(120);

        for tab in &self.tabs {
            let Some(wv) = &tab.wv else { continue };
            if Some(&tab.id) == self.left_tab_id.as_ref() {
                self.position_webview(wv, 0, top_h, left_w, content_h);
                let _ = wv.set_visible(true);
            } else if self.split_active && Some(&tab.id) == self.right_tab_id.as_ref() {
                self.position_webview(wv, left_w, top_h, right_w, content_h);
                let _ = wv.set_visible(true);
            } else {
                let _ = wv.set_visible(false);
            }
        }
    }

    fn authorize_user_localhost(&self, tab_id: &str, url: &str) {
        if let Some(key) = origin_key_from_url(url) {
            if let Ok(u) = url::Url::parse(url) {
                if let Some(host) = u.host_str() {
                    if orakle_core::is_host_loopback(host) {
                        if let Some(port) = u.port_or_known_default() {
                            self.proxy.authorize_user_localhost_port(port);
                        }
                        if let Ok(mut map) = self.authorized_localhost_origins.lock() {
                            map.entry(tab_id.to_string()).or_default().insert(key);
                        }
                    }
                }
            }
        }
    }

    fn shutdown_proxy_fail_closed(&mut self) {
        self.proxy.shutdown();
        self.proxy_running = false;
        self.proxy_running_flag.store(false, Ordering::SeqCst);
        self.set_toast("Proxy arrêté — navigation fail-closed");
        self.relayout();
        self.sync_ui();
    }

    fn restart_proxy_fail_closed(&mut self) {
        self.proxy.shutdown();
        let mut cfg = self.proxy_cfg.clone();
        cfg.bind_addr = SocketAddr::from(([127, 0, 0, 1], self.proxy_port));
        let (new_proxy, rx) = SecureLoopbackProxy::new(cfg, self.shield.clone());
        let res = self.rt.block_on(async { new_proxy.start(rx).await });
        match res {
            Ok(addr) => {
                self.proxy_port = addr.port();
                self.proxy = Arc::new(new_proxy);
                self.proxy_running = true;
                self.proxy_running_flag.store(true, Ordering::SeqCst);
                self.set_toast("Proxy Orakle redémarré");
            }
            Err(_) => {
                self.proxy_running = false;
                self.proxy_running_flag.store(false, Ordering::SeqCst);
                self.set_toast("Échec redémarrage proxy");
            }
        }
        self.relayout();
        self.sync_ui();
    }

    fn create_webview_for_tab(&mut self, tab_id: &str, url: &str, incognito: bool) -> WebView {
        let (top_h, content_w, content_h) = self.content_area();
        let bounds = make_rect(0, top_h, content_w, content_h);
        let allow_loopback = self.allow_loopback_nav;
        let auth_origins = self.authorized_localhost_origins.clone();
        let tab_id_nav = tab_id.to_string();

        let ev_load = self.event_proxy.clone();
        let id_load = tab_id.to_string();
        let ev_title = self.event_proxy.clone();
        let id_title = tab_id.to_string();
        let ev_new = self.event_proxy.clone();
        let ev_dl_start = self.event_proxy.clone();
        let ev_dl_done = self.event_proxy.clone();
        let dl_dir = self.download_dir.clone();

        let initial_url = if url == "about:home" {
            "about:blank"
        } else {
            url
        };

        // SÉCURITÉ : Aucune WebView distante ne reçoit `with_ipc_handler`.
        let mut builder = WebViewBuilder::new_with_web_context(&mut self.normal_ctx)
            .with_url(initial_url)
            .with_bounds(bounds)
            .with_incognito(incognito)
            .with_proxy_config(WryProxyConfig::Http(ProxyEndpoint {
                host: "127.0.0.1".into(),
                port: self.proxy_port.to_string(),
            }))
            .with_navigation_handler(move |target_url| {
                if validate_navigation_with_origin(&target_url, NavigationOrigin::WebContent)
                    .is_ok()
                {
                    return true;
                }
                if allow_loopback
                    && validate_navigation_with_origin(&target_url, NavigationOrigin::UserOmnibox)
                        .is_ok()
                {
                    return true;
                }
                if validate_navigation_with_origin(&target_url, NavigationOrigin::UserOmnibox)
                    .is_ok()
                {
                    if let Some(k) = origin_key_from_url(&target_url) {
                        if let Ok(guard) = auth_origins.lock() {
                            if guard
                                .get(&tab_id_nav)
                                .map(|set| set.contains(&k))
                                .unwrap_or(false)
                            {
                                return true;
                            }
                        }
                    }
                }
                false
            })
            .with_new_window_req_handler(move |target_url, _features| {
                if validate_navigation_with_origin(&target_url, NavigationOrigin::WebContent)
                    .is_ok()
                    || (allow_loopback
                        && validate_navigation_with_origin(
                            &target_url,
                            NavigationOrigin::UserOmnibox,
                        )
                        .is_ok())
                {
                    let _ = ev_new.send_event(UserEvent::NewTabFromWeb(target_url));
                }
                NewWindowResponse::Deny
            })
            .with_on_page_load_handler(move |evt, loaded_url| {
                let loading = matches!(evt, PageLoadEvent::Started);
                let _ = ev_load.send_event(UserEvent::WebLoadingChanged {
                    tab_id: id_load.clone(),
                    loading,
                });
                let _ = ev_load.send_event(UserEvent::WebUriChanged {
                    tab_id: id_load.clone(),
                    uri: loaded_url,
                });
            })
            .with_document_title_changed_handler(move |title| {
                let _ = ev_title.send_event(UserEvent::WebTitleChanged {
                    tab_id: id_title.clone(),
                    title,
                });
            })
            .with_download_started_handler(move |dl_url, save_path| {
                if validate_navigation_with_origin(&dl_url, NavigationOrigin::UserOmnibox).is_err()
                {
                    return false;
                }
                let raw_name = url::Url::parse(&dl_url)
                    .ok()
                    .and_then(|u| {
                        u.path_segments()
                            .and_then(|mut s| s.next_back())
                            .map(ToString::to_string)
                    })
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "download.bin".into());
                let safe_name = sanitize_filename(&raw_name);
                let dest = resolve_safe_destination(&dl_dir, &safe_name)
                    .unwrap_or_else(|_| dl_dir.join("download.bin"));
                let final_name = dest
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&safe_name)
                    .to_string();
                *save_path = dest.clone();
                let _ = ev_dl_start.send_event(UserEvent::DownloadStarted {
                    id: format!("dl-{}", now_ts()),
                    url: dl_url,
                    filename: final_name,
                    save_path: dest.to_string_lossy().to_string(),
                });
                true
            })
            .with_download_completed_handler(move |dl_url, _path, success| {
                let _ = ev_dl_done.send_event(UserEvent::DownloadCompleted {
                    url: dl_url,
                    success,
                });
            });

        #[cfg(target_os = "linux")]
        if let Some(ext_dir) = &self.ext_dir_opt {
            builder = builder.with_extensions_path(ext_dir);
        }

        #[cfg(target_os = "linux")]
        let wv = builder
            .build_gtk(&self.fixed_container)
            .expect("build_gtk tab webview");
        #[cfg(not(target_os = "linux"))]
        let wv = builder
            .build_as_child(&self.window)
            .expect("build_as_child tab webview");

        if url == "about:home" {
            let eff_theme = resolve_effective_theme(&self.theme_pref);
            let html = render_start_page_html(
                &self.profile.record.name,
                self.proxy.telemetry_snapshot().blocked_by_shield,
                &eff_theme,
                &self.search_engine,
                self.tips_on_startup,
            );
            let _ = wv.load_html(&html);
        }

        #[cfg(target_os = "linux")]
        {
            let gtk_wv = wv.webview();
            let ev_audio = self.event_proxy.clone();
            let id_audio = tab_id.to_string();
            gtk_wv.connect_is_playing_audio_notify(move |view| {
                let _ = ev_audio.send_event(UserEvent::WebAudioChanged {
                    tab_id: id_audio.clone(),
                    playing: view.is_playing_audio(),
                });
            });

            // Pages d'erreur lisibles (zéro erreur technique brute)
            let proxy_fail_check = self.proxy_running_flag.clone();
            gtk_wv.connect_load_failed(move |view, _load_ev, failing_uri, err| {
                let msg = err.message();
                let lower = msg.to_ascii_lowercase();
                if lower.contains("cancelled") || lower.contains("frame load interrupted") {
                    return false;
                }
                let kind = classify_webkit_error(msg, proxy_fail_check.load(Ordering::SeqCst));
                let html = render_error_page_html(kind, failing_uri);
                let view_clone = view.clone();
                gtk::glib::idle_add_local_once(move || {
                    view_clone.load_html(&html, None);
                });
                true
            });

            gtk_wv.connect_load_failed_with_tls_errors(move |view, failing_uri, _cert, _flags| {
                let html = render_error_page_html(ErrorPageKind::TlsInvalid, failing_uri);
                let view_clone = view.clone();
                gtk::glib::idle_add_local_once(move || {
                    view_clone.load_html(&html, None);
                });
                true
            });

            // Permissions matérielles fail-closed (micro, caméra, géolocalisation, notifications)
            let ev_perm = self.event_proxy.clone();
            let allowed_origins = Arc::new(
                self.profile
                    .storage
                    .get_domain_permissions()
                    .unwrap_or_default(),
            );
            gtk_wv.connect_permission_request(move |view, req| {
                let current_uri = view.uri().map(|g| g.to_string()).unwrap_or_default();
                let origin = origin_key_from_url(&current_uri).unwrap_or_else(|| "inconnu".into());
                let is_allowed = allowed_origins.iter().any(|p| {
                    p.origin == origin && p.permission == "media/geo" && p.decision == "allow"
                });
                if is_allowed {
                    req.allow();
                } else {
                    req.deny();
                    let _ = ev_perm.send_event(UserEvent::WebPermissionDenied {
                        origin,
                        permission: "media/geo".into(),
                    });
                }
                true
            });

            // Progression & annulation téléchargements WebKitGTK (hooké une seule fois par WebContext)
            if let Some(ctx) = gtk_wv.web_context() {
                let ptr_key = ctx.as_ptr() as usize;
                if self.download_ctx_hooked.insert(ptr_key) {
                    let ev_dl_prog = self.event_proxy.clone();
                    let wk_map = self.webkit_downloads.clone();
                    ctx.connect_download_started(move |_ctx, dl| {
                        let url = dl
                            .request()
                            .and_then(|r| r.uri())
                            .map(|g| g.to_string())
                            .unwrap_or_default();
                        let dl_id = format!("wk-{}", url);
                        wk_map.borrow_mut().insert(dl_id.clone(), dl.clone());
                        let last_state = Arc::new(Mutex::new((0u64, Instant::now())));
                        let ev_rx = ev_dl_prog.clone();
                        let dl_id_rx = dl_id.clone();
                        let url_rx = url.clone();
                        dl.connect_received_data(move |d, _delta| {
                            let rx_bytes = d.received_data_length();
                            let total = d.response().map(|r| r.content_length()).unwrap_or(0);
                            let mut speed = 0u64;
                            if let Ok(mut st) = last_state.lock() {
                                let dt = st.1.elapsed().as_secs_f64();
                                if dt >= 0.25 {
                                    speed = ((rx_bytes.saturating_sub(st.0)) as f64 / dt) as u64;
                                    *st = (rx_bytes, Instant::now());
                                    let dest =
                                        d.destination().map(|g| g.to_string()).unwrap_or_default();
                                    let fname = Path::new(&dest)
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("download.bin")
                                        .to_string();
                                    let _ = ev_rx.send_event(UserEvent::DownloadProgress {
                                        id: dl_id_rx.clone(),
                                        url: url_rx.clone(),
                                        filename: fname,
                                        save_path: dest,
                                        received_bytes: rx_bytes,
                                        total_bytes: total,
                                        speed_bps: speed,
                                        status: "En cours".into(),
                                        error_message: None,
                                    });
                                }
                            }
                            let _ = speed;
                        });

                        let ev_fail = ev_dl_prog.clone();
                        let url_fail = url.clone();
                        let dl_id_fail = dl_id.clone();
                        dl.connect_failed(move |d, _err| {
                            let dest = d.destination().map(|g| g.to_string()).unwrap_or_default();
                            let fname = Path::new(&dest)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("download.bin")
                                .to_string();
                            let _ = ev_fail.send_event(UserEvent::DownloadProgress {
                                id: dl_id_fail.clone(),
                                url: url_fail.clone(),
                                filename: fname,
                                save_path: dest,
                                received_bytes: d.received_data_length(),
                                total_bytes: d.response().map(|r| r.content_length()).unwrap_or(0),
                                speed_bps: 0,
                                status: "Échec".into(),
                                error_message: Some(
                                    "Téléchargement interrompu (réseau, proxy ou annulation)."
                                        .into(),
                                ),
                            });
                        });
                    });
                }
            }

            // Réception des blocages HTTPS Niveau 2 envoyés par liborakle_webkit_ext.so
            let ev_ext = self.event_proxy.clone();
            gtk_wv.connect_user_message_received(move |_view, msg| {
                if msg.name().as_deref() == Some("orakle-shield-blocked") {
                    if let Some(var) = msg.parameters() {
                        if let Some(payload) = var.str() {
                            let mut parts = payload.split('\t');
                            let u = parts.next().unwrap_or(payload).to_string();
                            let rtype = parts.next().unwrap_or("other").to_string();
                            let rule = parts
                                .next()
                                .filter(|s| !s.is_empty())
                                .map(ToString::to_string);
                            let _ = ev_ext.send_event(UserEvent::NativeAdapterBlocked {
                                url: u,
                                resource_type: rtype,
                                rule,
                            });
                        }
                    }
                    return true;
                }
                false
            });
        }

        wv
    }

    fn persist_tab(&self, tab: &TabItem, position: i32) {
        if tab.is_incognito {
            return;
        }
        let _ = self.profile.storage.upsert_tab(&TabRecord {
            id: tab.id.clone(),
            space_id: tab.space_id.clone(),
            group_id: tab.group_id.clone(),
            url: tab.url.clone(),
            title: tab.title.clone(),
            is_pinned: tab.is_pinned,
            is_suspended: tab.is_suspended,
            is_incognito: false,
            position,
            scroll_y: 0,
            last_active_at: now_ts(),
        });
    }

    fn open_new_tab(&mut self, url: &str, incognito: bool, make_active_left: bool) -> String {
        let id = self.gen_id("tab");
        if incognito && self.private_session.is_none() {
            self.private_session = PrivateSession::start().ok();
        }
        let wv = self.create_webview_for_tab(&id, url, incognito);
        let tab = TabItem {
            id: id.clone(),
            space_id: self.active_space_id.clone(),
            group_id: None,
            url: url.to_string(),
            title: if incognito {
                "Navigation Privée".into()
            } else {
                "Nouvel onglet".into()
            },
            is_pinned: false,
            is_suspended: false,
            is_incognito: incognito,
            is_loading: true,
            is_playing_audio: false,
            wv: Some(wv),
        };
        let pos = self.tabs.len() as i32;
        self.persist_tab(&tab, pos);
        self.tabs.push(tab);

        if make_active_left {
            self.left_tab_id = Some(id.clone());
        }
        self.relayout();
        self.sync_ui();
        id
    }

    fn suspend_tab(&mut self, tab_id: &str) {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == tab_id) {
            if let Some(wv) = self.tabs[pos].wv.take() {
                #[cfg(target_os = "linux")]
                {
                    let gwv = wv.webview();
                    gwv.terminate_web_process();
                    self.fixed_container.remove(&gwv);
                }
                drop(wv);
            }
            self.tabs[pos].is_suspended = true;
            self.tabs[pos].is_loading = false;
            let t = TabItem {
                id: self.tabs[pos].id.clone(),
                space_id: self.tabs[pos].space_id.clone(),
                group_id: self.tabs[pos].group_id.clone(),
                url: self.tabs[pos].url.clone(),
                title: self.tabs[pos].title.clone(),
                is_pinned: self.tabs[pos].is_pinned,
                is_suspended: true,
                is_incognito: self.tabs[pos].is_incognito,
                is_loading: false,
                is_playing_audio: false,
                wv: None,
            };
            self.persist_tab(&t, pos as i32);
        }
        self.relayout();
        self.sync_ui();
    }

    fn restore_tab(&mut self, tab_id: &str) {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == tab_id) {
            if self.tabs[pos].wv.is_none() {
                let url = self.tabs[pos].url.clone();
                let incog = self.tabs[pos].is_incognito;
                if validate_navigation_with_origin(&url, NavigationOrigin::UserOmnibox).is_err() {
                    let wv = self.create_webview_for_tab(tab_id, "about:blank", incog);
                    let html = render_error_page_html(ErrorPageKind::RestoreUnavailable, &url);
                    let _ = wv.load_html(&html);
                    self.tabs[pos].wv = Some(wv);
                    self.tabs[pos].is_suspended = false;
                    self.tabs[pos].is_loading = false;
                    self.set_toast("Restauration bloquée par politique de sécurité");
                } else {
                    let wv = self.create_webview_for_tab(tab_id, &url, incog);
                    self.tabs[pos].wv = Some(wv);
                    self.tabs[pos].is_suspended = false;
                    self.tabs[pos].is_loading = true;
                }
            }
        }
        self.relayout();
        self.sync_ui();
    }

    fn select_tab(&mut self, tab_id: &str) {
        let needs_restore = self
            .tabs
            .iter()
            .find(|t| t.id == tab_id)
            .map(|t| t.is_suspended || t.wv.is_none())
            .unwrap_or(false);
        if needs_restore {
            self.restore_tab(tab_id);
        }
        if self.split_active && self.focused_pane == "right" {
            self.right_tab_id = Some(tab_id.to_string());
        } else {
            self.left_tab_id = Some(tab_id.to_string());
        }
        self.relayout();
        self.sync_ui();
    }

    fn default_home_url(&self) -> String {
        self.demo_base_url
            .as_ref()
            .map(|b| format!("{}/home", b))
            .unwrap_or_else(|| "about:home".into())
    }

    fn refresh_home_tabs(&self) {
        let eff_theme = resolve_effective_theme(&self.theme_pref);
        let blocked = self.proxy.telemetry_snapshot().blocked_by_shield;
        let html = render_start_page_html(
            &self.profile.record.name,
            blocked,
            &eff_theme,
            &self.search_engine,
            self.tips_on_startup,
        );
        for tab in &self.tabs {
            if tab.url == "about:home" {
                if let Some(wv) = &tab.wv {
                    let _ = wv.load_html(&html);
                }
            }
        }
    }

    fn close_tab(&mut self, tab_id: &str) {
        let Some(idx) = self.tabs.iter().position(|t| t.id == tab_id) else {
            return;
        };
        let target_space = self.tabs[idx].space_id.clone();
        let removed_space_pos = self
            .tabs
            .iter()
            .filter(|t| t.space_id == target_space)
            .position(|t| t.id == tab_id)
            .unwrap_or(0);

        let mut removed = self.tabs.remove(idx);
        if let Some(wv) = removed.wv.take() {
            #[cfg(target_os = "linux")]
            {
                let gwv = wv.webview();
                gwv.terminate_web_process();
                self.fixed_container.remove(&gwv);
            }
            drop(wv);
        }
        let _ = self.profile.storage.delete_tab(&removed.id);

        let remaining_in_space: Vec<String> = self
            .tabs
            .iter()
            .filter(|t| t.space_id == self.active_space_id)
            .map(|t| t.id.clone())
            .collect();

        if removed.url != "about:home" || !remaining_in_space.is_empty() {
            self.closed_tabs.push(ClosedTab {
                space_id: removed.space_id,
                url: removed.url,
                title: removed.title,
                is_incognito: removed.is_incognito,
            });
        }

        if !self.tabs.iter().any(|t| t.is_incognito) {
            if let Some(priv_sess) = self.private_session.take() {
                let _ = priv_sess.cleanup();
            }
        }

        if remaining_in_space.is_empty() {
            self.split_active = false;
            self.right_tab_id = None;
            self.left_tab_id = None;
            self.focused_pane = "left".into();
            let home_url = self.default_home_url();
            self.open_new_tab(&home_url, false, true);
            return;
        }

        if self.left_tab_id.as_deref() == Some(tab_id) {
            if let Some(next_left) = pick_adjacent_tab_id(&remaining_in_space, removed_space_pos) {
                self.left_tab_id = Some(next_left.clone());
                self.restore_tab(&next_left);
            }
        }
        if self.right_tab_id.as_deref() == Some(tab_id) {
            let next_right = remaining_in_space
                .iter()
                .find(|id| Some((*id).as_str()) != self.left_tab_id.as_deref())
                .cloned();
            if let Some(ref rid) = next_right {
                self.right_tab_id = Some(rid.clone());
                self.restore_tab(rid);
            } else {
                self.right_tab_id = None;
                self.split_active = false;
                self.focused_pane = "left".into();
            }
        }
        self.relayout();
        self.sync_ui();
    }

    fn toggle_split(&mut self) {
        self.split_active = !self.split_active;
        if self.split_active {
            if self.right_tab_id.is_none() || self.right_tab_id == self.left_tab_id {
                let candidate = self
                    .tabs
                    .iter()
                    .find(|t| Some(&t.id) != self.left_tab_id.as_ref() && !t.is_suspended)
                    .map(|t| t.id.clone());
                if let Some(cid) = candidate {
                    self.right_tab_id = Some(cid);
                } else {
                    let fallback = self
                        .demo_base_url
                        .as_ref()
                        .map(|b| format!("{}/right", b))
                        .unwrap_or_else(|| "https://doc.rust-lang.org/".into());
                    let new_id = self.open_new_tab(&fallback, true, false);
                    self.right_tab_id = Some(new_id);
                }
            } else if let Some(rid) = self.right_tab_id.clone() {
                self.restore_tab(&rid);
            }
        }
        self.relayout();
        self.sync_ui();
    }

    fn sync_scroll_between_panes(&self) {
        if !(self.split_active && self.sync_scroll) {
            return;
        }
        let Some(src_id) = self.focused_tab_id() else {
            return;
        };
        let dst_id = if self.focused_pane == "right" {
            self.left_tab_id.clone()
        } else {
            self.right_tab_id.clone()
        };
        let Some(dst_id) = dst_id else { return };
        let src_wv = self
            .tabs
            .iter()
            .find(|t| t.id == src_id)
            .and_then(|t| t.wv.as_ref());
        if let Some(s) = src_wv {
            let ev = self.event_proxy.clone();
            let dst = dst_id.clone();
            let _ = s.evaluate_script_with_callback("String(window.scrollY || 0)", move |val| {
                if let Ok(y) = val.trim_matches('"').parse::<f64>() {
                    let _ = ev.send_event(UserEvent::SyncScrollApply {
                        tab_id: dst.clone(),
                        y,
                    });
                }
            });
        }
    }

    fn start_resumable_download_task(
        &mut self,
        id: String,
        url: String,
        final_path: PathBuf,
        filename: String,
        control: Arc<DownloadControl>,
    ) {
        let proxy_port = self.proxy_port;
        let allow_loop = self.allow_loopback_nav
            || url::Url::parse(&url)
                .ok()
                .and_then(|u| u.port_or_known_default())
                .map(|p| self.proxy.is_dev_port_authorized(p))
                .unwrap_or(false);
        let ev = self.event_proxy.clone();
        let id_prog = id.clone();
        let url_prog = url.clone();
        let fname_prog = filename.clone();
        let save_str = final_path.to_string_lossy().to_string();
        let save_prog = save_str.clone();
        let ctrl_clone = control.clone();
        let ctrl_snap = control.clone();
        let ev_err = ev.clone();
        let last_tick = Arc::new(Mutex::new((0u64, Instant::now())));

        self.rt.spawn(async move {
            let res = download_resumable_to_path(
                &url,
                &final_path,
                Some(proxy_port),
                allow_loop,
                (*ctrl_clone).clone(),
                move |snap| {
                    let mut speed = 0u64;
                    let emit = if snap.state != 0 {
                        true
                    } else if let Ok(mut st) = last_tick.lock() {
                        let dt = st.1.elapsed().as_secs_f64();
                        if dt >= 0.25 {
                            speed = ((snap.bytes_received.saturating_sub(st.0)) as f64 / dt) as u64;
                            *st = (snap.bytes_received, Instant::now());
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if emit {
                        let status = match snap.state {
                            s if s == DL_STATE_PAUSED => "En pause",
                            s if s == DL_STATE_COMPLETED => "Terminé",
                            s if s == DL_STATE_CANCELLED => "Annulé",
                            s if s == DL_STATE_ERROR => "Échec",
                            _ => "En cours",
                        };
                        let _ = ev.send_event(UserEvent::DownloadProgress {
                            id: id_prog.clone(),
                            url: url_prog.clone(),
                            filename: fname_prog.clone(),
                            save_path: save_prog.clone(),
                            received_bytes: snap.bytes_received,
                            total_bytes: snap.total_bytes,
                            speed_bps: speed,
                            status: status.to_string(),
                            error_message: None,
                        });
                    }
                },
            )
            .await;
            if let Err(err) = res {
                let snap = ctrl_snap.snapshot();
                let _ = ev_err.send_event(UserEvent::DownloadProgress {
                    id,
                    url,
                    filename,
                    save_path: save_str,
                    received_bytes: snap.bytes_received,
                    total_bytes: snap.total_bytes,
                    speed_bps: 0,
                    status: "Échec".into(),
                    error_message: Some(err.to_string()),
                });
            }
        });
    }

    fn navigate_tab_to(&mut self, tab_id: &str, raw_input: &str) {
        let (target, dev_port) = resolve_omnibox_input(raw_input, &self.search_engine);
        if validate_navigation_with_origin(&target, NavigationOrigin::UserOmnibox).is_err() {
            if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                t.url = target.clone();
                if let Some(wv) = &t.wv {
                    let html = render_error_page_html(ErrorPageKind::SecurityBlocked, &target);
                    let _ = wv.load_html(&html);
                }
            }
            self.set_toast("URL bloquée par politique de sécurité");
            self.sync_ui();
            return;
        }
        if dev_port.is_some() {
            self.authorize_user_localhost(tab_id, &target);
        }
        if !self.proxy_running {
            if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                t.url = target.clone();
                if let Some(wv) = &t.wv {
                    let html = render_error_page_html(ErrorPageKind::ProxyUnavailable, &target);
                    let _ = wv.load_html(&html);
                }
            }
            self.set_toast("Proxy arrêté — navigation fail-closed");
            self.sync_ui();
            return;
        }
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            t.url = target.clone();
            t.is_loading = true;
            if let Some(wv) = &t.wv {
                let _ = wv.load_url(&target);
            }
        }
        self.sync_ui();
    }

    fn sync_ui(&self) {
        let focused_id = self.focused_tab_id();
        #[allow(unused_mut)]
        let mut can_back = false;
        #[allow(unused_mut)]
        let mut can_fwd = false;

        #[cfg(target_os = "linux")]
        if let Some(fid) = &focused_id {
            if let Some(wv) = self
                .tabs
                .iter()
                .find(|t| t.id == *fid)
                .and_then(|t| t.wv.as_ref())
            {
                let g = wv.webview();
                can_back = g.can_go_back();
                can_fwd = g.can_go_forward();
            }
        }

        let tel = self.proxy.telemetry_snapshot();
        let eff_theme = resolve_effective_theme(&self.theme_pref);
        let downloads_list = self.profile.storage.get_downloads().unwrap_or_default();
        let snapshot = ChromeStateSnapshot {
            tabs: self
                .tabs
                .iter()
                .map(|t| TabViewState {
                    id: t.id.clone(),
                    space_id: t.space_id.clone(),
                    url: t.url.clone(),
                    title: t.title.clone(),
                    is_pinned: t.is_pinned,
                    is_suspended: t.is_suspended,
                    is_incognito: t.is_incognito,
                    is_loading: t.is_loading,
                    is_playing_audio: t.is_playing_audio,
                })
                .collect(),
            spaces: self.spaces.clone(),
            active_space_id: self.active_space_id.clone(),
            left_tab_id: self.left_tab_id.clone(),
            right_tab_id: self.right_tab_id.clone(),
            focused_pane: self.focused_pane.clone(),
            split_active: self.split_active,
            split_ratio: self.split_ratio,
            sync_scroll: self.sync_scroll,
            panel_open: self.panel_open,
            panel_section: self.panel_section.clone(),
            theme: eff_theme.clone(),
            accent: self.accent.clone(),
            density: self.density.clone(),
            shield_enabled: self.shield.is_enabled(),
            shield_blocked: tel.blocked_by_shield,
            proxy_running: self.proxy_running,
            can_go_back: can_back,
            can_go_forward: can_fwd,
            closed_count: self.closed_tabs.len(),
            downloads_count: downloads_list.len(),
            profile_name: self.profile.record.name.clone(),
            toast: self.toast.clone(),
            search_engine: self.search_engine.clone(),
            needs_search_onboarding: self.needs_search_onboarding,
            tips_on_startup: self.tips_on_startup,
        };

        if let Ok(json) = serde_json::to_string(&snapshot) {
            let _ = self
                .chrome_wv
                .evaluate_script(&format!("window.applyOrakleState({})", json));
        }

        if self.panel_open {
            let panel_payload = serde_json::json!({
                "section": self.panel_section,
                "theme": eff_theme,
                "theme_pref": self.theme_pref,
                "accent": self.accent,
                "density": self.density,
                "shield_enabled": self.shield.is_enabled(),
                "proxy_running": self.proxy_running,
                "proxy_addr": format!("127.0.0.1:{}", self.proxy_port),
                "telemetry": tel,
                "exceptions": self.shield.exception_domains(),
                "permissions": self.profile.storage.get_domain_permissions().unwrap_or_default(),
                "adapters": platform_capability_matrix(),
                "bookmarks": self.profile.storage.get_bookmarks().unwrap_or_default(),
                "history": self.profile.storage.get_history(80).unwrap_or_default(),
                "downloads": downloads_list,
                "live_downloads": self.live_downloads,
                "spaces": self.spaces,
                "active_space_id": self.active_space_id,
                "sessions": self.profile.storage.get_sessions().unwrap_or_default(),
                "notes": self.profile.storage.get_notes().unwrap_or_default(),
                "last_pulse": self.last_pulse,
                "search_engine": self.search_engine,
                "tips_on_startup": self.tips_on_startup,
                "profile_id": self.profile.record.id,
                "profile_name": self.profile.record.name,
                "profile_dir": self.profile.root_dir.to_string_lossy(),
                "profiles": self.registry.list_profiles(),
            });
            if let Ok(pjson) = serde_json::to_string(&panel_payload) {
                let _ = self
                    .panel_wv
                    .evaluate_script(&format!("window.applyPanelData({})", pjson));
            }
        }
    }

    fn handle_ipc(&mut self, raw: &str) {
        let Ok(msg) = serde_json::from_str::<IpcMessage>(raw) else {
            return;
        };
        match msg.cmd.as_str() {
            "ui_ready" => {
                self.sync_ui();
            }
            "set_chrome_overlay" => {
                let h = msg
                    .payload
                    .get("height")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                self.chrome_overlay_h = h.clamp(0, 280);
                self.relayout();
            }
            "complete_search_onboarding" => {
                let se = msg
                    .payload
                    .get("engine")
                    .and_then(|v| v.as_str())
                    .map(normalize_search_engine)
                    .unwrap_or("duckduckgo");
                self.search_engine = se.to_string();
                self.needs_search_onboarding = false;
                self.chrome_overlay_h = 0;
                let _ = self
                    .profile
                    .storage
                    .set_setting("search_engine", &self.search_engine);
                let _ = self
                    .profile
                    .storage
                    .set_setting("search_engine_configured", "1");
                self.set_toast(format!(
                    "Moteur : {}",
                    search_engine_label(&self.search_engine)
                ));
                self.refresh_home_tabs();
                self.relayout();
                self.sync_ui();
            }
            "set_tips_on_startup" => {
                let enabled = msg
                    .payload
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                self.tips_on_startup = enabled;
                let _ = self
                    .profile
                    .storage
                    .set_setting("tips_on_startup", if enabled { "1" } else { "0" });
                self.set_toast(if enabled {
                    "Astuces affichées sur l'accueil"
                } else {
                    "Astuces masquées sur l'accueil"
                });
                self.refresh_home_tabs();
                self.sync_ui();
            }
            "open_tips_overlay" => {
                self.panel_open = false;
                self.relayout();
                let _ = self
                    .chrome_wv
                    .evaluate_script("window.openTipsOverlay && window.openTipsOverlay();");
                self.sync_ui();
            }
            "query_omnibox" => {
                let q = msg
                    .payload
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let open_tabs: Vec<(String, String, String, bool)> = self
                    .tabs
                    .iter()
                    .map(|t| (t.id.clone(), t.title.clone(), t.url.clone(), t.is_suspended))
                    .collect();
                let bms = self.profile.storage.get_bookmarks().unwrap_or_default();
                let hist = self.profile.storage.get_history(40).unwrap_or_default();
                let suggs =
                    build_omnibox_suggestions(q, &self.search_engine, &open_tabs, &bms, &hist);
                if let Ok(json) = serde_json::to_string(&suggs) {
                    let _ = self
                        .chrome_wv
                        .evaluate_script(&format!("window.applyOmniboxSuggestions({})", json));
                }
            }
            "navigate" => {
                if let Some(u) = msg.payload.get("url").and_then(|v| v.as_str()) {
                    if let Some(fid) = self.focused_tab_id() {
                        self.navigate_tab_to(&fid, u);
                    }
                }
            }
            "restart_proxy" => {
                self.restart_proxy_fail_closed();
            }
            "duplicate_tab" => {
                if let Some(id) = msg.payload.get("tab_id").and_then(|v| v.as_str()) {
                    if let Some((u, incog)) = self
                        .tabs
                        .iter()
                        .find(|t| t.id == id)
                        .map(|t| (t.url.clone(), t.is_incognito))
                    {
                        self.open_new_tab(&u, incog, true);
                    }
                }
            }
            "nav_back" =>
            {
                #[cfg(target_os = "linux")]
                if let Some(fid) = self.focused_tab_id() {
                    if let Some(wv) = self
                        .tabs
                        .iter()
                        .find(|t| t.id == fid)
                        .and_then(|t| t.wv.as_ref())
                    {
                        wv.webview().go_back();
                    }
                }
            }
            "nav_forward" =>
            {
                #[cfg(target_os = "linux")]
                if let Some(fid) = self.focused_tab_id() {
                    if let Some(wv) = self
                        .tabs
                        .iter()
                        .find(|t| t.id == fid)
                        .and_then(|t| t.wv.as_ref())
                    {
                        wv.webview().go_forward();
                    }
                }
            }
            "nav_reload" => {
                if let Some(fid) = self.focused_tab_id() {
                    if let Some(wv) = self
                        .tabs
                        .iter()
                        .find(|t| t.id == fid)
                        .and_then(|t| t.wv.as_ref())
                    {
                        let _ = wv.reload();
                    }
                }
            }
            "new_tab" => {
                let incog = msg
                    .payload
                    .get("incognito")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let url = self.default_home_url();
                self.open_new_tab(&url, incog, true);
                let _ = self
                    .chrome_wv
                    .evaluate_script("window.focusOmnibox && window.focusOmnibox();");
            }
            "select_tab" => {
                if let Some(id) = msg.payload.get("tab_id").and_then(|v| v.as_str()) {
                    self.select_tab(id);
                }
            }
            "assign_right_tab" => {
                if let Some(id) = msg.payload.get("tab_id").and_then(|v| v.as_str()) {
                    self.restore_tab(id);
                    self.right_tab_id = Some(id.to_string());
                    self.split_active = true;
                    self.relayout();
                    self.sync_ui();
                }
            }
            "close_tab" => {
                if let Some(id) = msg.payload.get("tab_id").and_then(|v| v.as_str()) {
                    self.close_tab(id);
                }
            }
            "reopen_closed" => {
                if let Some(c) = self.closed_tabs.pop() {
                    let _ = self
                        .chrome_wv
                        .evaluate_script("window.closeOverlays && window.closeOverlays(); var oi=document.getElementById('urlInput'); if(oi) oi.blur();");
                    self.open_new_tab(&c.url, c.is_incognito, true);
                }
            }
            "toggle_pin" => {
                if let Some(id) = msg.payload.get("tab_id").and_then(|v| v.as_str()) {
                    if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
                        self.tabs[idx].is_pinned = !self.tabs[idx].is_pinned;
                        let item = self.tabs.remove(idx);
                        if item.is_pinned {
                            self.tabs.insert(0, item);
                        } else {
                            self.tabs.push(item);
                        }
                    }
                    self.sync_ui();
                }
            }
            "reorder_tab" => {
                let src = msg.payload.get("tab_id").and_then(|v| v.as_str());
                let dst = msg.payload.get("target_tab_id").and_then(|v| v.as_str());
                if let (Some(s_id), Some(d_id)) = (src, dst) {
                    if let (Some(si), Some(di)) = (
                        self.tabs.iter().position(|t| t.id == s_id),
                        self.tabs.iter().position(|t| t.id == d_id),
                    ) {
                        let moving = self.tabs.remove(si);
                        self.tabs.insert(di, moving);
                        self.sync_ui();
                    }
                }
            }
            "suspend_tab" => {
                if let Some(id) = msg.payload.get("tab_id").and_then(|v| v.as_str()) {
                    self.suspend_tab(id);
                }
            }
            "restore_tab" => {
                if let Some(id) = msg.payload.get("tab_id").and_then(|v| v.as_str()) {
                    self.restore_tab(id);
                }
            }
            "toggle_split" => self.toggle_split(),
            "focus_pane" => {
                if let Some(p) = msg.payload.get("pane").and_then(|v| v.as_str()) {
                    if p == "left" || p == "right" {
                        self.focused_pane = p.to_string();
                        if let Some(fid) = self.focused_tab_id() {
                            if let Some(wv) = self
                                .tabs
                                .iter()
                                .find(|t| t.id == fid)
                                .and_then(|t| t.wv.as_ref())
                            {
                                let _ = wv.focus();
                            }
                        }
                        self.sync_ui();
                    }
                }
            }
            "set_split_ratio" => {
                if let Some(r) = msg.payload.get("ratio").and_then(|v| v.as_f64()) {
                    self.split_ratio = r.clamp(0.20, 0.80);
                    self.relayout();
                    self.sync_ui();
                }
            }
            "swap_panes" => {
                std::mem::swap(&mut self.left_tab_id, &mut self.right_tab_id);
                self.relayout();
                self.sync_ui();
            }
            "toggle_sync_scroll" => {
                self.sync_scroll = !self.sync_scroll;
                self.sync_scroll_between_panes();
                self.sync_ui();
            }
            "toggle_panel" => {
                self.panel_open = !self.panel_open;
                self.relayout();
                self.sync_ui();
            }
            "open_panel" => {
                if let Some(sec) = msg.payload.get("section").and_then(|v| v.as_str()) {
                    if self.panel_open && self.panel_section == sec {
                        self.panel_open = false;
                    } else {
                        self.panel_open = true;
                        self.panel_section = sec.to_string();
                    }
                    self.relayout();
                    self.sync_ui();
                }
            }
            "toggle_shield" => {
                let next = !self.shield.is_enabled();
                self.shield.set_enabled(next);
                let _ = sync_rules_file_for_webkit_extension(&self.shield, &self.rules_file_path);
                self.sync_ui();
            }
            "add_exception" => {
                if let Some(d) = msg.payload.get("domain").and_then(|v| v.as_str()) {
                    self.shield.add_exception_domain(d);
                    let _ =
                        sync_rules_file_for_webkit_extension(&self.shield, &self.rules_file_path);
                    self.sync_ui();
                }
            }
            "remove_exception" => {
                if let Some(d) = msg.payload.get("domain").and_then(|v| v.as_str()) {
                    self.shield.remove_exception_domain(d);
                    let _ =
                        sync_rules_file_for_webkit_extension(&self.shield, &self.rules_file_path);
                    self.sync_ui();
                }
            }
            "add_bookmark" => {
                if let Some(fid) = self.focused_tab_id() {
                    if let Some(t) = self.tabs.iter().find(|t| t.id == fid) {
                        let _ = self.profile.storage.upsert_bookmark(&BookmarkEntry {
                            id: format!("bm-{}", now_ts()),
                            title: t.title.clone(),
                            url: t.url.clone(),
                            category: "Général".into(),
                            created_at: now_ts(),
                        });
                        self.set_toast("Favori enregistré");
                    }
                    self.sync_ui();
                }
            }
            "delete_permission" => {
                let orig = msg.payload.get("origin").and_then(|v| v.as_str());
                let perm = msg.payload.get("permission").and_then(|v| v.as_str());
                if let (Some(o), Some(p)) = (orig, perm) {
                    let _ = self.profile.storage.delete_domain_permission(o, p);
                    self.set_toast("Permission réinitialisée");
                    self.sync_ui();
                }
            }
            "start_resumable_download" => {
                if let Some(raw_url) = msg.payload.get("url").and_then(|v| v.as_str()) {
                    let (target, dev_port) = resolve_omnibox_input(raw_url, &self.search_engine);
                    if validate_navigation_with_origin(&target, NavigationOrigin::UserOmnibox)
                        .is_ok()
                        && self.proxy_running
                    {
                        if let Some(p) = dev_port {
                            self.proxy.authorize_user_localhost_port(p);
                        }
                        let raw_name = url::Url::parse(&target)
                            .ok()
                            .and_then(|u| {
                                u.path_segments()
                                    .and_then(|mut s| s.next_back())
                                    .map(ToString::to_string)
                            })
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| "download.bin".into());
                        let safe_name = sanitize_filename(&raw_name);
                        if let Ok(dest) = resolve_safe_destination(&self.download_dir, &safe_name) {
                            let final_name = dest
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(&safe_name)
                                .to_string();
                            let dl_id = self.gen_id("dl");
                            let risk = classify_filename_risk(&final_name);
                            let exec_warning =
                                matches!(risk, DownloadSafetyRisk::ExecutableWarning { .. });
                            let _ = self.profile.storage.upsert_download(&DownloadRecord {
                                id: dl_id.clone(),
                                url: target.clone(),
                                filename: final_name.clone(),
                                save_path: dest.to_string_lossy().to_string(),
                                status: "En cours".into(),
                                bytes_received: 0,
                                total_bytes: 0,
                                started_at: now_ts(),
                            });
                            self.live_downloads.insert(
                                dl_id.clone(),
                                LiveDownloadMeta {
                                    resumable: true,
                                    speed_bps: 0,
                                    exec_warning,
                                    error_message: None,
                                },
                            );
                            let ctrl = Arc::new(DownloadControl::new());
                            self.resumable_controls.insert(dl_id.clone(), ctrl.clone());
                            self.start_resumable_download_task(
                                dl_id, target, dest, final_name, ctrl,
                            );
                            self.set_toast("Téléchargement démarré");
                            self.sync_ui();
                        }
                    } else {
                        self.set_toast("URL bloquée ou proxy arrêté");
                        self.sync_ui();
                    }
                }
            }
            "pause_download" => {
                if let Some(id) = msg.payload.get("id").and_then(|v| v.as_str()) {
                    if let Some(ctrl) = self.resumable_controls.get(id) {
                        ctrl.pause();
                        self.set_toast("Téléchargement en pause");
                        self.sync_ui();
                    }
                }
            }
            "resume_download" => {
                if let Some(id) = msg.payload.get("id").and_then(|v| v.as_str()) {
                    if let Ok(list) = self.profile.storage.get_downloads() {
                        if let Some(mut rec) = list.into_iter().find(|d| d.id == id) {
                            rec.status = "En cours".into();
                            let _ = self.profile.storage.upsert_download(&rec);
                            let ctrl = Arc::new(DownloadControl::new());
                            self.resumable_controls.insert(id.to_string(), ctrl.clone());
                            self.start_resumable_download_task(
                                rec.id,
                                rec.url,
                                PathBuf::from(&rec.save_path),
                                rec.filename,
                                ctrl,
                            );
                            self.set_toast("Téléchargement repris");
                            self.sync_ui();
                        }
                    }
                }
            }
            "cancel_download" => {
                if let Some(id) = msg.payload.get("id").and_then(|v| v.as_str()) {
                    if let Some(ctrl) = self.resumable_controls.get(id) {
                        ctrl.cancel();
                    }
                    #[cfg(target_os = "linux")]
                    if let Some(wk_dl) = self.webkit_downloads.borrow().get(id) {
                        wk_dl.cancel();
                    }
                    if let Ok(list) = self.profile.storage.get_downloads() {
                        if let Some(mut rec) = list.into_iter().find(|d| d.id == id) {
                            rec.status = "Annulé".into();
                            let _ = self.profile.storage.upsert_download(&rec);
                        }
                    }
                    self.set_toast("Téléchargement annulé");
                    self.sync_ui();
                }
            }
            "open_download_file" | "open_download_folder" => {
                if let Some(id) = msg.payload.get("id").and_then(|v| v.as_str()) {
                    if let Ok(list) = self.profile.storage.get_downloads() {
                        if let Some(rec) = list.into_iter().find(|d| d.id == id) {
                            let target_path = if msg.cmd == "open_download_folder" {
                                Path::new(&rec.save_path)
                                    .parent()
                                    .unwrap_or(self.download_dir.as_path())
                                    .to_path_buf()
                            } else {
                                PathBuf::from(&rec.save_path)
                            };
                            #[cfg(target_os = "linux")]
                            {
                                let _ = std::process::Command::new("xdg-open")
                                    .arg(&target_path)
                                    .spawn();
                            }
                        }
                    }
                }
            }
            "delete_download" => {
                if let Some(id) = msg.payload.get("id").and_then(|v| v.as_str()) {
                    self.resumable_controls.remove(id);
                    self.live_downloads.remove(id);
                    let _ = self.profile.storage.delete_download(id);
                    self.sync_ui();
                }
            }
            "delete_bookmark" => {
                if let Some(id) = msg.payload.get("id").and_then(|v| v.as_str()) {
                    let _ = self.profile.storage.delete_bookmark(id);
                    self.sync_ui();
                }
            }
            "clear_history" => {
                let _ = self.profile.storage.clear_history();
                self.sync_ui();
            }
            "create_space" => {
                if let Some(name) = msg.payload.get("name").and_then(|v| v.as_str()) {
                    let sp = Space {
                        id: self.gen_id("space"),
                        name: name.to_string(),
                        color: "#10b981".into(),
                        created_at: now_ts(),
                    };
                    let _ = self.profile.storage.insert_space(&sp);
                    self.spaces.push(sp.clone());
                    self.active_space_id = sp.id;
                    let url = self
                        .demo_base_url
                        .as_ref()
                        .map(|b| format!("{}/home", b))
                        .unwrap_or_else(|| "https://doc.rust-lang.org/".into());
                    self.open_new_tab(&url, false, true);
                }
            }
            "select_space" => {
                if let Some(sid) = msg.payload.get("space_id").and_then(|v| v.as_str()) {
                    self.active_space_id = sid.to_string();
                    self.left_tab_id = self
                        .tabs
                        .iter()
                        .find(|t| t.space_id == sid)
                        .map(|t| t.id.clone());
                    self.relayout();
                    self.sync_ui();
                }
            }
            "save_session" => {
                let name = msg
                    .payload
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Session");
                let snap: Vec<TabRecord> = self
                    .tabs
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| !t.is_incognito)
                    .map(|(i, t)| TabRecord {
                        id: t.id.clone(),
                        space_id: t.space_id.clone(),
                        group_id: t.group_id.clone(),
                        url: t.url.clone(),
                        title: t.title.clone(),
                        is_pinned: t.is_pinned,
                        is_suspended: true,
                        is_incognito: false,
                        position: i as i32,
                        scroll_y: 0,
                        last_active_at: now_ts(),
                    })
                    .collect();
                let s = SessionRecord {
                    id: self.gen_id("sess"),
                    name: name.to_string(),
                    space_id: self.active_space_id.clone(),
                    snapshot_json: serde_json::to_string(&snap).unwrap_or_else(|_| "[]".into()),
                    created_at: now_ts(),
                };
                let _ = self.profile.storage.save_session(&s);
                self.sync_ui();
            }
            "restore_session" => {
                if let Some(id) = msg.payload.get("id").and_then(|v| v.as_str()) {
                    if let Ok(sessions) = self.profile.storage.get_sessions() {
                        if let Some(sess) = sessions.into_iter().find(|s| s.id == id) {
                            if let Ok(records) =
                                serde_json::from_str::<Vec<TabRecord>>(&sess.snapshot_json)
                            {
                                for r in records {
                                    self.open_new_tab(&r.url, false, true);
                                }
                            }
                        }
                    }
                }
            }
            "save_note" => {
                let title = msg
                    .payload
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Note");
                let content = msg
                    .payload
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let active_url = self.focused_tab_id().and_then(|fid| {
                    self.tabs
                        .iter()
                        .find(|t| t.id == fid)
                        .map(|t| t.url.clone())
                });
                let note_id = self.gen_id("note");
                let _ = self.profile.storage.upsert_note(&NoteEntry {
                    id: note_id,
                    title: title.to_string(),
                    content: content.to_string(),
                    url: active_url,
                    updated_at: now_ts(),
                });
                self.sync_ui();
            }
            "delete_note" => {
                if let Some(id) = msg.payload.get("id").and_then(|v| v.as_str()) {
                    let _ = self.profile.storage.delete_note(id);
                    self.sync_ui();
                }
            }
            "run_pulse" => {
                let target_url = self
                    .focused_tab_id()
                    .and_then(|fid| {
                        self.tabs
                            .iter()
                            .find(|t| t.id == fid)
                            .map(|t| t.url.clone())
                    })
                    .unwrap_or_else(|| "https://doc.rust-lang.org/".into());
                let ev = self.event_proxy.clone();
                self.rt.spawn(async move {
                    if let Ok(probe) = NetworkPulse::new() {
                        if let Ok(stats) = probe.probe_stats(&target_url, 3).await {
                            let _ = ev.send_event(UserEvent::PulseCompleted(PulseViewData {
                                url: stats.url,
                                samples_requested: stats.samples_requested,
                                samples_succeeded: stats.samples_succeeded,
                                dns_p50: stats.dns.p50_ms,
                                dns_p95: stats.dns.p95_ms,
                                tcp_p50: stats.tcp.p50_ms,
                                tcp_p95: stats.tcp.p95_ms,
                                tls_p50: stats.tls.p50_ms,
                                tls_p95: stats.tls.p95_ms,
                                ttfb_p50: stats.ttfb.p50_ms,
                                ttfb_p95: stats.ttfb.p95_ms,
                            }));
                        }
                    }
                });
            }
            "set_theme" => {
                if let Some(th) = msg.payload.get("theme").and_then(|v| v.as_str()) {
                    self.theme_pref = match th {
                        "light" => "light".into(),
                        "system" => "system".into(),
                        _ => "dark".into(),
                    };
                    let _ = self.profile.storage.set_setting("theme", &self.theme_pref);
                    self.sync_ui();
                }
            }
            "set_accent" => {
                if let Some(acc) = msg.payload.get("accent").and_then(|v| v.as_str()) {
                    if matches!(
                        acc,
                        "#0969da" | "#0f766e" | "#4f46e5" | "#b45309" | "#be123c"
                    ) {
                        self.accent = acc.to_string();
                        let _ = self.profile.storage.set_setting("accent", &self.accent);
                        self.sync_ui();
                    }
                }
            }
            "set_density" => {
                if let Some(d) = msg.payload.get("density").and_then(|v| v.as_str()) {
                    self.density = if d == "compact" {
                        "compact".into()
                    } else {
                        "comfortable".into()
                    };
                    let _ = self.profile.storage.set_setting("density", &self.density);
                    self.relayout();
                    self.sync_ui();
                }
            }
            "set_search_engine" => {
                if let Some(se) = msg.payload.get("engine").and_then(|v| v.as_str()) {
                    self.search_engine = normalize_search_engine(se).to_string();
                    self.needs_search_onboarding = false;
                    let _ = self
                        .profile
                        .storage
                        .set_setting("search_engine", &self.search_engine);
                    let _ = self
                        .profile
                        .storage
                        .set_setting("search_engine_configured", "1");
                    self.set_toast(format!(
                        "Moteur : {}",
                        search_engine_label(&self.search_engine)
                    ));
                    self.refresh_home_tabs();
                    self.sync_ui();
                }
            }
            "create_profile" => {
                if let Some(name) = msg.payload.get("name").and_then(|v| v.as_str()) {
                    if self.registry.create_profile(name).is_ok() {
                        self.set_toast("Nouveau profil créé");
                        self.sync_ui();
                    }
                }
            }
            "set_startup_profile" => {
                if let Some(id) = msg.payload.get("id").and_then(|v| v.as_str()) {
                    if self.registry.set_startup_profile(id).is_ok() {
                        self.set_toast("Profil de démarrage mis à jour");
                        self.sync_ui();
                    }
                }
            }
            "delete_profile" => {
                if let Some(id) = msg.payload.get("id").and_then(|v| v.as_str()) {
                    if id != self.profile.record.id && self.registry.delete_profile(id).is_ok() {
                        self.set_toast("Profil supprimé du disque");
                        self.sync_ui();
                    }
                }
            }
            "launch_profile" => {
                if let Some(id) = msg.payload.get("id").and_then(|v| v.as_str()) {
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = std::process::Command::new(exe)
                            .args(["--profile", id])
                            .spawn();
                        self.set_toast("Fenêtre profil ouverte");
                        self.sync_ui();
                    }
                }
            }
            "purge_profile_data" => {
                for t in &self.tabs {
                    if let Some(wv) = &t.wv {
                        let _ = wv.clear_all_browsing_data();
                    }
                }
                let _ = self.profile.purge_all_local_data();
                self.set_toast("Données locales du profil purgées");
                self.sync_ui();
            }
            _ => {}
        }
    }
}

fn capture_screenshot(dir: &Path, filename: &str) {
    let _ = fs::create_dir_all(dir);
    let path = dir.join(filename);
    let _ = std::process::Command::new("import")
        .args(["-window", "root", &path.to_string_lossy()])
        .status();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let verify_mode = args.iter().any(|a| a == "--verify");
    let verify_phase5 = args.iter().any(|a| a == "--verify-phase5");
    let verify_ux = args.iter().any(|a| a == "--verify-ux");
    let any_verify = verify_mode || verify_phase5 || verify_ux;
    let bench_mode = args.iter().any(|a| a == "--bench");
    let cli_profile = args
        .windows(2)
        .find(|w| w[0] == "--profile")
        .map(|w| w[1].clone());
    let capture_dir = args
        .windows(2)
        .find(|w| w[0] == "--capture-dir")
        .map(|w| PathBuf::from(&w[1]));

    let rt = Arc::new(tokio::runtime::Runtime::new().expect("tokio runtime"));
    let shield = Arc::new(ShieldEngine::new_with_rules(DEFAULT_RULES));

    if bench_mode {
        let runs = args
            .windows(2)
            .find(|w| w[0] == "--runs")
            .and_then(|w| w[1].parse::<usize>().ok())
            .unwrap_or(3);
        let bench_out = args
            .windows(2)
            .find(|w| w[0] == "--bench-out")
            .map(|w| PathBuf::from(&w[1]));
        let (proxy, rx) = SecureLoopbackProxy::new(ProxyConfig::default(), shield.clone());
        let addr = rt.block_on(proxy.start(rx)).expect("bench proxy");
        bench::run_memory_benchmark_suite(addr.port(), runs, bench_out);
        proxy.shutdown();
        return;
    }

    let base_data_dir = if any_verify {
        tempfile::tempdir().expect("verify profile tempdir").keep()
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".local/share/orakle-browser")
    };

    let registry = ProfileRegistry::load_or_init(&base_data_dir).expect("profile registry");
    let active_profile_id =
        cli_profile.unwrap_or_else(|| registry.startup_profile_id().to_string());
    let active_profile_name = registry
        .list_profiles()
        .into_iter()
        .find(|p| p.id == active_profile_id)
        .map(|p| p.name)
        .unwrap_or_else(|| "Profil Principal".to_string());
    let profile =
        ProfileContext::open_persistent(&base_data_dir, &active_profile_id, &active_profile_name)
            .expect("open active profile");
    let _ = registry.save();

    let spaces = match profile.storage.get_spaces() {
        Ok(v) if !v.is_empty() => v,
        _ => {
            let def = Space {
                id: "default".into(),
                name: "Travail".into(),
                color: "#0969da".into(),
                created_at: now_ts(),
            };
            let _ = profile.storage.insert_space(&def);
            vec![def]
        }
    };

    let rules_file_path = profile.root_dir.join("orakle_shield_rules.txt");
    sync_rules_file_for_webkit_extension(&shield, &rules_file_path).expect("sync rules file");
    std::env::set_var("ORAKLE_SHIELD_RULES_FILE", &rules_file_path);
    let ext_dir_opt = find_webkit_extension_dir(&profile.root_dir);

    let (demo_addr_opt, proxy_cfg, proxy_addr, proxy) = rt.block_on(async {
        let demo = if any_verify {
            Some(spawn_local_demo_server().await)
        } else {
            None
        };
        let mut cfg = ProxyConfig::default();
        if let Some(d) = demo {
            cfg.allowed_ports = vec![80, 443, 8080, d.port()];
            cfg.allow_loopback_upstream = true;
        }
        let cfg_clone = cfg.clone();
        let (proxy, rx) = SecureLoopbackProxy::new(cfg, shield.clone());
        let addr = proxy.start(rx).await.expect("start loopback proxy");
        (demo, cfg_clone, addr, Arc::new(proxy))
    });

    let download_dir = profile.root_dir.join("downloads");
    let _ = fs::create_dir_all(&download_dir);

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let event_proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Orakle Browser V2")
        .with_inner_size(LogicalSize::new(1360.0, 860.0))
        .with_min_inner_size(LogicalSize::new(520.0, 360.0))
        .with_resizable(true)
        .build(&event_loop)
        .expect("create main window");

    #[cfg(target_os = "linux")]
    let (scrolled_container, fixed_container) = {
        let vbox = window.default_vbox().expect("GTK default vbox");
        let sw = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
        sw.set_policy(gtk::PolicyType::External, gtk::PolicyType::External);
        sw.set_shadow_type(gtk::ShadowType::None);
        sw.set_min_content_width(480);
        sw.set_min_content_height(320);
        let fixed = gtk::Fixed::new();
        sw.add(&fixed);
        vbox.pack_start(&sw, true, true, 0);
        sw.show_all();
        let alloc_proxy = event_proxy.clone();
        let last_alloc = Rc::new(std::cell::Cell::new((0_i32, 0_i32)));
        sw.connect_size_allocate(move |_, alloc| {
            let w = alloc.width();
            let h = alloc.height();
            if w >= 120 && h >= 120 && last_alloc.get() != (w, h) {
                last_alloc.set((w, h));
                let _ = alloc_proxy.send_event(UserEvent::HostResized(w, h));
            }
        });
        (sw, fixed)
    };

    #[allow(unused_mut)]
    let mut normal_ctx = WebContext::new(Some(profile.webview_data_dir.clone()));

    let chrome_ipc = event_proxy.clone();
    let chrome_builder = WebViewBuilder::new()
        .with_html(CHROME_HTML)
        .with_bounds(make_rect(0, 0, 1360, 86))
        .with_navigation_handler(|u| u == "about:blank")
        .with_ipc_handler(move |req| {
            let _ = chrome_ipc.send_event(UserEvent::IpcCommand(req.body().to_string()));
        });

    #[cfg(target_os = "linux")]
    let chrome_wv = chrome_builder
        .build_gtk(&fixed_container)
        .expect("build chrome_wv");
    #[cfg(not(target_os = "linux"))]
    let chrome_wv = chrome_builder
        .build_as_child(&window)
        .expect("build chrome_wv");

    #[cfg(target_os = "linux")]
    if let Some(c) = chrome_wv.webview().web_context() {
        c.set_cache_model(CacheModel::DocumentViewer);
    }

    let panel_ipc = event_proxy.clone();
    let panel_builder = WebViewBuilder::new()
        .with_html(PANEL_HTML)
        .with_bounds(make_rect(1000, 86, PANEL_WIDTH, 860 - 86))
        .with_navigation_handler(|u| u == "about:blank")
        .with_ipc_handler(move |req| {
            let _ = panel_ipc.send_event(UserEvent::IpcCommand(req.body().to_string()));
        });

    #[cfg(target_os = "linux")]
    let panel_wv = panel_builder
        .build_gtk(&fixed_container)
        .expect("build panel_wv");
    #[cfg(not(target_os = "linux"))]
    let panel_wv = panel_builder
        .build_as_child(&window)
        .expect("build panel_wv");
    let _ = panel_wv.set_visible(false);

    #[cfg(target_os = "linux")]
    if let Some(c) = panel_wv.webview().web_context() {
        c.set_cache_model(CacheModel::DocumentViewer);
    }

    let saved_theme = profile
        .storage
        .get_setting("theme")
        .ok()
        .flatten()
        .unwrap_or_else(|| "dark".into());
    let saved_accent = profile
        .storage
        .get_setting("accent")
        .ok()
        .flatten()
        .unwrap_or_else(|| "#0969da".into());
    let saved_density = profile
        .storage
        .get_setting("density")
        .ok()
        .flatten()
        .unwrap_or_else(|| "comfortable".into());
    let saved_search = profile
        .storage
        .get_setting("search_engine")
        .ok()
        .flatten()
        .map(|s| normalize_search_engine(&s).to_string())
        .unwrap_or_else(|| "duckduckgo".into());
    let search_configured = profile
        .storage
        .get_setting("search_engine_configured")
        .ok()
        .flatten()
        .is_some();
    let needs_search_onboarding = (!any_verify && !search_configured) || verify_ux;
    let tips_on_startup = profile
        .storage
        .get_setting("tips_on_startup")
        .ok()
        .flatten()
        .as_deref()
        != Some("0");

    let demo_base_url = demo_addr_opt.map(|d| format!("http://{}", d));
    let mut host = BrowserHost {
        rt,
        registry,
        profile,
        private_session: None,
        shield,
        proxy,
        proxy_cfg,
        proxy_port: proxy_addr.port(),
        proxy_running: true,
        proxy_running_flag: Arc::new(AtomicBool::new(true)),
        rules_file_path,
        ext_dir_opt: ext_dir_opt.clone(),
        download_dir,
        allow_loopback_nav: demo_base_url.is_some(),
        demo_base_url: demo_base_url.clone(),
        window,
        #[cfg(target_os = "linux")]
        scrolled_container,
        #[cfg(target_os = "linux")]
        fixed_container,
        event_proxy: event_proxy.clone(),
        normal_ctx,
        chrome_wv,
        panel_wv,
        tabs: Vec::new(),
        closed_tabs: Vec::new(),
        active_space_id: spaces[0].id.clone(),
        spaces,
        left_tab_id: None,
        right_tab_id: None,
        focused_pane: "left".into(),
        split_active: false,
        split_ratio: 0.50,
        sync_scroll: false,
        panel_open: false,
        panel_section: "shield".into(),
        chrome_overlay_h: 0,
        theme_pref: saved_theme,
        accent: saved_accent,
        density: saved_density,
        toast: None,
        search_engine: saved_search,
        needs_search_onboarding,
        tips_on_startup,
        last_pulse: None,
        live_downloads: HashMap::new(),
        resumable_controls: HashMap::new(),
        #[cfg(target_os = "linux")]
        webkit_downloads: Rc::new(RefCell::new(HashMap::new())),
        download_ctx_hooked: HashSet::new(),
        authorized_localhost_origins: Arc::new(Mutex::new(HashMap::new())),
        win_w: 1360,
        win_h: 860,
        next_id: 0,
    };

    let restored_tabs = if !any_verify {
        host.profile
            .storage
            .get_tabs_for_space(&host.active_space_id)
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    if restored_tabs.is_empty() {
        let initial_url = if verify_ux {
            "about:home".into()
        } else {
            demo_base_url
                .as_ref()
                .map(|b| format!("{}/home", b))
                .unwrap_or_else(|| "about:home".into())
        };
        host.open_new_tab(&initial_url, false, true);
    } else {
        let last_idx = restored_tabs.len() - 1;
        for (i, r) in restored_tabs.into_iter().enumerate() {
            if i == last_idx {
                host.authorize_user_localhost(&r.id, &r.url);
                let wv = host.create_webview_for_tab(&r.id, &r.url, false);
                host.tabs.push(TabItem {
                    id: r.id.clone(),
                    space_id: r.space_id,
                    group_id: r.group_id,
                    url: r.url,
                    title: r.title,
                    is_pinned: r.is_pinned,
                    is_suspended: false,
                    is_incognito: false,
                    is_loading: true,
                    is_playing_audio: false,
                    wv: Some(wv),
                });
                host.left_tab_id = Some(r.id);
            } else {
                // Les onglets inactifs restaurés démarrent suspendus pour optimiser mémoire et latence
                host.tabs.push(TabItem {
                    id: r.id,
                    space_id: r.space_id,
                    group_id: r.group_id,
                    url: r.url,
                    title: r.title,
                    is_pinned: r.is_pinned,
                    is_suspended: true,
                    is_incognito: false,
                    is_loading: false,
                    is_playing_audio: false,
                    wv: None,
                });
            }
        }
        host.relayout();
        host.sync_ui();
    }

    if verify_mode {
        let vp = event_proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(2200));
            let _ = vp.send_event(UserEvent::VerifyStep(1));
            std::thread::sleep(Duration::from_millis(1800));
            let _ = vp.send_event(UserEvent::VerifyStep(2));
            std::thread::sleep(Duration::from_millis(1600));
            let _ = vp.send_event(UserEvent::VerifyStep(3));
            std::thread::sleep(Duration::from_millis(1600));
            let _ = vp.send_event(UserEvent::VerifyStep(4));
            std::thread::sleep(Duration::from_millis(1600));
            let _ = vp.send_event(UserEvent::VerifyStep(5));
        });
    } else if verify_phase5 {
        let vp = event_proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(2200));
            for step in 1..=7 {
                let _ = vp.send_event(UserEvent::VerifyPhase5Step(step));
                std::thread::sleep(Duration::from_millis(1600));
            }
        });
    } else if verify_ux {
        let vp = event_proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(2200));
            for step in 1..=6 {
                let _ = vp.send_event(UserEvent::VerifyUxStep(step));
                std::thread::sleep(Duration::from_millis(1600));
            }
        });
    }

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if let Some(priv_sess) = host.private_session.take() {
                    let _ = priv_sess.cleanup();
                }
                host.proxy.shutdown();
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(new_size),
                ..
            } => {
                let scale = host.window.scale_factor();
                let logical = new_size.to_logical::<i32>(scale);
                #[cfg(target_os = "linux")]
                let (w, h) = {
                    let alloc = host.scrolled_container.allocation();
                    if alloc.width() >= 120 && alloc.height() >= 120 {
                        (alloc.width(), alloc.height())
                    } else {
                        (logical.width, logical.height)
                    }
                };
                #[cfg(not(target_os = "linux"))]
                let (w, h) = (logical.width, logical.height);
                if w >= 120 && h >= 120 {
                    host.win_w = w;
                    host.win_h = h;
                    #[cfg(target_os = "linux")]
                    {
                        host.scrolled_container.hadjustment().set_value(0.0);
                        host.scrolled_container.vadjustment().set_value(0.0);
                    }
                    host.relayout();
                }
            }
            Event::WindowEvent {
                event: WindowEvent::ScaleFactorChanged { new_inner_size, .. },
                ..
            } => {
                let scale = host.window.scale_factor();
                let logical = new_inner_size.to_logical::<i32>(scale);
                if logical.width >= 120 && logical.height >= 120 {
                    host.win_w = logical.width;
                    host.win_h = logical.height;
                    host.relayout();
                }
            }
            Event::UserEvent(UserEvent::HostResized(w, h)) => {
                if w >= 120 && h >= 120 && (host.win_w != w || host.win_h != h) {
                    host.win_w = w;
                    host.win_h = h;
                    #[cfg(target_os = "linux")]
                    {
                        host.scrolled_container.hadjustment().set_value(0.0);
                        host.scrolled_container.vadjustment().set_value(0.0);
                    }
                    host.relayout();
                }
            }
            Event::UserEvent(UserEvent::IpcCommand(raw)) => {
                host.handle_ipc(&raw);
            }
            Event::UserEvent(UserEvent::NewTabFromWeb(u)) => {
                host.open_new_tab(&u, false, true);
            }
            Event::UserEvent(UserEvent::WebTitleChanged { tab_id, title }) => {
                if let Some(idx) = host.tabs.iter().position(|t| t.id == tab_id) {
                    if title.trim().is_empty() && host.tabs[idx].url == "about:home" {
                        // Conserver le titre lisible d'accueil si WebKit émet un titre vide
                    } else {
                        host.tabs[idx].title = title;
                    }
                    let snapshot = TabItem {
                        id: host.tabs[idx].id.clone(),
                        space_id: host.tabs[idx].space_id.clone(),
                        group_id: host.tabs[idx].group_id.clone(),
                        url: host.tabs[idx].url.clone(),
                        title: host.tabs[idx].title.clone(),
                        is_pinned: host.tabs[idx].is_pinned,
                        is_suspended: host.tabs[idx].is_suspended,
                        is_incognito: host.tabs[idx].is_incognito,
                        is_loading: host.tabs[idx].is_loading,
                        is_playing_audio: host.tabs[idx].is_playing_audio,
                        wv: None,
                    };
                    host.persist_tab(&snapshot, idx as i32);
                    host.sync_ui();
                }
            }
            Event::UserEvent(UserEvent::WebUriChanged { tab_id, uri }) => {
                if let Some(idx) = host.tabs.iter().position(|t| t.id == tab_id) {
                    if uri == "about:blank" && host.tabs[idx].url != "about:blank" {
                        // Préserver l'URL cible lors d'un chargement HTML local (erreur ou accueil)
                    } else {
                        host.tabs[idx].url = uri.clone();
                        if !host.tabs[idx].is_incognito && uri != "about:blank" {
                            let _ = host.profile.storage.insert_history_entry(&HistoryEntry {
                                id: None,
                                url: uri,
                                title: host.tabs[idx].title.clone(),
                                visited_at: now_ts(),
                            });
                        }
                        host.sync_ui();
                    }
                }
            }
            Event::UserEvent(UserEvent::WebLoadingChanged { tab_id, loading }) => {
                if let Some(t) = host.tabs.iter_mut().find(|t| t.id == tab_id) {
                    t.is_loading = loading;
                    host.sync_ui();
                }
            }
            Event::UserEvent(UserEvent::WebAudioChanged { tab_id, playing }) => {
                if let Some(t) = host.tabs.iter_mut().find(|t| t.id == tab_id) {
                    t.is_playing_audio = playing;
                    host.sync_ui();
                }
            }
            Event::UserEvent(UserEvent::NativeAdapterBlocked {
                url,
                resource_type,
                rule,
            }) => {
                host.proxy
                    .record_native_adapter_block_detailed(&url, &resource_type, rule);
                host.sync_ui();
            }
            Event::UserEvent(UserEvent::WebPermissionDenied { origin, permission }) => {
                let _ = host.profile.storage.set_domain_permission(&DomainPermission {
                    origin,
                    permission,
                    decision: "blocked".into(),
                    updated_at: now_ts(),
                });
                host.sync_ui();
            }
            Event::UserEvent(UserEvent::DownloadStarted {
                id,
                url,
                filename,
                save_path,
            }) => {
                let _ = host.profile.storage.upsert_download(&DownloadRecord {
                    id: id.clone(),
                    url,
                    filename: filename.clone(),
                    save_path,
                    status: "En cours".into(),
                    bytes_received: 0,
                    total_bytes: 0,
                    started_at: now_ts(),
                });
                host.live_downloads.insert(
                    id,
                    LiveDownloadMeta {
                        resumable: false,
                        speed_bps: 0,
                        exec_warning: matches!(
                            classify_filename_risk(&filename),
                            DownloadSafetyRisk::ExecutableWarning { .. }
                        ),
                        error_message: None,
                    },
                );
                host.sync_ui();
            }
            Event::UserEvent(UserEvent::DownloadCompleted { url, success }) => {
                if let Ok(list) = host.profile.storage.get_downloads() {
                    if let Some(mut d) = list.into_iter().find(|d| d.url == url) {
                        if let Ok(meta) = fs::metadata(&d.save_path) {
                            d.bytes_received = meta.len() as i64;
                            if d.total_bytes <= 0 {
                                d.total_bytes = d.bytes_received;
                            }
                        }
                        d.status = if success {
                            "Terminé".into()
                        } else {
                            "Échec".into()
                        };
                        let _ = host.profile.storage.upsert_download(&d);
                        if success {
                            host.set_toast(format!("Téléchargement terminé : {}", d.filename));
                        }
                    }
                }
                host.sync_ui();
            }
            Event::UserEvent(UserEvent::DownloadProgress {
                id,
                url,
                filename,
                save_path,
                received_bytes,
                total_bytes,
                speed_bps,
                status,
                error_message,
            }) => {
                let exec_warning = matches!(
                    classify_filename_risk(&filename),
                    DownloadSafetyRisk::ExecutableWarning { .. }
                );
                host.live_downloads.insert(
                    id.clone(),
                    LiveDownloadMeta {
                        resumable: true,
                        speed_bps,
                        exec_warning,
                        error_message: error_message.clone(),
                    },
                );
                let existing_ts = host
                    .profile
                    .storage
                    .get_downloads()
                    .ok()
                    .and_then(|l| l.into_iter().find(|d| d.id == id).map(|d| d.started_at))
                    .unwrap_or_else(now_ts);
                let _ = host.profile.storage.upsert_download(&DownloadRecord {
                    id: id.clone(),
                    url,
                    filename: filename.clone(),
                    save_path,
                    status: status.clone(),
                    bytes_received: received_bytes as i64,
                    total_bytes: total_bytes as i64,
                    started_at: existing_ts,
                });
                if status == "Terminé" {
                    host.resumable_controls.remove(&id);
                    host.set_toast(format!("Téléchargement terminé : {}", filename));
                } else if status == "Échec" || status == "Annulé" {
                    host.resumable_controls.remove(&id);
                }
                let _ = DL_STATE_PAUSED;
                host.sync_ui();
            }
            Event::UserEvent(UserEvent::PulseCompleted(p)) => {
                host.last_pulse = Some(p);
                host.sync_ui();
            }
            Event::UserEvent(UserEvent::SyncScrollApply { tab_id, y }) => {
                if let Some(d) = host.tabs.iter().find(|t| t.id == tab_id).and_then(|t| t.wv.as_ref()) {
                    let _ = d.evaluate_script(&format!(
                        "window.scrollTo({{ top: {:.1}, behavior: 'auto' }})",
                        y
                    ));
                }
            }
            Event::UserEvent(UserEvent::VerifyStep(step)) => match step {
                1 => {
                    let s = metrics::capture_memory_cpu_snapshot();
                    println!(
                        "[P4 STEP 1 — Home chargée (Extension active={})] total_rss={} KB ({:.1} MiB) | shield_blocked={}",
                        ext_dir_opt.is_some(),
                        s.total_rss_kb,
                        s.total_rss_kb as f64 / 1024.0,
                        host.proxy.telemetry_snapshot().blocked_by_shield
                    );
                    if let Some(d) = &capture_dir {
                        capture_screenshot(d, "01-orakle-v2-home-dark.png");
                    }
                    // Activer Split View + Volet Gauche/Droit indépendants + Side Panel Shield
                    let left_url = format!("{}/left", host.demo_base_url.as_ref().unwrap());
                    let right_url = format!("{}/right", host.demo_base_url.as_ref().unwrap());
                    if let Some(lid) = host.left_tab_id.clone() {
                        if let Some(t) = host.tabs.iter_mut().find(|t| t.id == lid) {
                            t.url = left_url.clone();
                            if let Some(wv) = &t.wv {
                                let _ = wv.load_url(&left_url);
                            }
                        }
                    }
                    let rid = host.open_new_tab(&right_url, true, false);
                    host.right_tab_id = Some(rid);
                    host.split_active = true;
                    host.panel_open = true;
                    host.panel_section = "shield".into();
                    host.relayout();
                    host.sync_ui();
                }
                2 => {
                    let s = metrics::capture_memory_cpu_snapshot();
                    println!(
                        "[P4 STEP 2 — Split View 50/50 + Panneau Shield] web_procs={} | total_rss={} KB ({:.1} MiB) | total_pss={} KB ({:.1} MiB)",
                        s.webkit_web.count,
                        s.total_rss_kb,
                        s.total_rss_kb as f64 / 1024.0,
                        s.total_pss_kb,
                        s.total_pss_kb as f64 / 1024.0
                    );
                    if let Some(d) = &capture_dir {
                        capture_screenshot(d, "02-orakle-v2-split-shield-dark.png");
                    }
                    // Ratio 70/30 + swap + thème clair
                    host.split_ratio = 0.70;
                    std::mem::swap(&mut host.left_tab_id, &mut host.right_tab_id);
                    host.focused_pane = "right".into();
                    host.theme_pref = "light".into();
                    let _ = host.profile.storage.set_setting("theme", "light");
                    host.panel_section = "spaces".into();
                    host.relayout();
                    host.sync_ui();
                }
                3 => {
                    if let Some(d) = &capture_dir {
                        capture_screenshot(d, "03-orakle-v2-split-light.png");
                    }
                    // Suspension du volet droit -> destruction WebView réelle
                    if let Some(rid) = host.right_tab_id.clone() {
                        host.suspend_tab(&rid);
                        host.split_active = false;
                        host.panel_open = false;
                        host.relayout();
                        host.sync_ui();
                    }
                }
                4 => {
                    let s = metrics::capture_memory_cpu_snapshot();
                    println!(
                        "[P4 STEP 4 — Après Suspension Volet Droit] web_procs={} | total_rss={} KB ({:.1} MiB)",
                        s.webkit_web.count,
                        s.total_rss_kb,
                        s.total_rss_kb as f64 / 1024.0
                    );
                    // Restauration du volet droit + vérification étanchéité historique Privé
                    if let Some(rid) = host.right_tab_id.clone() {
                        host.restore_tab(&rid);
                        host.split_active = true;
                        host.relayout();
                        host.sync_ui();
                    }
                }
                5 => {
                    let s = metrics::capture_memory_cpu_snapshot();
                    let tel = host.proxy.telemetry_snapshot();
                    let hist = host.profile.storage.get_history(50).unwrap_or_default();
                    let priv_leaked = hist.iter().any(|h| h.url.contains("/right"));
                    assert!(!priv_leaked, "Un onglet privé ne doit jamais fuiter dans l'historique SQLite !");
                    println!(
                        "[P4 STEP 5 — Après Restauration] web_procs={} | total_rss={} KB ({:.1} MiB) | shield_blocked={} (natif={}) | history_count={} | priv_leaked={}",
                        s.webkit_web.count,
                        s.total_rss_kb,
                        s.total_rss_kb as f64 / 1024.0,
                        tel.blocked_by_shield,
                        tel.native_adapter_blocks,
                        hist.len(),
                        priv_leaked
                    );
                    if let Some(priv_sess) = host.private_session.take() {
                        let _ = priv_sess.cleanup();
                    }
                    host.proxy.shutdown();
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            },
            Event::UserEvent(UserEvent::VerifyPhase5Step(step)) => {
                let demo = host
                    .demo_base_url
                    .clone()
                    .unwrap_or_else(|| "http://127.0.0.1:8080".into());
                match step {
                    1 => {
                        // Capture Accueil sombre stabilisée -> ouvrir HTTP /page2 + Favori + Bang + préparer dropdown Omnibox
                        if let Some(d) = &capture_dir {
                            capture_screenshot(d, "p5-01-dark-home.png");
                        }
                        let p2_url = format!("{}/page2", demo);
                        let new_id = host.open_new_tab(&p2_url, false, true);
                        let bm_id = host.gen_id("bm");
                        let _ = host.profile.storage.upsert_bookmark(&BookmarkEntry {
                            id: bm_id,
                            url: p2_url.clone(),
                            title: "Page 2 Demo".into(),
                            category: "Validation".into(),
                            created_at: now_ts(),
                        });
                        let (bang_url, _) = resolve_omnibox_input("!gh orakle", &host.search_engine);
                        assert!(bang_url.contains("github.com/search"));

                        host.chrome_overlay_h = 190;
                        host.relayout();
                        let open_tabs: Vec<(String, String, String, bool)> = host
                            .tabs
                            .iter()
                            .map(|t| (t.id.clone(), t.title.clone(), t.url.clone(), t.is_suspended))
                            .collect();
                        let bms = host.profile.storage.get_bookmarks().unwrap_or_default();
                        let hist = host.profile.storage.get_history(40).unwrap_or_default();
                        let suggs = build_omnibox_suggestions(
                            "page",
                            &host.search_engine,
                            &open_tabs,
                            &bms,
                            &hist,
                        );
                        if let Ok(json) = serde_json::to_string(&suggs) {
                            let _ = host.chrome_wv.evaluate_script(&format!(
                                "var oi=document.getElementById('urlInput');if(oi){{oi.focus();oi.value='page';}}window.applyOmniboxSuggestions({});",
                                json
                            ));
                        }
                        println!(
                            "[P5 STEP 1] Onglet HTTP ouvert ({}), Favori ajouté, Bang !gh résolu -> {}, Omnibox suggestions={}",
                            new_id, bang_url, suggs.len()
                        );
                    }
                    2 => {
                        // Capture Omnibox Dropdown stabilisé -> fermer overlay -> back/forward/reload -> Split View 70/30 + Shield
                        if let Some(d) = &capture_dir {
                            capture_screenshot(d, "p5-02-omnibox-dropdown.png");
                        }
                        host.chrome_overlay_h = 0;
                        let _ = host
                            .chrome_wv
                            .evaluate_script("window.closeOverlays && window.closeOverlays(); var oi=document.getElementById('urlInput'); if(oi) oi.blur();");
                        if let Some(wv) = host
                            .focused_tab_id()
                            .and_then(|id| host.tabs.iter().find(|t| t.id == id))
                            .and_then(|t| t.wv.as_ref())
                        {
                            let _ = wv.evaluate_script("history.back(); history.forward(); location.reload();");
                        }
                        let left_u = format!("{}/left", demo);
                        let right_u = format!("{}/right", demo);
                        if let Some(lid) = host.left_tab_id.clone() {
                            host.navigate_tab_to(&lid, &left_u);
                        }
                        let rid = host.open_new_tab(&right_u, false, false);
                        host.right_tab_id = Some(rid);
                        host.split_active = true;
                        host.split_ratio = 0.70;
                        host.panel_open = true;
                        host.panel_section = "shield".into();
                        host.relayout();
                        host.sync_ui();
                        println!("[P5 STEP 2] Nav Back/Forward/Reload + Split View 70/30 + Shield actif");
                    }
                    3 => {
                        // Capture Split View 70/30 + Shield -> basculer Thème Clair + Compact + Historique
                        if let Some(d) = &capture_dir {
                            capture_screenshot(d, "p5-03-split-shield.png");
                        }
                        host.theme_pref = "light".into();
                        host.density = "compact".into();
                        host.accent = "indigo".into();
                        host.panel_open = true;
                        host.panel_section = "history".into();
                        let _ = host.profile.storage.set_setting("theme", "light");
                        let _ = host.profile.storage.set_setting("density", "compact");
                        let _ = host.profile.storage.set_setting("accent", "indigo");
                        host.relayout();
                        host.sync_ui();
                        println!("[P5 STEP 3] Thème clair + densité compact + historique");
                    }
                    4 => {
                        // Capture Light + Compact + Historique -> Téléchargement résumable /download-test.bin
                        if let Some(d) = &capture_dir {
                            capture_screenshot(d, "p5-04-light-history.png");
                        }
                        host.split_active = false;
                        host.panel_open = true;
                        host.panel_section = "downloads".into();
                        let dl_url = format!("{}/download-test.bin", demo);
                        host.handle_ipc(&format!(
                            "{{\"cmd\":\"start_resumable_download\",\"payload\":{{\"url\":\"{}\"}}}}",
                            dl_url
                        ));
                        host.relayout();
                        host.sync_ui();
                        println!("[P5 STEP 4] Téléchargement résumable démarré ({})", dl_url);
                    }
                    5 => {
                        // Capture Downloads terminé -> Créer profil + Proxy Fail-Closed Outage + Panneau Profils
                        if let Some(d) = &capture_dir {
                            capture_screenshot(d, "p5-05-downloads.png");
                        }
                        let _ = host.registry.create_profile("Profil Test P5");
                        host.shutdown_proxy_fail_closed();
                        host.panel_open = true;
                        host.panel_section = "profiles".into();
                        host.relayout();
                        host.sync_ui();
                        println!("[P5 STEP 5] Proxy Fail-Closed activé + Profils affichés");
                    }
                    6 => {
                        // Capture Panne Proxy + Profils -> Redémarrer Proxy + Page d'erreur native (port éphemère fermé)
                        if let Some(d) = &capture_dir {
                            capture_screenshot(d, "p5-06-proxy-outage-profiles.png");
                        }
                        host.restart_proxy_fail_closed();
                        host.panel_open = false;
                        let closed_port = std::net::TcpListener::bind("127.0.0.1:0")
                            .ok()
                            .and_then(|l| l.local_addr().ok())
                            .map(|a| a.port())
                            .unwrap_or(59999);
                        let err_target = format!("https://127.0.0.1:{}/unreachable", closed_port);
                        if let Some(fid) = host.focused_tab_id() {
                            host.navigate_tab_to(&fid, &err_target);
                        }
                        host.relayout();
                        host.sync_ui();
                        println!("[P5 STEP 6] Proxy redémarré + déclenchement page d'erreur native ({})", err_target);
                    }
                    7 => {
                        // Capture Page d'erreur stabilisée -> Fermer/Rouvrir onglet + Save session + Bilan
                        if let Some(d) = &capture_dir {
                            capture_screenshot(d, "p5-07-error-page.png");
                        }
                        if host.tabs.len() >= 2 {
                            let to_close = host.tabs[0].id.clone();
                            host.close_tab(&to_close);
                            assert!(!host.closed_tabs.is_empty());
                            host.handle_ipc("{\"cmd\":\"reopen_closed\",\"payload\":{}}");
                        }
                        host.handle_ipc("{\"cmd\":\"save_session\",\"payload\":{\"name\":\"Session Phase 5\"}}");
                        let tabs_db = host
                            .profile
                            .storage
                            .get_tabs_for_space(&host.active_space_id)
                            .unwrap_or_default();
                        let bms_db = host.profile.storage.get_bookmarks().unwrap_or_default();
                        let hist_db = host.profile.storage.get_history(50).unwrap_or_default();
                        let dls_db = host.profile.storage.get_downloads().unwrap_or_default();
                        let sessions_db = host.profile.storage.get_sessions().unwrap_or_default();
                        let s = metrics::capture_memory_cpu_snapshot();
                        println!(
                            "[P5 STEP 7 — BILAN FINAL] tabs_open={} tabs_db={} bookmarks={} history={} downloads={} sessions={} profiles={} rss_mib={:.1} pss_mib={:.1}",
                            host.tabs.len(),
                            tabs_db.len(),
                            bms_db.len(),
                            hist_db.len(),
                            dls_db.len(),
                            sessions_db.len(),
                            host.registry.list_profiles().len(),
                            s.total_rss_kb as f64 / 1024.0,
                            s.total_pss_kb as f64 / 1024.0
                        );
                        if let Some(priv_sess) = host.private_session.take() {
                            let _ = priv_sess.cleanup();
                        }
                        host.proxy.shutdown();
                        *control_flow = ControlFlow::Exit;
                    }
                    _ => {}
                }
            }
            Event::UserEvent(UserEvent::VerifyUxStep(step)) => {
                let demo = host
                    .demo_base_url
                    .clone()
                    .unwrap_or_else(|| "http://127.0.0.1:8080".into());
                match step {
                    1 => {
                        assert!(host.needs_search_onboarding, "L'assistant moteur doit être actif au 1er lancement");
                        assert!(host.chrome_overlay_h >= 120, "Overlay onboarding non ouvert (h={})", host.chrome_overlay_h);
                        if let Some(d) = &capture_dir {
                            capture_screenshot(d, "ux-01-search-onboarding.png");
                        }
                        host.handle_ipc("{\"cmd\":\"complete_search_onboarding\",\"payload\":{\"engine\":\"qwant\"}}");
                        assert_eq!(host.search_engine, "qwant");
                        assert!(!host.needs_search_onboarding);
                        assert_eq!(host.chrome_overlay_h, 0);
                        assert_eq!(
                            host.profile.storage.get_setting("search_engine").ok().flatten().as_deref(),
                            Some("qwant")
                        );
                        assert_eq!(
                            host.profile.storage.get_setting("search_engine_configured").ok().flatten().as_deref(),
                            Some("1")
                        );
                        let (q_url, _) = resolve_omnibox_input("orakle rust", &host.search_engine);
                        assert!(q_url.starts_with("https://www.qwant.com/"));
                        println!("[UX STEP 1] Onboarding moteur Qwant validé -> {}", q_url);
                    }
                    2 => {
                        host.handle_ipc("{\"cmd\":\"open_tips_overlay\",\"payload\":{}}");
                        std::thread::sleep(Duration::from_millis(250));
                        // Assurer hauteur overlay si JS asynchrone
                        if host.chrome_overlay_h < 180 {
                            host.chrome_overlay_h = 224;
                            host.relayout();
                        }
                        println!("[UX STEP 2] Panneau Astuces ouvert (overlay_h={})", host.chrome_overlay_h);
                    }
                    3 => {
                        if let Some(d) = &capture_dir {
                            capture_screenshot(d, "ux-02-tips-overlay.png");
                        }
                        let _ = host.chrome_wv.evaluate_script("window.closeOverlays && window.closeOverlays();");
                        host.chrome_overlay_h = 0;
                        host.relayout();

                        let t0 = host.tabs[0].id.clone();
                        host.handle_ipc("{\"cmd\":\"new_tab\",\"payload\":{\"incognito\":false}}");
                        host.handle_ipc("{\"cmd\":\"new_tab\",\"payload\":{\"incognito\":false}}");
                        assert_eq!(host.tabs.len(), 3);
                        let t1 = host.tabs[1].id.clone();
                        let t2 = host.tabs[2].id.clone();
                        host.select_tab(&t1);
                        host.close_tab(&t1);
                        assert_eq!(host.tabs.len(), 2);
                        assert_eq!(
                            host.left_tab_id.as_deref(),
                            Some(t2.as_str()),
                            "Fermer l'onglet du milieu doit activer l'onglet adjacent"
                        );
                        host.close_tab(&t2);
                        assert_eq!(host.left_tab_id.as_deref(), Some(t0.as_str()));

                        let p2_url = format!("{}/page2", demo);
                        host.navigate_tab_to(&t0, &p2_url);
                        host.close_tab(&t0);
                        assert_eq!(host.tabs.len(), 1, "Fermer le dernier onglet doit ouvrir un nouvel onglet propre");
                        assert_ne!(host.tabs[0].id, t0);
                        assert!(!host.closed_tabs.is_empty());

                        host.handle_ipc("{\"cmd\":\"reopen_closed\",\"payload\":{}}");
                        assert_eq!(host.tabs.len(), 2);
                        println!("[UX STEP 3] Nouvel onglet + Fermeture adjacent + Fermeture dernier onglet + Réouverture validés");
                    }
                    4 => {
                        host.window.set_maximized(true);
                        host.window.set_inner_size(LogicalSize::new(1680.0, 960.0));
                        println!("[UX STEP 4] Agrandissement / Maximisation fenêtre déclenché");
                    }
                    5 => {
                        assert!(
                            host.win_w >= 1450 && host.win_h >= 880,
                            "La fenêtre maximisée/agrandie doit mettre à jour win_w/win_h ({}x{})",
                            host.win_w,
                            host.win_h
                        );
                        if let Some(d) = &capture_dir {
                            capture_screenshot(d, "ux-03-maximized.png");
                        }
                        println!("[UX STEP 5] Fenêtre agrandie mesurée : {}x{}", host.win_w, host.win_h);
                        host.window.set_maximized(false);
                        host.window.set_inner_size(LogicalSize::new(900.0, 580.0));
                    }
                    6 => {
                        assert!(
                            host.win_w <= 960 && host.win_h <= 640,
                            "La fenêtre doit pouvoir rétrécir après maximisation sans verrou GTK ({}x{})",
                            host.win_w,
                            host.win_h
                        );
                        if let Some(d) = &capture_dir {
                            capture_screenshot(d, "ux-04-shrunk.png");
                        }
                        println!(
                            "[UX STEP 6 — BILAN UX OK] Fenêtre réduite mesurée : {}x{} | tabs={}",
                            host.win_w,
                            host.win_h,
                            host.tabs.len()
                        );
                        if let Some(priv_sess) = host.private_session.take() {
                            let _ = priv_sess.cleanup();
                        }
                        host.proxy.shutdown();
                        *control_flow = ControlFlow::Exit;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::pick_adjacent_tab_id;

    #[test]
    fn adjacent_tab_selection_on_close() {
        let ids = vec!["t0".to_string(), "t2".to_string(), "t3".to_string()];
        // Fermeture de l'ancien index 1 ('t1') -> choisit 't2' (nouvel index 1)
        assert_eq!(pick_adjacent_tab_id(&ids, 1).as_deref(), Some("t2"));
        // Fermeture du dernier index (pos 3) -> choisit le dernier restant ('t3')
        assert_eq!(pick_adjacent_tab_id(&ids, 3).as_deref(), Some("t3"));
        // Fermeture du premier -> choisit 't0'
        assert_eq!(pick_adjacent_tab_id(&ids, 0).as_deref(), Some("t0"));
        assert_eq!(pick_adjacent_tab_id(&[], 0), None);
    }
}
