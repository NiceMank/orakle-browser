use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use url::Url;

use crate::error::{OrakleError, Result};
use crate::shield::ShieldEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationOrigin {
    UserOmnibox,
    WebContent,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProxyHealth {
    Stopped,
    Running,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShieldLogEntry {
    pub timestamp_ms: u64,
    pub layer: String,
    pub host_and_path: String,
    pub resource_type: String,
    pub rule: Option<String>,
    pub reason: String,
}

/// Nettoie une URL pour le journal de protection : supprime identifiants, query strings (tokens/PII),
/// fragments et tronque les chemins longs afin de ne jamais stocker de données sensibles.
pub fn sanitize_url_for_log(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Ok(parsed) = Url::parse(trimmed) {
        let host = parsed.host_str().unwrap_or("unknown");
        let port_part = parsed.port().map(|p| format!(":{}", p)).unwrap_or_default();
        let raw_path = parsed.path();
        let clean_path = if raw_path.len() > 48 {
            let truncated: String = raw_path.chars().take(48).collect();
            format!("{}...", truncated)
        } else {
            raw_path.to_string()
        };
        return format!("{}://{}{}{}", parsed.scheme(), host, port_part, clean_path);
    }
    trimmed
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .chars()
        .take(80)
        .collect()
}

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub bind_addr: SocketAddr,
    pub allowed_ports: Vec<u16>,
    pub block_private_ranges: bool,
    /// Uniquement activé dans les tests d'intégration locaux pour cibler un serveur mock 127.0.0.1 / ::1.
    /// Toujours `false` en production.
    pub allow_loopback_upstream: bool,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0),
            allowed_ports: vec![80, 443, 8080, 8443],
            block_private_ranges: true,
            allow_loopback_upstream: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProxyDecision {
    Allowed,
    BlockedByShield(Option<String>),
    BlockedSsrf(String),
    BlockedPort(u16),
    UpstreamError(String),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProxyEvent {
    pub method: String,
    pub destination: String,
    pub layer: String,
    pub blocked: bool,
    pub rule: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ProxyTelemetry {
    pub total_requests: u64,
    pub allowed_requests: u64,
    pub blocked_by_shield: u64,
    pub blocked_ssrf: u64,
    pub blocked_port: u64,
    pub upstream_errors: u64,
    pub http_requests: u64,
    pub connect_requests: u64,
    pub socks5_requests: u64,
    pub native_adapter_blocks: u64,
    /// Octets réellement relayés à travers le proxy (zéro estimation heuristique)
    pub bytes_proxied: u64,
    pub events: Vec<ProxyEvent>,
    pub shield_logs: Vec<ShieldLogEntry>,
}

pub struct SecureLoopbackProxy {
    config: ProxyConfig,
    shield: Arc<ShieldEngine>,
    shutdown_tx: watch::Sender<bool>,
    telemetry: Arc<Mutex<ProxyTelemetry>>,
    running: Arc<AtomicBool>,
    allowed_dev_ports: Arc<RwLock<HashSet<u16>>>,
}

impl SecureLoopbackProxy {
    pub fn new(config: ProxyConfig, shield: Arc<ShieldEngine>) -> (Self, watch::Receiver<bool>) {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        (
            Self {
                config,
                shield,
                shutdown_tx,
                telemetry: Arc::new(Mutex::new(ProxyTelemetry::default())),
                running: Arc::new(AtomicBool::new(false)),
                allowed_dev_ports: Arc::new(RwLock::new(HashSet::new())),
            },
            shutdown_rx,
        )
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn health_status(&self) -> ProxyHealth {
        if self.is_running() {
            ProxyHealth::Running
        } else {
            ProxyHealth::Stopped
        }
    }

    pub fn authorize_user_localhost_port(&self, port: u16) {
        if let Ok(mut guard) = self.allowed_dev_ports.write() {
            guard.insert(port);
        }
    }

    pub fn revoke_user_localhost_port(&self, port: u16) {
        if let Ok(mut guard) = self.allowed_dev_ports.write() {
            guard.remove(&port);
        }
    }

    pub fn is_dev_port_authorized(&self, port: u16) -> bool {
        self.allowed_dev_ports
            .read()
            .map(|g| g.contains(&port))
            .unwrap_or(false)
    }

    pub fn telemetry_snapshot(&self) -> ProxyTelemetry {
        self.telemetry.lock().expect("telemetry lock").clone()
    }

    /// Enregistre un blocage réellement effectué par un adaptateur natif OS (ex: WebKitWebExtension send-request)
    pub fn record_native_adapter_block(&self, url: &str, rule: Option<String>) {
        self.record_native_adapter_block_detailed(url, "other", rule);
    }

    pub fn record_native_adapter_block_detailed(
        &self,
        url: &str,
        resource_type: &str,
        rule: Option<String>,
    ) {
        let clean_dest = sanitize_url_for_log(url);
        if let Ok(mut guard) = self.telemetry.lock() {
            guard.total_requests += 1;
            guard.blocked_by_shield += 1;
            guard.native_adapter_blocks += 1;
            let layer = "WebKitWebExtension (Niveau 2)".to_string();
            let detail = "Bloque par adaptateur natif send-request".to_string();
            push_bounded_event(
                &mut guard.events,
                ProxyEvent {
                    method: "SUBRESOURCE".into(),
                    destination: clean_dest.clone(),
                    layer: layer.clone(),
                    blocked: true,
                    rule: rule.clone(),
                    detail: detail.clone(),
                },
            );
            push_bounded_shield_log(
                &mut guard.shield_logs,
                ShieldLogEntry {
                    timestamp_ms: now_epoch_ms(),
                    layer,
                    host_and_path: clean_dest,
                    resource_type: resource_type.to_string(),
                    rule,
                    reason: detail,
                },
            );
        }
    }

    /// Démarre le serveur proxy (HTTP + HTTPS CONNECT + SOCKS5) sur loopback uniquement
    pub async fn start(&self, mut shutdown_rx: watch::Receiver<bool>) -> Result<SocketAddr> {
        if !self.config.bind_addr.ip().is_loopback() {
            return Err(OrakleError::SecurityViolation(
                "Le proxy Orakle refuse formellement de s'attacher à une interface non-loopback"
                    .into(),
            ));
        }

        let listener = TcpListener::bind(self.config.bind_addr).await?;
        let local_addr = listener.local_addr()?;
        let shield = self.shield.clone();
        let config = self.config.clone();
        let telemetry = self.telemetry.clone();
        let running = self.running.clone();
        let dev_ports = self.allowed_dev_ports.clone();
        running.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept_res = listener.accept() => {
                        match accept_res {
                            Ok((client_stream, _client_addr)) => {
                                let shield_ref = shield.clone();
                                let cfg_ref = config.clone();
                                let tel_ref = telemetry.clone();
                                let dev_ref = dev_ports.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handle_client(client_stream, shield_ref, cfg_ref, tel_ref, dev_ref).await {
                                        tracing::debug!("Proxy connection closed/error : {:?}", e);
                                    }
                                });
                            }
                            Err(e) => {
                                tracing::warn!("Proxy accept error : {:?}", e);
                            }
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            tracing::info!("Arrêt propre du proxy Orakle Loopback");
                            running.store(false, Ordering::SeqCst);
                            break;
                        }
                    }
                }
            }
            running.store(false, Ordering::SeqCst);
        });

        Ok(local_addr)
    }

    /// Déclenche l'arrêt propre de toutes les écoutes du proxy
    pub fn shutdown(&self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = self.shutdown_tx.send(true);
    }
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn push_bounded_event(events: &mut Vec<ProxyEvent>, event: ProxyEvent) {
    const MAX_EVENTS: usize = 250;
    if events.len() >= MAX_EVENTS {
        events.remove(0);
    }
    events.push(event);
}

