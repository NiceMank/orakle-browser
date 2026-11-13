use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use url::Url;
use crate::error::{OrakleError, Result};
use crate::shield::ShieldEngine;

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub bind_addr: SocketAddr,
    pub allowed_ports: Vec<u16>,
    pub block_private_ranges: bool,
    /// Uniquement activé dans les tests d'intégration locaux pour cibler un serveur mock 127.0.0.1.
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

#[derive(Debug, Clone)]
pub struct ProxyEvent {
    pub method: String,
    pub destination: String,
    pub decision: ProxyDecision,
}

#[derive(Debug, Clone, Default)]
pub struct ProxyTelemetry {
    pub total_requests: u64,
    pub allowed_requests: u64,
    pub blocked_by_shield: u64,
    pub blocked_ssrf: u64,
    pub blocked_port: u64,
    pub upstream_errors: u64,
    pub events: Vec<ProxyEvent>,
}

pub struct SecureLoopbackProxy {
    config: ProxyConfig,
    shield: Arc<ShieldEngine>,
    shutdown_tx: watch::Sender<bool>,
    telemetry: Arc<Mutex<ProxyTelemetry>>,
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
            },
            shutdown_rx,
        )
    }

    pub fn telemetry_snapshot(&self) -> ProxyTelemetry {
        self.telemetry.lock().expect("telemetry lock").clone()
    }

    /// Démarre le serveur proxy en tâche d'arrière-plan Tokio et retourne l'adresse locale écoutée
    pub async fn start(&self, mut shutdown_rx: watch::Receiver<bool>) -> Result<SocketAddr> {
        if !self.config.bind_addr.ip().is_loopback() {
            return Err(OrakleError::SecurityViolation(
                "Le proxy Orakle refuse formellement de s'attacher à une interface non-loopback".into(),
            ));
        }

        let listener = TcpListener::bind(self.config.bind_addr).await?;
        let local_addr = listener.local_addr()?;
        let shield = self.shield.clone();
        let config = self.config.clone();
        let telemetry = self.telemetry.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept_res = listener.accept() => {
                        match accept_res {
                            Ok((client_stream, _client_addr)) => {
                                let shield_ref = shield.clone();
                                let cfg_ref = config.clone();
                                let tel_ref = telemetry.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handle_client(client_stream, shield_ref, cfg_ref, tel_ref).await {
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
                            break;
                        }
                    }
                }
            }
        });

        Ok(local_addr)
    }

    /// Déclenche l'arrêt propre de toutes les écoutes du proxy
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

fn record_event(telemetry: &Arc<Mutex<ProxyTelemetry>>, method: &str, destination: &str, decision: ProxyDecision) {
    if let Ok(mut guard) = telemetry.lock() {
        guard.total_requests += 1;
        match &decision {
            ProxyDecision::Allowed => guard.allowed_requests += 1,
            ProxyDecision::BlockedByShield(_) => guard.blocked_by_shield += 1,
            ProxyDecision::BlockedSsrf(_) => guard.blocked_ssrf += 1,
            ProxyDecision::BlockedPort(_) => guard.blocked_port += 1,
            ProxyDecision::UpstreamError(_) => guard.upstream_errors += 1,
        }
        guard.events.push(ProxyEvent {
            method: method.to_string(),
            destination: destination.to_string(),
            decision,
        });
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
                // CGNAT 100.64.0.0/10
                || (octets[0] == 100 && (octets[1] & 0b1100_0000) == 64)
                // Documentation / Benchmark / Reserved
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
                // ULA fc00::/7
                || (v6.segments()[0] & 0xfe00) == 0xfc00
        }
    }
}

/// Valide le port et vérifie immédiatement si l'hôte est une IP littérale interdite (sans émettre de requête DNS).
fn check_port_and_literal_ip(
    host: &str,
    port: u16,
    config: &ProxyConfig,
) -> std::result::Result<Option<SocketAddr>, (ProxyDecision, OrakleError)> {
    if !config.allowed_ports.contains(&port) {
        return Err((
            ProxyDecision::BlockedPort(port),
            OrakleError::SecurityViolation(format!("Port non autorisé : {}", port)),
        ));
    }

    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
        if config.block_private_ranges && is_forbidden_private_ip(ip, config.allow_loopback_upstream) {
            return Err((
                ProxyDecision::BlockedSsrf(ip.to_string()),
                OrakleError::SecurityViolation(format!("SSRF bloqué vers IP privée : {}", ip)),
            ));
        }
        return Ok(Some(SocketAddr::new(ip, port)));
    }

    Ok(None)
}

