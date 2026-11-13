use crate::error::{OrakleError, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;
use url::Url;

#[derive(Debug, Clone, PartialEq)]
pub struct PulseResult {
    pub url: String,
    pub ip: String,
    pub dns_ms: f64,
    pub tcp_connect_ms: f64,
    pub tls_handshake_ms: f64,
    pub ttfb_ms: f64,
    pub total_ms: f64,
    pub status_code: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhasePercentiles {
    pub min_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PulseBenchmarkStats {
    pub url: String,
    pub samples_requested: usize,
    pub samples_succeeded: usize,
    pub dns: PhasePercentiles,
    pub tcp: PhasePercentiles,
    pub tls: PhasePercentiles,
    pub ttfb: PhasePercentiles,
    pub total: PhasePercentiles,
}

pub struct NetworkPulse {
    tls_config: Arc<ClientConfig>,
    timeout: Duration,
}

pub fn build_tls_client_config() -> Result<Arc<ClientConfig>> {
    let root_store = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };

    let provider = tokio_rustls::rustls::crypto::ring::default_provider();
    let config = ClientConfig::builder_with_provider(Arc::new(provider))
        .with_safe_default_protocol_versions()
        .map_err(|e| OrakleError::Tls(format!("Erreur versions protocole TLS : {:?}", e)))?
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(Arc::new(config))
}

pub async fn tls_handshake_client(
    tcp_stream: TcpStream,
    host: &str,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>> {
    let tls_config = build_tls_client_config()?;
    let connector = TlsConnector::from(tls_config);
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| OrakleError::Tls(format!("Nom de serveur TLS invalide : {}", e)))?;
    connector
        .connect(server_name, tcp_stream)
        .await
        .map_err(|e| OrakleError::Tls(format!("Échec handshake TLS : {}", e)))
}

impl NetworkPulse {
    pub fn new() -> Result<Self> {
        Self::with_timeout(Duration::from_secs(5))
    }

    pub fn with_timeout(timeout: Duration) -> Result<Self> {
        Ok(Self {
            tls_config: build_tls_client_config()?,
            timeout,
        })
    }

    /// Sonde complète d'une URL HTTPS avec mesures de chronométrie réelles à chaque couche et timeout strict
    pub async fn probe(&self, target_url: &str) -> Result<PulseResult> {
        let parsed = Url::parse(target_url)?;
        let host = parsed
            .host_str()
            .ok_or_else(|| OrakleError::Network("Hôte manquant".into()))?;
        let port = parsed.port_or_known_default().unwrap_or(443);
        let path = if parsed.path().is_empty() {
            "/"
        } else {
            parsed.path()
        };

        let t_total_start = Instant::now();

        // 1. Phase DNS Réelle
        let t_dns_start = Instant::now();
        let addr_str = format!("{}:{}", host, port);
        let mut addrs = tokio::time::timeout(self.timeout, tokio::net::lookup_host(&addr_str))
            .await
            .map_err(|_| {
                OrakleError::Network(format!("Timeout DNS ({:?}) pour {}", self.timeout, host))
            })?
            .map_err(|e| OrakleError::Network(format!("Échec résolution DNS : {}", e)))?;
        let resolved_addr = addrs
            .next()
            .ok_or_else(|| OrakleError::Network("Aucune adresse IP résolue".into()))?;
        let dns_ms = t_dns_start.elapsed().as_secs_f64() * 1000.0;

        // 2. Phase Connexion TCP Réelle
        let t_tcp_start = Instant::now();
        let tcp_stream = tokio::time::timeout(self.timeout, TcpStream::connect(resolved_addr))
            .await
            .map_err(|_| {
                OrakleError::Network(format!(
                    "Timeout TCP ({:?}) vers {}",
                    self.timeout, resolved_addr
                ))
            })?
            .map_err(|e| OrakleError::Network(format!("Échec connexion socket TCP : {}", e)))?;
        let tcp_connect_ms = t_tcp_start.elapsed().as_secs_f64() * 1000.0;

        // 3. Phase Négociation TLS Réelle (Handshake cryptographique)
        let t_tls_start = Instant::now();
        let server_name = ServerName::try_from(host.to_string())
            .map_err(|_| OrakleError::Network("Nom de serveur TLS invalide".into()))?;
        let connector = TlsConnector::from(self.tls_config.clone());
        let mut tls_stream =
            tokio::time::timeout(self.timeout, connector.connect(server_name, tcp_stream))
                .await
                .map_err(|_| {
                    OrakleError::Tls(format!(
                        "Timeout handshake TLS ({:?}) vers {}",
                        self.timeout, host
                    ))
                })?
                .map_err(|e| OrakleError::Tls(format!("Échec handshake TLS 1.3 : {}", e)))?;
        let tls_handshake_ms = t_tls_start.elapsed().as_secs_f64() * 1000.0;

        // 4. Phase Émission HTTP & Premier Octet (TTFB)
        let t_http_start = Instant::now();
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: OrakleBrowser/2.0\r\nConnection: close\r\nAccept: */*\r\n\r\n",
            path, host
        );
        tokio::time::timeout(self.timeout, tls_stream.write_all(request.as_bytes()))
            .await
            .map_err(|_| OrakleError::Network("Timeout écriture requête HTTP".into()))?
            .map_err(|e| OrakleError::Network(format!("Échec émission requête HTTP : {}", e)))?;

        let mut buffer = [0u8; 1024];
        let n = tokio::time::timeout(self.timeout, tls_stream.read(&mut buffer))
            .await
            .map_err(|_| OrakleError::Network("Timeout attente TTFB HTTP".into()))?
            .map_err(|e| OrakleError::Network(format!("Échec lecture premier octet : {}", e)))?;
        let ttfb_ms = t_http_start.elapsed().as_secs_f64() * 1000.0;
        let total_ms = t_total_start.elapsed().as_secs_f64() * 1000.0;

        let status_code = if n > 12 {
            let resp_str = String::from_utf8_lossy(&buffer[..n]);
            let parts: Vec<&str> = resp_str.split_whitespace().collect();
            if parts.len() > 1 {
                parts[1].parse::<u16>().unwrap_or(200)
            } else {
                200
            }
        } else {
            200
        };

        Ok(PulseResult {
            url: target_url.to_string(),
            ip: resolved_addr.to_string(),
            dns_ms,
            tcp_connect_ms,
            tls_handshake_ms,
            ttfb_ms,
            total_ms,
            status_code,
        })
    }

    /// Exécute N sondes séquentielles et calcule min, p50 (médiane), p95 et max sur chaque couche
    pub async fn probe_stats(
        &self,
        target_url: &str,
        samples: usize,
    ) -> Result<PulseBenchmarkStats> {
        let mut results = Vec::with_capacity(samples);
        let mut last_err = None;

        for _ in 0..samples {
            match self.probe(target_url).await {
                Ok(res) => results.push(res),
                Err(e) => last_err = Some(e),
            }
        }

        if results.is_empty() {
            return Err(
                last_err.unwrap_or_else(|| OrakleError::Network("Aucun échantillon réussi".into()))
            );
        }

        let dns_vals: Vec<f64> = results.iter().map(|r| r.dns_ms).collect();
        let tcp_vals: Vec<f64> = results.iter().map(|r| r.tcp_connect_ms).collect();
        let tls_vals: Vec<f64> = results.iter().map(|r| r.tls_handshake_ms).collect();
        let ttfb_vals: Vec<f64> = results.iter().map(|r| r.ttfb_ms).collect();
        let total_vals: Vec<f64> = results.iter().map(|r| r.total_ms).collect();

        Ok(PulseBenchmarkStats {
            url: target_url.to_string(),
            samples_requested: samples,
            samples_succeeded: results.len(),
            dns: compute_percentiles(dns_vals),
            tcp: compute_percentiles(tcp_vals),
            tls: compute_percentiles(tls_vals),
            ttfb: compute_percentiles(ttfb_vals),
            total: compute_percentiles(total_vals),
        })
    }
}

fn compute_percentiles(mut values: Vec<f64>) -> PhasePercentiles {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let len = values.len();
    let min_ms = values[0];
    let max_ms = values[len - 1];
    let p50_ms = values[len / 2];
    let p95_idx = ((len as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(len - 1);
    let p95_ms = values[p95_idx];

    PhasePercentiles {
        min_ms,
        p50_ms,
        p95_ms,
        max_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_real_pulse_probe_and_stats() {
        let pulse =
            NetworkPulse::with_timeout(Duration::from_secs(4)).expect("initialisation pulse");
        let stats_res = pulse.probe_stats("https://doc.rust-lang.org/", 5).await;

        match stats_res {
            Ok(s) => {
                println!(
                    "  [TEST PULSE STATISTIQUE] {} ({}/{} réussis)",
                    s.url, s.samples_succeeded, s.samples_requested
                );
                println!(
                    "       • DNS  : p50={:.2} ms | p95={:.2} ms | min={:.2} ms | max={:.2} ms",
                    s.dns.p50_ms, s.dns.p95_ms, s.dns.min_ms, s.dns.max_ms
                );
                println!(
                    "       • TCP  : p50={:.2} ms | p95={:.2} ms | min={:.2} ms | max={:.2} ms",
                    s.tcp.p50_ms, s.tcp.p95_ms, s.tcp.min_ms, s.tcp.max_ms
                );
                println!(
                    "       • TLS  : p50={:.2} ms | p95={:.2} ms | min={:.2} ms | max={:.2} ms",
                    s.tls.p50_ms, s.tls.p95_ms, s.tls.min_ms, s.tls.max_ms
                );
                println!(
                    "       • TTFB : p50={:.2} ms | p95={:.2} ms | min={:.2} ms | max={:.2} ms",
                    s.ttfb.p50_ms, s.ttfb.p95_ms, s.ttfb.min_ms, s.ttfb.max_ms
                );
                println!(
                    "       • TOTAL: p50={:.2} ms | p95={:.2} ms",
                    s.total.p50_ms, s.total.p95_ms
                );

                assert!(s.samples_succeeded >= 1);
                assert!(s.tcp.p50_ms > 0.0);
                assert!(s.tls.p50_ms > 0.0);
                assert!(s.ttfb.p50_ms > 0.0);
            }
            Err(e) => {
                println!(
                    "  [INFO] Test réseau ignoré (réseau indisponible ou bloqué) : {:?}",
                    e
                );
            }
        }
    }
}