fn push_bounded_shield_log(logs: &mut Vec<ShieldLogEntry>, entry: ShieldLogEntry) {
    const MAX_SHIELD_LOGS: usize = 200;
    if logs.len() >= MAX_SHIELD_LOGS {
        logs.remove(0);
    }
    logs.push(entry);
}

fn record_event(
    telemetry: &Arc<Mutex<ProxyTelemetry>>,
    method: &str,
    destination: &str,
    layer: &str,
    decision: ProxyDecision,
) {
    if let Ok(mut guard) = telemetry.lock() {
        guard.total_requests += 1;
        if method.eq_ignore_ascii_case("SOCKS5") {
            guard.socks5_requests += 1;
        } else if method.eq_ignore_ascii_case("CONNECT") {
            guard.connect_requests += 1;
        } else {
            guard.http_requests += 1;
        }

        let (blocked, rule, detail) = match &decision {
            ProxyDecision::Allowed => {
                guard.allowed_requests += 1;
                (false, None, "Autorise".to_string())
            }
            ProxyDecision::BlockedByShield(r) => {
                guard.blocked_by_shield += 1;
                (true, r.clone(), "Bloque par Orakle Shield".to_string())
            }
            ProxyDecision::BlockedSsrf(ip) => {
                guard.blocked_ssrf += 1;
                (true, None, format!("SSRF bloque ({})", ip))
            }
            ProxyDecision::BlockedPort(p) => {
                guard.blocked_port += 1;
                (true, None, format!("Port bloque ({})", p))
            }
            ProxyDecision::UpstreamError(msg) => {
                guard.upstream_errors += 1;
                (true, None, msg.clone())
            }
        };

        let clean_dest = sanitize_url_for_log(destination);
        push_bounded_event(
            &mut guard.events,
            ProxyEvent {
                method: method.to_string(),
                destination: clean_dest.clone(),
                layer: layer.to_string(),
                blocked,
                rule: rule.clone(),
                detail: detail.clone(),
            },
        );
        if blocked {
            push_bounded_shield_log(
                &mut guard.shield_logs,
                ShieldLogEntry {
                    timestamp_ms: now_epoch_ms(),
                    layer: layer.to_string(),
                    host_and_path: clean_dest,
                    resource_type: method.to_ascii_lowercase(),
                    rule,
                    reason: detail,
                },
            );
        }
    }
}

fn add_proxied_bytes(telemetry: &Arc<Mutex<ProxyTelemetry>>, bytes: u64) {
    if let Ok(mut guard) = telemetry.lock() {
        guard.bytes_proxied = guard.bytes_proxied.saturating_add(bytes);
    }
}

/// Contrôle anti-SSRF strict : bloque RFC1918, loopback, link-local/metadata, CGNAT, ULA et IPv4-mapped IPv6
pub fn is_forbidden_private_ip(ip: IpAddr, allow_loopback: bool) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_loopback() {
                return !allow_loopback;
            }
            let octets = v4.octets();
            v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_unspecified()
                || (octets[0] == 100 && (octets[1] & 0b1100_0000) == 64)
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
                || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
                || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
                || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
                || octets[0] >= 240
        }
        IpAddr::V6(v6) => {
            if let Some(mapped_v4) = v6.to_ipv4_mapped() {
                return is_forbidden_private_ip(IpAddr::V4(mapped_v4), allow_loopback);
            }
            if v6.is_loopback() {
                return !allow_loopback;
            }
            v6.is_multicast()
                || v6.is_unspecified()
                || v6.is_unicast_link_local()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
        }
    }
}