/// Résout l'hôte en amont et valide TOUTES les adresses IP avant ouverture du socket (anti-DNS-rebinding)
async fn resolve_and_validate_upstream(
    host: &str,
    port: u16,
    config: &ProxyConfig,
) -> std::result::Result<SocketAddr, (ProxyDecision, OrakleError)> {
    let lookup = tokio::net::lookup_host((host, port)).await.map_err(|e| {
        let msg = format!("Échec résolution DNS pour {}:{} : {}", host, port, e);
        (ProxyDecision::UpstreamError(msg.clone()), OrakleError::Network(msg))
    })?;

    let mut chosen: Option<SocketAddr> = None;
    for addr in lookup {
        if config.block_private_ranges
            && is_forbidden_private_ip(addr.ip(), config.allow_loopback_upstream)
        {
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
        (ProxyDecision::UpstreamError(msg.clone()), OrakleError::Network(msg))
    })
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
            return Err(OrakleError::Proxy("En-tête HTTP trop volumineux (>16 Ko)".into()));
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
    decision: ProxyDecision,
    err: OrakleError,
) -> OrakleError {
    record_event(telemetry, method, dest, decision.clone());
    let msg = match decision {
        ProxyDecision::BlockedPort(_) => ("403 Forbidden (Port non autorise)", "Port non autorise"),
        ProxyDecision::BlockedSsrf(_) => ("403 Forbidden (Adresse IP privee interdite)", "Adresse IP privee interdite"),
        _ => ("502 Bad Gateway", "Erreur resolution amont"),
    };
    write_error_and_close(client, msg.0, msg.1).await;
    err
}

