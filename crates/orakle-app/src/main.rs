mod bench;
mod metrics;

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
use webkit2gtk::{PermissionRequestExt, UserMessageExt, WebViewExt as WebKitWebViewExt};
#[cfg(target_os = "linux")]
use wry::{WebViewBuilderExtUnix, WebViewExtUnix};

use orakle_core::{
    platform_capability_matrix, sync_rules_file_for_webkit_extension, validate_navigation_url,
    BookmarkEntry, DownloadRecord, HistoryEntry, NetworkPulse, NoteEntry, PrivateSession,
    ProfileContext, ProxyConfig, SecureLoopbackProxy, SessionRecord, ShieldEngine, Space,
    TabRecord,
};

const CHROME_HTML: &str = include_str!("chrome.html");
const PANEL_HTML: &str = include_str!("panel.html");

const TOP_BASE_HEIGHT: i32 = 86; // 46px topbar + 40px tabstrip
const SPLIT_BAR_HEIGHT: i32 = 28;
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
    PulseCompleted(PulseViewData),
    SyncScrollApply {
        tab_id: String,
        y: f64,
    },
    VerifyStep(u8),
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
    shield_enabled: bool,
    shield_blocked: u64,
    can_go_back: bool,
    can_go_forward: bool,
    closed_count: usize,
    downloads_count: usize,
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

struct BrowserHost {
    rt: Arc<tokio::runtime::Runtime>,
    profile: ProfileContext,
    private_session: Option<PrivateSession>,
    shield: Arc<ShieldEngine>,
    proxy: Arc<SecureLoopbackProxy>,
    proxy_port: u16,
    rules_file_path: PathBuf,
    ext_dir_opt: Option<PathBuf>,
    download_dir: PathBuf,
    allow_loopback_nav: bool,
    demo_base_url: Option<String>,
    window: tao::window::Window,
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
    theme: String,
    search_engine: String,
    last_pulse: Option<PulseViewData>,
    win_w: i32,
    win_h: i32,
    next_id: usize,
}

impl BrowserHost {
    fn gen_id(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{}-{}-{}", prefix, now_ts(), self.next_id)
    }

    fn focused_tab_id(&self) -> Option<String> {
        if self.split_active && self.focused_pane == "right" {
            self.right_tab_id.clone()
        } else {
            self.left_tab_id.clone()
        }
    }