/// Vérifie qu'une URL de navigation WebView distante est sûre (pas de file://, javascript:, ni IP privée directe)
pub fn is_host_loopback(host: &str) -> bool {
    let trimmed = host.trim().trim_matches(['[', ']']).trim_end_matches('.');
    let lower = trimmed.to_ascii_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") {
        return true;
    }
    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(v4) => v4.is_loopback(),
            IpAddr::V6(v6) => {
                v6.is_loopback()
                    || v6
                        .to_ipv4_mapped()
                        .map(|v4| v4.is_loopback())
                        .unwrap_or(false)
            }
        };
    }
    false
}

/// Vérifie si une page Web distante tente d'accéder silencieusement à une ressource loopback/privée (SSRF cross-origin).
/// Une page déjà servie sur loopback (développement local) conserve l'accès à loopback mais jamais aux plages RFC1918/metadata.
pub fn is_cross_site_local_ssrf(url_str: &str, source_str: &str) -> bool {
    let Ok(dest) = Url::parse(url_str) else {
        return false;
    };
    let Some(dest_host) = dest.host_str() else {
        return false;
    };
    let dest_is_loopback = is_host_loopback(dest_host);
    let dest_is_forbidden = dest_host
        .trim_matches(['[', ']'])
        .parse::<IpAddr>()
        .map(|ip| is_forbidden_private_ip(ip, false))
        .unwrap_or(dest_is_loopback);
    if !dest_is_forbidden {
        return false;
    }
    if dest_is_loopback {
        if let Ok(src) = Url::parse(source_str) {
            if let Some(src_host) = src.host_str() {
                if is_host_loopback(src_host) {
                    return false;
                }
            }
        }
    }
    true
}

pub fn validate_navigation_url(raw_url: &str, allow_loopback: bool) -> Result<()> {
    let origin = if allow_loopback {
        NavigationOrigin::UserOmnibox
    } else {
        NavigationOrigin::WebContent
    };
    validate_navigation_with_origin(raw_url, origin)
}

pub fn validate_navigation_with_origin(raw_url: &str, origin: NavigationOrigin) -> Result<()> {
    if raw_url.chars().any(|c| c.is_control()) {
        return Err(OrakleError::SecurityViolation(
            "URL rejetée : présence de caractères de contrôle / CRLF".into(),
        ));
    }
    let trimmed = raw_url.trim();
    if trimmed == "about:blank" {
        return Ok(());
    }
    let parsed = Url::parse(trimmed)
        .map_err(|e| OrakleError::SecurityViolation(format!("URL invalide : {}", e)))?;

    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(OrakleError::SecurityViolation(format!(
                "Schéma bloqué dans WebView distante : {}",
                other
            )));
        }
    }

    let allow_loopback = matches!(origin, NavigationOrigin::UserOmnibox);
    let host = parsed
        .host_str()
        .ok_or_else(|| OrakleError::SecurityViolation("URL rejetée : hôte manquant".into()))?;
    if host.is_empty() || host.chars().any(|c| c.is_whitespace()) {
        return Err(OrakleError::SecurityViolation(
            "URL rejetée : hôte malformé".into(),
        ));
    }
    let host_lower = host.trim_end_matches('.').to_ascii_lowercase();
    if !allow_loopback && (host_lower == "localhost" || host_lower.ends_with(".localhost")) {
        return Err(OrakleError::SecurityViolation(format!(
            "Navigation bloquée vers hôte loopback depuis contenu Web : {}",
            host
        )));
    }
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
        if is_forbidden_private_ip(ip, allow_loopback) {
            return Err(OrakleError::SecurityViolation(format!(
                "Navigation bloquée vers IP privée/interne : {}",
                ip
            )));
        }
    }
    Ok(())
}

fn is_dev_port_open(dev_ports: &RwLock<HashSet<u16>>, port: u16) -> bool {
    dev_ports.read().map(|g| g.contains(&port)).unwrap_or(false)
}

fn check_port_and_literal_ip(
    host: &str,
    port: u16,
    config: &ProxyConfig,
    dev_ports: &RwLock<HashSet<u16>>,
) -> std::result::Result<Option<SocketAddr>, (ProxyDecision, OrakleError)> {
    let loopback_host = is_host_loopback(host);
    let dev_authorized = loopback_host && is_dev_port_open(dev_ports, port);
    if !config.allowed_ports.contains(&port) && !dev_authorized {
        return Err((
            ProxyDecision::BlockedPort(port),
            OrakleError::SecurityViolation(format!("Port non autorisé : {}", port)),
        ));
    }

    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
        let allow_lb = config.allow_loopback_upstream || dev_authorized;
        if config.block_private_ranges && is_forbidden_private_ip(ip, allow_lb) {
            return Err((
                ProxyDecision::BlockedSsrf(ip.to_string()),
                OrakleError::SecurityViolation(format!("SSRF bloqué vers IP privée : {}", ip)),
            ));
        }
        return Ok(Some(SocketAddr::new(ip, port)));
    }

    Ok(None)
}