async fn handle_client(
    mut client: TcpStream,
    shield: Arc<ShieldEngine>,
    config: ProxyConfig,
    telemetry: Arc<Mutex<ProxyTelemetry>>,
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

        // 1. Validation port + anti-SSRF sur IP littérale (sans fuite DNS)
        let literal_addr = match check_port_and_literal_ip(&host, port, &config) {
            Ok(opt) => opt,
            Err((decision, err)) => {
                return Err(handle_upstream_rejection(&mut client, &telemetry, method, target, decision, err).await);
            }
        };

        // 2. Filtrage Orakle Shield au niveau domaine/SNI AVANT toute résolution DNS
        let check_url = format!("https://{}/", host);
        let decision = shield.check_network_request(&check_url, &check_url, "main_frame", "CONNECT")?;
        if decision.blocked {
            tracing::info!("[PROXY SHIELD BLOQUÉ CONNECT] {}", host);
            record_event(
                &telemetry,
                method,
                &check_url,
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

        // 3. Résolution DNS + anti-DNS-rebinding SSRF
        let upstream_addr = match literal_addr {
            Some(addr) => addr,
            None => match resolve_and_validate_upstream(&host, port, &config).await {
                Ok(addr) => addr,
                Err((decision, err)) => {
                    return Err(handle_upstream_rejection(&mut client, &telemetry, method, target, decision, err).await);
                }
            },
        };

        // 4. Connexion TCP vers l'IP validée (Zero MITM)
        let mut upstream = match TcpStream::connect(upstream_addr).await {
            Ok(s) => s,
            Err(e) => {
                let err_msg = format!("Echec connexion upstream {} : {}", upstream_addr, e);
                record_event(&telemetry, method, target, ProxyDecision::UpstreamError(err_msg.clone()));
                write_error_and_close(&mut client, "502 Bad Gateway", &err_msg).await;
                return Err(OrakleError::Network(err_msg));
            }
        };

        record_event(&telemetry, method, target, ProxyDecision::Allowed);
        client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await?;

        if !trailing_bytes.is_empty() {
            upstream.write_all(&trailing_bytes).await?;
        }

        // 5. Tunnel bidirectionnel aveugle (chiffrement TLS bout-en-bout intact)
        let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
    } else {
        // Traitement HTTP proxy standard (ex: GET http://host:port/path HTTP/1.1)
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

        // 1. Validation port + anti-SSRF sur IP littérale
        let literal_addr = match check_port_and_literal_ip(host, port, &config) {
            Ok(opt) => opt,
            Err((decision, err)) => {
                return Err(handle_upstream_rejection(&mut client, &telemetry, method, parsed_url.as_str(), decision, err).await);
            }
        };

        // 2. Filtrage Orakle Shield (URL complète + type ressource + exceptions @@) AVANT DNS
        let decision =
            shield.check_network_request(parsed_url.as_str(), &source_url, req_type, method)?;
        if decision.blocked {
            tracing::info!("[PROXY SHIELD BLOQUÉ HTTP] {}", parsed_url);
            record_event(
                &telemetry,
                method,
                parsed_url.as_str(),
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

        // 3. Résolution DNS + anti-DNS-rebinding SSRF
        let upstream_addr = match literal_addr {
            Some(addr) => addr,
            None => match resolve_and_validate_upstream(host, port, &config).await {
                Ok(addr) => addr,
                Err((decision, err)) => {
                    return Err(handle_upstream_rejection(&mut client, &telemetry, method, parsed_url.as_str(), decision, err).await);
                }
            },
        };

        // 4. Connexion amont et réécriture de l'URI absolue en URI relative standard
        let mut upstream = match TcpStream::connect(upstream_addr).await {
            Ok(s) => s,
            Err(e) => {
                let err_msg = format!("Echec connexion upstream {} : {}", upstream_addr, e);
                record_event(
                    &telemetry,
                    method,
                    parsed_url.as_str(),
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

        record_event(&telemetry, method, parsed_url.as_str(), ProxyDecision::Allowed);
        upstream.write_all(rewritten_head.as_bytes()).await?;
        if !trailing_bytes.is_empty() {
            upstream.write_all(&trailing_bytes).await?;
        }

        let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
    }

    Ok(())
}

fn parse_host_port(target: &str) -> Result<(String, u16)> {
    // Support IPv6 [::1]:443 et host:port
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
    fn test_ssrf_ip_validation_exhaustive() {
        // Privées RFC1918 & Link-local / Metadata & Loopback
        assert!(is_forbidden_private_ip("127.0.0.1".parse().unwrap(), false));
        assert!(!is_forbidden_private_ip("127.0.0.1".parse().unwrap(), true));
        assert!(is_forbidden_private_ip("10.0.0.5".parse().unwrap(), false));
        assert!(is_forbidden_private_ip("172.16.0.1".parse().unwrap(), false));
        assert!(is_forbidden_private_ip("192.168.1.1".parse().unwrap(), false));
        assert!(is_forbidden_private_ip("169.254.169.254".parse().unwrap(), false));
        assert!(is_forbidden_private_ip("100.64.0.1".parse().unwrap(), false));
        assert!(is_forbidden_private_ip("::1".parse().unwrap(), false));
        assert!(is_forbidden_private_ip("::ffff:127.0.0.1".parse().unwrap(), false));
        assert!(is_forbidden_private_ip("::ffff:169.254.169.254".parse().unwrap(), false));
        assert!(is_forbidden_private_ip("fc00::1".parse().unwrap(), false));
        // IP publique valide
        assert!(!is_forbidden_private_ip("1.1.1.1".parse().unwrap(), false));
        assert!(!is_forbidden_private_ip("93.184.216.34".parse().unwrap(), false));
    }

    #[tokio::test]
    async fn test_secure_loopback_proxy_blocking() {
        let shield = Arc::new(ShieldEngine::new_with_rules(TEST_RULES));
        let config = ProxyConfig::default();
        let (proxy, shutdown_rx) = SecureLoopbackProxy::new(config, shield);

        let proxy_addr = proxy.start(shutdown_rx).await.expect("start proxy");
        assert!(proxy_addr.ip().is_loopback());

        // 1. Test connexion CONNECT vers un domaine bloqué (DoubleClick)
        let mut client_blocked = TcpStream::connect(proxy_addr).await.expect("connect proxy");
        client_blocked
            .write_all(b"CONNECT googleads.g.doubleclick.net:443 HTTP/1.1\r\nHost: googleads.g.doubleclick.net:443\r\n\r\n")
            .await
            .unwrap();

        let mut resp_buf = [0u8; 512];
        let n = client_blocked.read(&mut resp_buf).await.unwrap();
        let resp_str = String::from_utf8_lossy(&resp_buf[..n]);
        assert!(
            resp_str.contains("403 Forbidden"),
            "DoubleClick doit être bloqué avec 403 par le proxy"
        );

        // 2. Test SSRF vers IP privée et loopback
        for target in ["192.168.1.1:443", "127.0.0.1:443", "169.254.169.254:80"] {
            let mut client_ssrf = TcpStream::connect(proxy_addr).await.expect("connect proxy");
            let req = format!("CONNECT {} HTTP/1.1\r\nHost: {}\r\n\r\n", target, target);
            client_ssrf.write_all(req.as_bytes()).await.unwrap();

            let mut ssrf_buf = [0u8; 512];
            let n_ssrf = client_ssrf.read(&mut ssrf_buf).await.unwrap();
            let ssrf_str = String::from_utf8_lossy(&ssrf_buf[..n_ssrf]);
            assert!(
                ssrf_str.contains("403 Forbidden"),
                "SSRF vers {} doit être bloqué en 403",
                target
            );
        }

        let stats = proxy.telemetry_snapshot();
        assert_eq!(stats.blocked_by_shield, 1);
        assert_eq!(stats.blocked_ssrf, 3);

        proxy.shutdown();
    }
}