    fn normalize_input_url(&self, raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            return trimmed.to_string();
        }
        if trimmed.contains('.') && !trimmed.contains(' ') {
            return format!("https://{}", trimmed);
        }
        let encoded = trimmed.replace(' ', "+");
        match self.search_engine.as_str() {
            "startpage" => format!("https://www.startpage.com/do/search?q={}", encoded),
            "qwant" => format!("https://www.qwant.com/?q={}", encoded),
            _ => format!("https://duckduckgo.com/?q={}", encoded),
        }
    }

    fn content_area(&self) -> (i32, i32, i32) {
        let top_h = TOP_BASE_HEIGHT
            + if self.split_active {
                SPLIT_BAR_HEIGHT
            } else {
                0
            };
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

    fn create_webview_for_tab(&mut self, tab_id: &str, url: &str, incognito: bool) -> WebView {
        let (top_h, content_w, content_h) = self.content_area();
        let bounds = make_rect(0, top_h, content_w, content_h);
        let allow_loopback = self.allow_loopback_nav;

        let ev_load = self.event_proxy.clone();
        let id_load = tab_id.to_string();
        let ev_title = self.event_proxy.clone();
        let id_title = tab_id.to_string();
        let ev_new = self.event_proxy.clone();
        let ev_dl_start = self.event_proxy.clone();
        let ev_dl_done = self.event_proxy.clone();
        let dl_dir = self.download_dir.clone();

        // SÉCURITÉ : Aucune WebView distante ne reçoit `with_ipc_handler`.
        let mut builder = WebViewBuilder::new_with_web_context(&mut self.normal_ctx)
            .with_url(url)
            .with_bounds(bounds)
            .with_incognito(incognito)
            .with_proxy_config(WryProxyConfig::Http(ProxyEndpoint {
                host: "127.0.0.1".into(),
                port: self.proxy_port.to_string(),
            }))
            .with_navigation_handler(move |target_url| {
                validate_navigation_url(&target_url, allow_loopback).is_ok()
            })
            .with_new_window_req_handler(move |target_url, _features| {
                if validate_navigation_url(&target_url, allow_loopback).is_ok() {
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
                let fname = url::Url::parse(&dl_url)
                    .ok()
                    .and_then(|u| {
                        u.path_segments()
                            .and_then(|mut s| s.next_back())
                            .map(ToString::to_string)
                    })
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "download.bin".into());
                *save_path = dl_dir.join(&fname);
                let _ = ev_dl_start.send_event(UserEvent::DownloadStarted {
                    id: format!("dl-{}", now_ts()),
                    url: dl_url,
                    filename: fname,
                    save_path: save_path.to_string_lossy().to_string(),
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

            // Permissions matérielles fail-closed (micro, caméra, géolocalisation, notifications)
            gtk_wv.connect_permission_request(move |_view, req| {
                req.deny();
                true
            });

            // Réception des blocages HTTPS Niveau 2 envoyés par liborakle_webkit_ext.so
            let ev_ext = self.event_proxy.clone();
            gtk_wv.connect_user_message_received(move |_view, msg| {
                if msg.name().as_deref() == Some("orakle-shield-blocked") {
                    if let Some(var) = msg.parameters() {
                        if let Some(u) = var.str() {
                            let _ = ev_ext
                                .send_event(UserEvent::NativeAdapterBlocked { url: u.to_string() });
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
            self.tabs[pos].wv = None; // Détruit la WebView + WebKitWebProcess associé
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
                let wv = self.create_webview_for_tab(tab_id, &url, incog);
                self.tabs[pos].wv = Some(wv);
                self.tabs[pos].is_suspended = false;
                self.tabs[pos].is_loading = true;
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

    fn close_tab(&mut self, tab_id: &str) {
        if self
            .tabs
            .iter()
            .filter(|t| t.space_id == self.active_space_id)
            .count()
            <= 1
        {
            return;
        }
        if let Some(idx) = self.tabs.iter().position(|t| t.id == tab_id) {
            let removed = self.tabs.remove(idx);
            let _ = self.profile.storage.delete_tab(&removed.id);
            self.closed_tabs.push(ClosedTab {
                space_id: removed.space_id,
                url: removed.url,
                title: removed.title,
                is_incognito: removed.is_incognito,
            });
            if self.left_tab_id.as_deref() == Some(tab_id) {
                self.left_tab_id = self
                    .tabs
                    .iter()
                    .find(|t| t.space_id == self.active_space_id)
                    .map(|t| t.id.clone());
            }
            if self.right_tab_id.as_deref() == Some(tab_id) {
                self.right_tab_id = self
                    .tabs
                    .iter()
                    .find(|t| Some(&t.id) != self.left_tab_id.as_ref())
                    .map(|t| t.id.clone());
                if self.right_tab_id.is_none() {
                    self.split_active = false;
                }
            }
            if !self.tabs.iter().any(|t| t.is_incognito) {
                if let Some(priv_sess) = self.private_session.take() {
                    let _ = priv_sess.cleanup();
                }
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
            theme: self.theme.clone(),
            shield_enabled: self.shield.is_enabled(),
            shield_blocked: tel.blocked_by_shield,
            can_go_back: can_back,
            can_go_forward: can_fwd,
            closed_count: self.closed_tabs.len(),
            downloads_count: self
                .profile
                .storage
                .get_downloads()
                .map(|v| v.len())
                .unwrap_or(0),
        };

        if let Ok(json) = serde_json::to_string(&snapshot) {
            let _ = self
                .chrome_wv
                .evaluate_script(&format!("window.applyOrakleState({})", json));
        }

        if self.panel_open {
            let panel_payload = serde_json::json!({
                "section": self.panel_section,
                "theme": self.theme,
                "shield_enabled": self.shield.is_enabled(),
                "proxy_addr": format!("127.0.0.1:{}", self.proxy_port),
                "telemetry": tel,
                "exceptions": self.shield.exception_domains(),
                "adapters": platform_capability_matrix(),
                "bookmarks": self.profile.storage.get_bookmarks().unwrap_or_default(),
                "history": self.profile.storage.get_history(80).unwrap_or_default(),
                "downloads": self.profile.storage.get_downloads().unwrap_or_default(),
                "spaces": self.spaces,
                "active_space_id": self.active_space_id,
                "sessions": self.profile.storage.get_sessions().unwrap_or_default(),
                "notes": self.profile.storage.get_notes().unwrap_or_default(),
                "last_pulse": self.last_pulse,
                "search_engine": self.search_engine,
                "profile_name": self.profile.record.name,
                "profile_dir": self.profile.root_dir.to_string_lossy(),
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
            "navigate" => {
                if let Some(u) = msg.payload.get("url").and_then(|v| v.as_str()) {
                    let target = self.normalize_input_url(u);
                    if validate_navigation_url(&target, self.allow_loopback_nav).is_ok() {
                        if let Some(fid) = self.focused_tab_id() {
                            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == fid) {
                                tab.url = target.clone();
                                if let Some(wv) = &tab.wv {
                                    let _ = wv.load_url(&target);
                                }
                            }
                        }
                    }
                    self.sync_ui();
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
                let url = self
                    .demo_base_url
                    .as_ref()
                    .map(|b| format!("{}/home", b))
                    .unwrap_or_else(|| "https://doc.rust-lang.org/".into());
                self.open_new_tab(&url, incog, true);
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
                    }
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
                    self.theme = if th == "light" {
                        "light".into()
                    } else {
                        "dark".into()
                    };
                    let _ = self.profile.storage.set_setting("theme", &self.theme);
                    self.sync_ui();
                }
            }
            "set_search_engine" => {
                if let Some(se) = msg.payload.get("engine").and_then(|v| v.as_str()) {
                    self.search_engine = se.to_string();
                    let _ = self
                        .profile
                        .storage
                        .set_setting("search_engine", &self.search_engine);
                    self.sync_ui();
                }
            }
            "purge_profile_data" => {
                for t in &self.tabs {
                    if let Some(wv) = &t.wv {
                        let _ = wv.clear_all_browsing_data();
                    }
                }
                let _ = self.profile.purge_all_local_data();
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
    let bench_mode = args.iter().any(|a| a == "--bench");
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
        let (proxy, rx) = SecureLoopbackProxy::new(ProxyConfig::default(), shield.clone());
        let addr = rt.block_on(proxy.start(rx)).expect("bench proxy");
        bench::run_memory_benchmark_suite(addr.port(), runs);
        proxy.shutdown();
        return;
    }

    let base_data_dir = if verify_mode {
        tempfile::tempdir().expect("verify profile tempdir").keep()
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".local/share/orakle-browser")
    };

    let profile = ProfileContext::open_persistent(&base_data_dir, "default", "Profil Principal")
        .expect("open default profile");
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

    let (demo_addr_opt, proxy_addr, proxy) = rt.block_on(async {
        let demo = if verify_mode {
            Some(spawn_local_demo_server().await)
        } else {
            None
        };
        let mut cfg = ProxyConfig::default();
        if let Some(d) = demo {
            cfg.allowed_ports = vec![80, 443, 8080, d.port()];
            cfg.allow_loopback_upstream = true;
        }
        let (proxy, rx) = SecureLoopbackProxy::new(cfg, shield.clone());
        let addr = proxy.start(rx).await.expect("start loopback proxy");
        (demo, addr, Arc::new(proxy))
    });

    let download_dir = profile.root_dir.join("downloads");
    let _ = fs::create_dir_all(&download_dir);

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let event_proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Orakle Browser V2")
        .with_inner_size(LogicalSize::new(1360.0, 860.0))
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

    #[allow(unused_mut)]
    let mut normal_ctx = WebContext::new(Some(profile.webview_data_dir.clone()));

    let chrome_ipc = event_proxy.clone();
    let chrome_builder = WebViewBuilder::new()
        .with_html(CHROME_HTML)
        .with_bounds(make_rect(0, 0, 1360, TOP_BASE_HEIGHT))
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

    let panel_ipc = event_proxy.clone();
    let panel_builder = WebViewBuilder::new()
        .with_html(PANEL_HTML)
        .with_bounds(make_rect(
            1000,
            TOP_BASE_HEIGHT,
            PANEL_WIDTH,
            860 - TOP_BASE_HEIGHT,
        ))
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

    let saved_theme = profile
        .storage
        .get_setting("theme")
        .ok()
        .flatten()
        .unwrap_or_else(|| "dark".into());
    let saved_search = profile
        .storage
        .get_setting("search_engine")
        .ok()
        .flatten()
        .unwrap_or_else(|| "duckduckgo".into());

    let demo_base_url = demo_addr_opt.map(|d| format!("http://{}", d));
    let mut host = BrowserHost {
        rt,
        profile,
        private_session: None,
        shield,
        proxy,
        proxy_port: proxy_addr.port(),
        rules_file_path,
        ext_dir_opt: ext_dir_opt.clone(),
        download_dir,
        allow_loopback_nav: demo_base_url.is_some(),
        demo_base_url: demo_base_url.clone(),
        window,
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
        theme: saved_theme,
        search_engine: saved_search,
        last_pulse: None,
        win_w: 1360,
        win_h: 860,
        next_id: 0,
    };

    let initial_url = demo_base_url
        .as_ref()
        .map(|b| format!("{}/home", b))
        .unwrap_or_else(|| "https://doc.rust-lang.org/".into());
    host.open_new_tab(&initial_url, false, true);

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
                host.win_w = logical.width;
                host.win_h = logical.height;
                host.relayout();
            }
            Event::UserEvent(UserEvent::IpcCommand(raw)) => {
                host.handle_ipc(&raw);
            }
            Event::UserEvent(UserEvent::NewTabFromWeb(u)) => {
                host.open_new_tab(&u, false, true);
            }
            Event::UserEvent(UserEvent::WebTitleChanged { tab_id, title }) => {
                if let Some(idx) = host.tabs.iter().position(|t| t.id == tab_id) {
                    host.tabs[idx].title = title;
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
            Event::UserEvent(UserEvent::NativeAdapterBlocked { url }) => {
                host.proxy.record_native_adapter_block(&url, None);
                host.sync_ui();
            }
            Event::UserEvent(UserEvent::DownloadStarted {
                id,
                url,
                filename,
                save_path,
            }) => {
                let _ = host.profile.storage.upsert_download(&DownloadRecord {
                    id,
                    url,
                    filename,
                    save_path,
                    status: "En cours".into(),
                    bytes_received: 0,
                    started_at: now_ts(),
                });
                host.sync_ui();
            }
            Event::UserEvent(UserEvent::DownloadCompleted { url, success }) => {
                if let Ok(list) = host.profile.storage.get_downloads() {
                    if let Some(mut d) = list.into_iter().find(|d| d.url == url) {
                        d.status = if success { "Terminé".into() } else { "Échec".into() };
                        let _ = host.profile.storage.upsert_download(&d);
                    }
                }
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
                    host.theme = "light".into();
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
            _ => {}
        }
    });
}