async fn resolve_and_validate_upstream(
    host: &str,
    port: u16,
    config: &ProxyConfig,
    dev_ports: &RwLock<HashSet<u16>>,
) -> std::result::Result<SocketAddr, (ProxyDecision, OrakleError)> {
    let dev_authorized = is_host_loopback(host) && is_dev_port_open(dev_ports, port);
    let allow_lb = config.allow_loopback_upstream || dev_authorized;
    let lookup = tokio::net::lookup_host((host, port)).await.map_err(|e| {
        let msg = format!("Échec résolution DNS pour {}:{} : {}", host, port, e);
        (
            ProxyDecision::UpstreamError(msg.clone()),
            OrakleError::Network(msg),
        )
    })?;

    let mut chosen: Option<SocketAddr> = None;
    for addr in lookup {
        if config.block_private_ranges && is_forbidden_private_ip(addr.ip(), allow_lb) {
            return Err((
                ProxyDecision::BlockedSsrf(addr.ip().to_string()),
                OrakleError::SecurityViolation(format!(
                    "SSRF bloqué (résolution DNS {} -> {})",
                    host,
                    addr.ip()
                )),
            ));
        }
        if chosen.is_none() {
            chosen = Some(addr);
        }
    }

    chosen.ok_or_else(|| {
        let msg = format!("Aucune adresse résolue pour {}", host);
        (
            ProxyDecision::UpstreamError(msg.clone()),
            OrakleError::Network(msg),
        )
    })
}

fn is_external_origin_targeting_loopback(target_host: &str, head_str: &str) -> bool {
    if !is_host_loopback(target_host) {
        return false;
    }
    for line in head_str.split("\r\n").skip(1) {
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim();
            if key.eq_ignore_ascii_case("origin") || key.eq_ignore_ascii_case("referer") {
                let val = v.trim();
                if val.eq_ignore_ascii_case("null") {
                    return true;
                }
                if let Ok(parsed) = Url::parse(val) {
                    if let Some(src_host) = parsed.host_str() {
                        if !is_host_loopback(src_host) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

async fn read_http_head(stream: &mut TcpStream) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 2048];
    const MAX_HEADER_BYTES: usize = 16 * 1024;

    loop {
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            return Ok((buf, Vec::new()));
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let split_idx = pos + 4;
            let body_rest = buf.split_off(split_idx);
            return Ok((buf, body_rest));
        }
        if buf.len() > MAX_HEADER_BYTES {
            return Err(OrakleError::Proxy(
                "En-tête HTTP trop volumineux (>16 Ko)".into(),
            ));
        }
    }
}

fn infer_resource_type(url: &str, headers_lower: &str) -> &'static str {
    let path = url.split('?').next().unwrap_or(url).to_ascii_lowercase();
    if path.ends_with(".js") || path.ends_with(".mjs") {
        "script"
    } else if path.ends_with(".css") {
        "stylesheet"
    } else if path.ends_with(".png")
        || path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".gif")
        || path.ends_with(".svg")
        || path.ends_with(".webp")
        || path.ends_with(".ico")
    {
        "image"
    } else if headers_lower.contains("upgrade: websocket") {
        "websocket"
    } else if path.contains("/api/") || headers_lower.contains("sec-fetch-dest: empty") {
        "xmlhttprequest"
    } else {
        "main_frame"
    }
}

async fn write_error_and_close(client: &mut TcpStream, status_line: &str, body: &str) {
    let resp = format!(
        "HTTP/1.1 {}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status_line,
        body.len(),
        body
    );
    let _ = client.write_all(resp.as_bytes()).await;
    let _ = client.shutdown().await;
}

async fn handle_upstream_rejection(
    client: &mut TcpStream,
    telemetry: &Arc<Mutex<ProxyTelemetry>>,
    method: &str,
    dest: &str,
    layer: &str,
    decision: ProxyDecision,
    err: OrakleError,
) -> OrakleError {
    record_event(telemetry, method, dest, layer, decision.clone());
    let msg = match decision {
        ProxyDecision::BlockedPort(_) => ("403 Forbidden (Port non autorise)", "Port non autorise"),
        ProxyDecision::BlockedSsrf(_) => (
            "403 Forbidden (Adresse IP privee interdite)",
            "Adresse IP privee interdite",
        ),
        _ => ("502 Bad Gateway", "Erreur resolution amont"),
    };
    write_error_and_close(client, msg.0, msg.1).await;
    err
}

async fn handle_client(
    client: TcpStream,
    shield: Arc<ShieldEngine>,
    config: ProxyConfig,
    telemetry: Arc<Mutex<ProxyTelemetry>>,
    dev_ports: Arc<RwLock<HashSet<u16>>>,
) -> Result<()> {
    let mut first = [0u8; 1];
    let n = client.peek(&mut first).await?;
    if n == 0 {
        return Ok(());
    }

    if first[0] == 0x05 {
        handle_socks5_client(client, shield, config, telemetry, dev_ports).await
    } else {
        handle_http_client(client, shield, config, telemetry, dev_ports).await
    }
}

/// Implémentation complète SOCKS5 (RFC 1928) sans authentification, fail-closed, anti-SSRF & Shield pré-DNS
async fn handle_socks5_client(
    mut client: TcpStream,
    shield: Arc<ShieldEngine>,
    config: ProxyConfig,
    telemetry: Arc<Mutex<ProxyTelemetry>>,
    dev_ports: Arc<RwLock<HashSet<u16>>>,
) -> Result<()> {
    let mut greet = [0u8; 2];
    client.read_exact(&mut greet).await?;
    if greet[0] != 0x05 {
        return Err(OrakleError::Proxy("Version SOCKS5 invalide".into()));
    }
    let nmethods = greet[1] as usize;
    let mut methods = vec![0u8; nmethods];
    client.read_exact(&mut methods).await?;
    if !methods.contains(&0x00) {
        let _ = client.write_all(&[0x05, 0xFF]).await;
        return Err(OrakleError::Proxy(
            "Méthode d'authentification SOCKS5 non supportée".into(),
        ));
    }
    client.write_all(&[0x05, 0x00]).await?;

    // Lecture requête : VER(1), CMD(1), RSV(1), ATYP(1)
    let mut req_head = [0u8; 4];
    client.read_exact(&mut req_head).await?;
    if req_head[0] != 0x05 || req_head[1] != 0x01 {
        // Seul CONNECT (0x01) est supporté
        let _ = client
            .write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await;
        return Err(OrakleError::Proxy("Commande SOCKS5 non supportée".into()));
    }

    let (host, literal_ip): (String, Option<IpAddr>) = match req_head[3] {
        0x01 => {
            let mut b = [0u8; 4];
            client.read_exact(&mut b).await?;
            let ip = IpAddr::V4(Ipv4Addr::from(b));
            (ip.to_string(), Some(ip))
        }
        0x03 => {
            let mut len_buf = [0u8; 1];
            client.read_exact(&mut len_buf).await?;
            let mut dom = vec![0u8; len_buf[0] as usize];
            client.read_exact(&mut dom).await?;
            let s = String::from_utf8(dom)
                .map_err(|_| OrakleError::Proxy("Nom de domaine SOCKS5 non UTF-8".into()))?;
            (s, None)
        }
        0x04 => {
            let mut b = [0u8; 16];
            client.read_exact(&mut b).await?;
            let ip = IpAddr::V6(Ipv6Addr::from(b));
            (format!("[{}]", ip), Some(ip))
        }
        _ => {
            let _ = client
                .write_all(&[0x05, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await;
            return Err(OrakleError::Proxy("ATYP SOCKS5 non supporté".into()));
        }
    };

    let mut port_buf = [0u8; 2];
    client.read_exact(&mut port_buf).await?;
    let port = u16::from_be_bytes(port_buf);
    let dest_label = format!("{}:{}", host, port);

    // 1. Port + IP littérale SSRF (zéro fuite DNS)
    let literal_addr = match check_port_and_literal_ip(&host, port, &config, &dev_ports) {
        Ok(opt) => opt.or_else(|| {
            literal_ip.and_then(|ip| {
                let allow_lb = config.allow_loopback_upstream
                    || (is_host_loopback(&host) && is_dev_port_open(&dev_ports, port));
                if config.block_private_ranges && is_forbidden_private_ip(ip, allow_lb) {
                    None
                } else {
                    Some(SocketAddr::new(ip, port))
                }
            })
        }),
        Err((decision, err)) => {
            record_event(
                &telemetry,
                "SOCKS5",
                &dest_label,
                "Proxy SOCKS5 (Niveau 1)",
                decision,
            );
            let _ = client
                .write_all(&[0x05, 0x02, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await;
            return Err(err);
        }
    };

    // 2. Filtrage Orakle Shield au niveau domaine AVANT toute résolution DNS
    let scheme = if port == 443 || port == 8443 {
        "https"
    } else {
        "http"
    };
    let check_url = format!("{}://{}/", scheme, host);
    let decision = shield.check_network_request(&check_url, &check_url, "main_frame", "SOCKS5")?;
    if decision.blocked {
        record_event(
            &telemetry,
            "SOCKS5",
            &check_url,
            "Proxy SOCKS5 (Niveau 1)",
            ProxyDecision::BlockedByShield(decision.rule),
        );
        let _ = client
            .write_all(&[0x05, 0x02, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await;
        return Ok(());
    }

    // 3. Résolution DNS + anti-DNS-rebinding SSRF
    let upstream_addr = match literal_addr {
        Some(addr) => addr,
        None => match resolve_and_validate_upstream(&host, port, &config, &dev_ports).await {
            Ok(addr) => addr,
            Err((decision, err)) => {
                record_event(
                    &telemetry,
                    "SOCKS5",
                    &dest_label,
                    "Proxy SOCKS5 (Niveau 1)",
                    decision.clone(),
                );
                let reply_code = match decision {
                    ProxyDecision::BlockedSsrf(_) | ProxyDecision::BlockedPort(_) => 0x02,
                    _ => 0x04,
                };
                let _ = client
                    .write_all(&[0x05, reply_code, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                    .await;
                return Err(err);
            }
        },
    };

    // 4. Connexion TCP vers l'amont validé
    let mut upstream = match TcpStream::connect(upstream_addr).await {
        Ok(s) => s,
        Err(e) => {
            let err_msg = format!("Echec connexion upstream SOCKS5 {} : {}", upstream_addr, e);
            record_event(
                &telemetry,
                "SOCKS5",
                &dest_label,
                "Proxy SOCKS5 (Niveau 1)",
                ProxyDecision::UpstreamError(err_msg.clone()),
            );
            let _ = client
                .write_all(&[0x05, 0x04, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await;
            return Err(OrakleError::Network(err_msg));
        }
    };

    record_event(
        &telemetry,
        "SOCKS5",
        &dest_label,
        "Proxy SOCKS5 (Niveau 1)",
        ProxyDecision::Allowed,
    );
    client
        .write_all(&[0x05, 0x00, 0x00, 0x01, 127, 0, 0, 1, 0, 0])
        .await?;

    if let Ok((up_b, down_b)) = tokio::io::copy_bidirectional(&mut client, &mut upstream).await {
        add_proxied_bytes(&telemetry, up_b + down_b);
    }

    Ok(())
}

async fn handle_http_client(
    mut client: TcpStream,
    shield: Arc<ShieldEngine>,
    config: ProxyConfig,
    telemetry: Arc<Mutex<ProxyTelemetry>>,
    dev_ports: Arc<RwLock<HashSet<u16>>>,
) -> Result<()> {
    let (head_bytes, trailing_bytes) = read_http_head(&mut client).await?;
    if head_bytes.is_empty() {
        return Ok(());
    }

    let head_str = String::from_utf8_lossy(&head_bytes);
    let mut lines = head_str.split("\r\n");
    let first_line = lines.next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();

    if parts.len() < 3 {
        write_error_and_close(&mut client, "400 Bad Request", "Requete HTTP invalide").await;
        return Err(OrakleError::Proxy("Requête HTTP invalide".into()));
    }

    let method = parts[0];
    let target = parts[1];
    let version = parts[2];

    if method.eq_ignore_ascii_case("CONNECT") {
        let (host, port) = parse_host_port(target)?;

        let literal_addr = match check_port_and_literal_ip(&host, port, &config, &dev_ports) {
            Ok(opt) => opt,
            Err((decision, err)) => {
                return Err(handle_upstream_rejection(
                    &mut client,
                    &telemetry,
                    method,
                    target,
                    "Proxy CONNECT (Niveau 1)",
                    decision,
                    err,
                )
                .await);
            }
        };

        let check_url = format!("https://{}/", host);
        let decision =
            shield.check_network_request(&check_url, &check_url, "main_frame", "CONNECT")?;
        if decision.blocked {
            tracing::info!("[PROXY SHIELD BLOQUÉ CONNECT] {}", host);
            record_event(
                &telemetry,
                method,
                &check_url,
                "Proxy CONNECT (Niveau 1)",
                ProxyDecision::BlockedByShield(decision.rule.clone()),
            );
            write_error_and_close(
                &mut client,
                "403 Forbidden (Bloque par Orakle Shield)",
                "Bloque par Orakle Shield",
            )
            .await;
            return Ok(());
        }

        let upstream_addr = match literal_addr {
            Some(addr) => addr,
            None => match resolve_and_validate_upstream(&host, port, &config, &dev_ports).await {
                Ok(addr) => addr,
                Err((decision, err)) => {
                    return Err(handle_upstream_rejection(
                        &mut client,
                        &telemetry,
                        method,
                        target,
                        "Proxy CONNECT (Niveau 1)",
                        decision,
                        err,
                    )
                    .await);
                }
            },
        };

        let mut upstream = match TcpStream::connect(upstream_addr).await {
            Ok(s) => s,
            Err(e) => {
                let err_msg = format!("Echec connexion upstream {} : {}", upstream_addr, e);
                record_event(
                    &telemetry,
                    method,
                    target,
                    "Proxy CONNECT (Niveau 1)",
                    ProxyDecision::UpstreamError(err_msg.clone()),
                );
                write_error_and_close(&mut client, "502 Bad Gateway", &err_msg).await;
                return Err(OrakleError::Network(err_msg));
            }
        };

        record_event(
            &telemetry,
            method,
            target,
            "Proxy CONNECT (Niveau 1)",
            ProxyDecision::Allowed,
        );
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;

        if !trailing_bytes.is_empty() {
            upstream.write_all(&trailing_bytes).await?;
        }

        if let Ok((up_b, down_b)) = tokio::io::copy_bidirectional(&mut client, &mut upstream).await
        {
            add_proxied_bytes(&telemetry, up_b + down_b);
        }
    } else {
        let parsed_url = if target.starts_with("http://") || target.starts_with("https://") {
            Url::parse(target)?
        } else {
            let host_header = lines
                .clone()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    if k.trim().eq_ignore_ascii_case("host") {
                        Some(v.trim())
                    } else {
                        None
                    }
                })
                .ok_or_else(|| OrakleError::Proxy("En-tête Host manquant".into()))?;
            Url::parse(&format!("http://{}{}", host_header, target))?
        };

        let host = parsed_url
            .host_str()
            .ok_or_else(|| OrakleError::Proxy("Hôte absent dans URL proxy".into()))?;
        let port = parsed_url.port_or_known_default().unwrap_or(80);

        let head_lower = head_str.to_ascii_lowercase();
        let source_url = lines
            .clone()
            .find_map(|l| {
                let (k, v) = l.split_once(':')?;
                let key = k.trim();
                if key.eq_ignore_ascii_case("referer") || key.eq_ignore_ascii_case("origin") {
                    Some(v.trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| parsed_url.as_str().to_string());

        let req_type = infer_resource_type(parsed_url.as_str(), &head_lower);

        if is_external_origin_targeting_loopback(host, &head_str) {
            let err = OrakleError::SecurityViolation(format!(
                "Requête cross-origin vers loopback bloquée (CSRF/SSRF) : {}",
                host
            ));
            return Err(handle_upstream_rejection(
                &mut client,
                &telemetry,
                method,
                parsed_url.as_str(),
                "Proxy HTTP (Anti-CSRF Loopback)",
                ProxyDecision::BlockedSsrf(format!("cross-origin->{}", host)),
                err,
            )
            .await);
        }

        let literal_addr = match check_port_and_literal_ip(host, port, &config, &dev_ports) {
            Ok(opt) => opt,
            Err((decision, err)) => {
                return Err(handle_upstream_rejection(
                    &mut client,
                    &telemetry,
                    method,
                    parsed_url.as_str(),
                    "Proxy HTTP (Niveau 1)",
                    decision,
                    err,
                )
                .await);
            }
        };

        let decision =
            shield.check_network_request(parsed_url.as_str(), &source_url, req_type, method)?;
        if decision.blocked {
            tracing::info!("[PROXY SHIELD BLOQUÉ HTTP] {}", parsed_url);
            record_event(
                &telemetry,
                method,
                parsed_url.as_str(),
                "Proxy HTTP (Niveau 1)",
                ProxyDecision::BlockedByShield(decision.rule.clone()),
            );
            write_error_and_close(
                &mut client,
                "403 Forbidden (Bloque par Orakle Shield)",
                "Bloque par Orakle Shield",
            )
            .await;
            return Ok(());
        }

        let upstream_addr = match literal_addr {
            Some(addr) => addr,
            None => match resolve_and_validate_upstream(host, port, &config, &dev_ports).await {
                Ok(addr) => addr,
                Err((decision, err)) => {
                    return Err(handle_upstream_rejection(
                        &mut client,
                        &telemetry,
                        method,
                        parsed_url.as_str(),
                        "Proxy HTTP (Niveau 1)",
                        decision,
                        err,
                    )
                    .await);
                }
            },
        };

        let mut upstream = match TcpStream::connect(upstream_addr).await {
            Ok(s) => s,
            Err(e) => {
                let err_msg = format!("Echec connexion upstream {} : {}", upstream_addr, e);
                record_event(
                    &telemetry,
                    method,
                    parsed_url.as_str(),
                    "Proxy HTTP (Niveau 1)",
                    ProxyDecision::UpstreamError(err_msg.clone()),
                );
                write_error_and_close(&mut client, "502 Bad Gateway", &err_msg).await;
                return Err(OrakleError::Network(err_msg));
            }
        };

        let origin_path = match parsed_url.query() {
            Some(q) => format!("{}?{}", parsed_url.path(), q),
            None => parsed_url.path().to_string(),
        };

        let mut rewritten_head = format!("{} {} {}\r\n", method, origin_path, version);
        for line in lines {
            if line.is_empty() {
                continue;
            }
            if let Some((k, _)) = line.split_once(':') {
                if k.trim().eq_ignore_ascii_case("proxy-connection") {
                    continue;
                }
            }
            rewritten_head.push_str(line);
            rewritten_head.push_str("\r\n");
        }
        rewritten_head.push_str("\r\n");

        record_event(
            &telemetry,
            method,
            parsed_url.as_str(),
            "Proxy HTTP (Niveau 1)",
            ProxyDecision::Allowed,
        );
        upstream.write_all(rewritten_head.as_bytes()).await?;
        if !trailing_bytes.is_empty() {
            upstream.write_all(&trailing_bytes).await?;
        }

        let inspect_redirect = !head_lower.contains("upgrade:")
            && !head_lower.contains("content-length:")
            && !head_lower.contains("transfer-encoding: chunked")
            && trailing_bytes.is_empty();

        if inspect_redirect {
            let (resp_head, resp_rest) = read_http_head(&mut upstream).await?;
            if !resp_head.is_empty() {
                let resp_str = String::from_utf8_lossy(&resp_head);
                let mut resp_lines = resp_str.split("\r\n");
                let status_line = resp_lines.next().unwrap_or("");
                let status_code = status_line.split_whitespace().nth(1).unwrap_or("");
                if matches!(status_code, "301" | "302" | "303" | "307" | "308") {
                    if let Some(loc) = resp_lines.find_map(|l| {
                        let (k, v) = l.split_once(':')?;
                        if k.trim().eq_ignore_ascii_case("location") {
                            Some(v.trim())
                        } else {
                            None
                        }
                    }) {
                        let resolved = parsed_url
                            .join(loc)
                            .map(|u| u.to_string())
                            .unwrap_or_else(|_| loc.to_string());
                        let redirect_origin =
                            if config.allow_loopback_upstream && is_host_loopback(host) {
                                NavigationOrigin::UserOmnibox
                            } else {
                                NavigationOrigin::WebContent
                            };
                        if let Err(err) =
                            validate_navigation_with_origin(&resolved, redirect_origin)
                        {
                            return Err(handle_upstream_rejection(
                                &mut client,
                                &telemetry,
                                method,
                                &resolved,
                                "Proxy HTTP (Anti-SSRF Redirect)",
                                ProxyDecision::BlockedSsrf(sanitize_url_for_log(&resolved)),
                                err,
                            )
                            .await);
                        }
                    }
                }
                client.write_all(&resp_head).await?;
                if !resp_rest.is_empty() {
                    client.write_all(&resp_rest).await?;
                }
                add_proxied_bytes(&telemetry, (resp_head.len() + resp_rest.len()) as u64);
            }
        }

        if let Ok((up_b, down_b)) = tokio::io::copy_bidirectional(&mut client, &mut upstream).await
        {
            add_proxied_bytes(&telemetry, up_b + down_b);
        }
    }

    Ok(())
}

fn parse_host_port(target: &str) -> Result<(String, u16)> {
    if target.starts_with('[') {
        if let Some(end_bracket) = target.find(']') {
            let host = &target[..=end_bracket];
            let rest = &target[end_bracket + 1..];
            if let Some(port_str) = rest.strip_prefix(':') {
                let port: u16 = port_str
                    .parse()
                    .map_err(|_| OrakleError::Proxy(format!("Port invalide dans {}", target)))?;
                return Ok((host.to_string(), port));
            }
            return Ok((host.to_string(), 443));
        }
    }
    if let Some((h, p)) = target.rsplit_once(':') {
        let port: u16 = p
            .parse()
            .map_err(|_| OrakleError::Proxy(format!("Port invalide dans {}", target)))?;
        Ok((h.to_string(), port))
    } else {
        Ok((target.to_string(), 443))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_RULES: &str = "
||googleads.g.doubleclick.net^
||pixel.facebook.com^
||analytics.google.com^
";

    #[test]
    fn test_ssrf_and_navigation_url_validation() {
        assert!(is_forbidden_private_ip("127.0.0.1".parse().unwrap(), false));
        assert!(!is_forbidden_private_ip("127.0.0.1".parse().unwrap(), true));
        assert!(is_forbidden_private_ip("10.0.0.5".parse().unwrap(), false));
        assert!(is_forbidden_private_ip(
            "172.16.0.1".parse().unwrap(),
            false
        ));
        assert!(is_forbidden_private_ip(
            "192.168.1.1".parse().unwrap(),
            false
        ));
        assert!(is_forbidden_private_ip(
            "169.254.169.254".parse().unwrap(),
            false
        ));
        assert!(is_forbidden_private_ip(
            "100.64.0.1".parse().unwrap(),
            false
        ));
        assert!(is_forbidden_private_ip("::1".parse().unwrap(), false));
        assert!(is_forbidden_private_ip(
            "::ffff:127.0.0.1".parse().unwrap(),
            false
        ));
        assert!(is_forbidden_private_ip(
            "::ffff:169.254.169.254".parse().unwrap(),
            false
        ));
        assert!(is_forbidden_private_ip("fc00::1".parse().unwrap(), false));
        assert!(!is_forbidden_private_ip("1.1.1.1".parse().unwrap(), false));
        assert!(!is_forbidden_private_ip(
            "93.184.216.34".parse().unwrap(),
            false
        ));

        assert!(validate_navigation_url("file:///etc/passwd", false).is_err());
        assert!(validate_navigation_url("javascript:alert(1)", false).is_err());
        assert!(validate_navigation_url("http://192.168.1.10/admin", false).is_err());
        assert!(validate_navigation_url("https://doc.rust-lang.org/", false).is_ok());
    }

    #[tokio::test]
    async fn test_secure_loopback_proxy_http_and_socks5_blocking() {
        let shield = Arc::new(ShieldEngine::new_with_rules(TEST_RULES));
        let config = ProxyConfig::default();
        let (proxy, shutdown_rx) = SecureLoopbackProxy::new(config, shield);

        let proxy_addr = proxy.start(shutdown_rx).await.expect("start proxy");
        assert!(proxy_addr.ip().is_loopback());

        // 1. Test HTTP CONNECT bloqué
        let mut client_blocked = TcpStream::connect(proxy_addr).await.expect("connect proxy");
        client_blocked
            .write_all(b"CONNECT googleads.g.doubleclick.net:443 HTTP/1.1\r\nHost: googleads.g.doubleclick.net:443\r\n\r\n")
            .await
            .unwrap();

        let mut resp_buf = [0u8; 512];
        let n = client_blocked.read(&mut resp_buf).await.unwrap();
        let resp_str = String::from_utf8_lossy(&resp_buf[..n]);
        assert!(resp_str.contains("403 Forbidden"));

        // 2. Test SOCKS5 domaine bloqué (pixel.facebook.com:443 -> code 0x02)
        let mut s5_blocked = TcpStream::connect(proxy_addr)
            .await
            .expect("connect socks5");
        s5_blocked.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
        let mut s5_greet = [0u8; 2];
        s5_blocked.read_exact(&mut s5_greet).await.unwrap();
        assert_eq!(s5_greet, [0x05, 0x00]);

        let dom = b"pixel.facebook.com";
        let mut s5_req = vec![0x05, 0x01, 0x00, 0x03, dom.len() as u8];
        s5_req.extend_from_slice(dom);
        s5_req.extend_from_slice(&443u16.to_be_bytes());
        s5_blocked.write_all(&s5_req).await.unwrap();
        let mut s5_rep = [0u8; 10];
        s5_blocked.read_exact(&mut s5_rep).await.unwrap();
        assert_eq!(
            s5_rep[1], 0x02,
            "SOCKS5 doit renvoyer 0x02 (règle) pour un domaine bloqué"
        );

        // 3. Test SSRF HTTP & SOCKS5 IPv6/IPv4
        for target in [
            "192.168.1.1:443",
            "127.0.0.1:443",
            "169.254.169.254:80",
            "[::1]:443",
        ] {
            let mut client_ssrf = TcpStream::connect(proxy_addr).await.expect("connect proxy");
            let req = format!("CONNECT {} HTTP/1.1\r\nHost: {}\r\n\r\n", target, target);
            client_ssrf.write_all(req.as_bytes()).await.unwrap();
            let mut ssrf_buf = [0u8; 512];
            let n_ssrf = client_ssrf.read(&mut ssrf_buf).await.unwrap();
            assert!(String::from_utf8_lossy(&ssrf_buf[..n_ssrf]).contains("403 Forbidden"));
        }

        // SOCKS5 IPv6 ::1 SSRF bloqué
        let mut s5_v6 = TcpStream::connect(proxy_addr)
            .await
            .expect("connect socks5 v6");
        s5_v6.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
        let mut g = [0u8; 2];
        s5_v6.read_exact(&mut g).await.unwrap();
        let mut req_v6 = vec![0x05, 0x01, 0x00, 0x04];
        req_v6.extend_from_slice(&Ipv6Addr::LOCALHOST.octets());
        req_v6.extend_from_slice(&443u16.to_be_bytes());
        s5_v6.write_all(&req_v6).await.unwrap();
        let mut rep_v6 = [0u8; 10];
        s5_v6.read_exact(&mut rep_v6).await.unwrap();
        assert_eq!(rep_v6[1], 0x02);

        let stats = proxy.telemetry_snapshot();
        assert_eq!(stats.blocked_by_shield, 2);
        assert_eq!(stats.blocked_ssrf, 5);
        assert_eq!(stats.socks5_requests, 2);

        proxy.shutdown();
    }
}
