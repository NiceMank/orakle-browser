use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Notify;
use url::Url;

use crate::error::{OrakleError, Result};
use crate::proxy::is_forbidden_private_ip;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "level", rename_all = "snake_case")]
pub enum DownloadSafetyRisk {
    Safe,
    Archive,
    ExecutableWarning { extension: String },
}

/// Indique si une extension correspond à un exécutable ou script potentiellement risqué.
/// Avertissement honnête par extension : Orakle n'intègre pas de moteur antivirus.
pub fn classify_filename_risk(filename: &str) -> DownloadSafetyRisk {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "exe" | "msi" | "bat" | "cmd" | "ps1" | "vbs" | "js" | "jse" | "sh" | "bash"
        | "appimage" | "deb" | "rpm" | "dmg" | "pkg" | "apk" | "jar" | "bin" | "run" | "scr" => {
            DownloadSafetyRisk::ExecutableWarning { extension: ext }
        }
        "zip" | "tar" | "gz" | "xz" | "7z" | "rar" | "iso" => DownloadSafetyRisk::Archive,
        _ => DownloadSafetyRisk::Safe,
    }
}

/// Nettoie strictement un nom de fichier utilisateur/HTTP pour bloquer path traversal,
/// caractères de contrôle, octets nuls et noms réservés.
pub fn sanitize_filename(raw: &str) -> String {
    let stripped = raw.replace('\0', "").replace('\\', "/");
    let leaf = stripped
        .rsplit('/')
        .find(|seg| !seg.is_empty() && *seg != "." && *seg != "..")
        .unwrap_or("download.bin");

    let mut clean: String = leaf
        .chars()
        .filter(|c| !c.is_control() && *c != '/' && *c != '\\' && *c != ':')
        .collect();

    clean = clean.trim().trim_matches('.').to_string();
    if clean.is_empty() || clean == "." || clean == ".." {
        clean = "download.bin".to_string();
    }

    let upper_stem = Path::new(&clean)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_uppercase();
    if matches!(
        upper_stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        clean = format!("_{}", clean);
    }

    if clean.len() > 180 {
        if let Some(ext) = Path::new(&clean).extension().and_then(|e| e.to_str()) {
            let max_stem = 180usize.saturating_sub(ext.len() + 1);
            let stem: String = clean.chars().take(max_stem).collect();
            clean = format!("{}.{}", stem, ext);
        } else {
            clean = clean.chars().take(180).collect();
        }
    }

    clean
}

pub fn partial_path_for(final_path: &Path) -> PathBuf {
    let mut os = final_path.as_os_str().to_os_string();
    os.push(".oraklepart");
    PathBuf::from(os)
}

/// Résout un chemin de destination garanti à l'intérieur de `download_dir`, sans écrasement involontaire.
pub fn resolve_safe_destination(download_dir: &Path, raw_filename: &str) -> Result<PathBuf> {
    fs::create_dir_all(download_dir)?;
    let canon_dir = fs::canonicalize(download_dir)?;
    let safe_name = sanitize_filename(raw_filename);

    // Vérifie qu'aucun composant parent n'est présent
    let rel = Path::new(&safe_name);
    for comp in rel.components() {
        if !matches!(comp, Component::Normal(_)) {
            return Err(OrakleError::SecurityViolation(
                "Nom de fichier invalide (traversée bloquée)".into(),
            ));
        }
    }

    let base_stem = Path::new(&safe_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("download")
        .to_string();
    let ext = Path::new(&safe_name)
        .extension()
        .and_then(|s| s.to_str())
        .map(ToString::to_string);

    for idx in 0..10_000 {
        let candidate_name = if idx == 0 {
            safe_name.clone()
        } else if let Some(e) = &ext {
            format!("{} ({}).{}", base_stem, idx, e)
        } else {
            format!("{} ({})", base_stem, idx)
        };
        let candidate = canon_dir.join(&candidate_name);
        if !candidate.starts_with(&canon_dir) {
            return Err(OrakleError::SecurityViolation(
                "Chemin de téléchargement hors dossier autorisé".into(),
            ));
        }
        let part = partial_path_for(&candidate);
        if !candidate.exists() && !part.exists() {
            return Ok(candidate);
        }
    }

    Err(OrakleError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "Trop de collisions de noms dans le dossier de téléchargement",
    )))
}

pub fn finalize_partial_download(part_path: &Path, final_path: &Path) -> Result<()> {
    if !part_path.exists() {
        return Err(OrakleError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Fichier partiel introuvable lors de la finalisation",
        )));
    }
    if final_path.exists() {
        return Err(OrakleError::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Destination déjà existante : écrasement refusé",
        )));
    }
    fs::rename(part_path, final_path)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct DownloadStatusSnapshot {
    pub bytes_received: u64,
    pub total_bytes: u64,
    pub speed_bps: u64,
    pub state: u8, // 0=active, 1=paused, 2=completed, 3=cancelled, 4=error
}

pub const DL_STATE_ACTIVE: u8 = 0;
pub const DL_STATE_PAUSED: u8 = 1;
pub const DL_STATE_COMPLETED: u8 = 2;
pub const DL_STATE_CANCELLED: u8 = 3;
pub const DL_STATE_ERROR: u8 = 4;

#[derive(Clone)]
pub struct DownloadControl {
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
    state: Arc<AtomicU8>,
    bytes_received: Arc<AtomicU64>,
    total_bytes: Arc<AtomicU64>,
    speed_bps: Arc<AtomicU64>,
    notify: Arc<Notify>,
}

impl Default for DownloadControl {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadControl {
    pub fn new() -> Self {
        Self {
            paused: Arc::new(AtomicBool::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
            state: Arc::new(AtomicU8::new(DL_STATE_ACTIVE)),
            bytes_received: Arc::new(AtomicU64::new(0)),
            total_bytes: Arc::new(AtomicU64::new(0)),
            speed_bps: Arc::new(AtomicU64::new(0)),
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
        self.speed_bps.store(0, Ordering::Relaxed);
        self.state.store(DL_STATE_PAUSED, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
        self.state.store(DL_STATE_ACTIVE, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.state.store(DL_STATE_CANCELLED, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn set_snapshot_values(&self, received: u64, total: u64, speed: u64, st: u8) {
        self.bytes_received.store(received, Ordering::Relaxed);
        self.total_bytes.store(total, Ordering::Relaxed);
        self.speed_bps.store(speed, Ordering::Relaxed);
        self.state.store(st, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> DownloadStatusSnapshot {
        DownloadStatusSnapshot {
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
            speed_bps: self.speed_bps.load(Ordering::Relaxed),
            state: self.state.load(Ordering::Relaxed),
        }
    }
}

/// Effectue un téléchargement HTTP/HTTPS en streaming via le proxy loopback Orakle (ou direct si `proxy_port == None`),
/// avec prise en charge de Pause / Reprise HTTP `Range`, fichier `.oraklepart` et vérification SSRF.
pub async fn download_resumable_to_path<F>(
    raw_url: &str,
    final_path: &Path,
    proxy_port: Option<u16>,
    allow_loopback: bool,
    control: DownloadControl,
    mut on_progress: F,
) -> Result<()>
where
    F: FnMut(DownloadStatusSnapshot) + Send,
{
    let parsed = Url::parse(raw_url).map_err(|e| {
        OrakleError::SecurityViolation(format!("URL téléchargement invalide : {}", e))
    })?;
    let scheme = parsed.scheme().to_string();
    if scheme != "http" && scheme != "https" {
        return Err(OrakleError::SecurityViolation(format!(
            "Protocole non autorisé pour téléchargement : {}",
            scheme
        )));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| OrakleError::SecurityViolation("Hôte manquant".into()))?
        .to_string();
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<std::net::IpAddr>() {
        if is_forbidden_private_ip(ip, allow_loopback) {
            return Err(OrakleError::SecurityViolation(format!(
                "Téléchargement bloqué vers IP privée : {}",
                ip
            )));
        }
    }

    let part_path = partial_path_for(final_path);
    let mut offset: u64 = if part_path.exists() {
        fs::metadata(&part_path)?.len()
    } else {
        0
    };

    loop {
        if control.cancelled.load(Ordering::SeqCst) {
            let _ = fs::remove_file(&part_path);
            control.state.store(DL_STATE_CANCELLED, Ordering::SeqCst);
            on_progress(control.snapshot());
            return Err(OrakleError::Proxy("Téléchargement annulé".into()));
        }

        while control.paused.load(Ordering::SeqCst) {
            control.speed_bps.store(0, Ordering::Relaxed);
            control.state.store(DL_STATE_PAUSED, Ordering::SeqCst);
            on_progress(control.snapshot());
            control.notify.notified().await;
            if control.cancelled.load(Ordering::SeqCst) {
                let _ = fs::remove_file(&part_path);
                control.state.store(DL_STATE_CANCELLED, Ordering::SeqCst);
                on_progress(control.snapshot());
                return Err(OrakleError::Proxy("Téléchargement annulé".into()));
            }
        }

        let port = parsed.port_or_known_default().unwrap_or(80);
        let path_q = if let Some(q) = parsed.query() {
            format!("{}?{}", parsed.path(), q)
        } else {
            parsed.path().to_string()
        };

        // Connexion via SecureLoopbackProxy HTTP/CONNECT si configuré, sinon HTTP clair direct (tests)
        let mut stream = if let Some(pport) = proxy_port {
            TcpStream::connect(("127.0.0.1", pport)).await?
        } else {
            TcpStream::connect((host.as_str(), port)).await?
        };

        let host_header = if parsed.port().is_some() {
            format!("{}:{}", host, port)
        } else {
            host.clone()
        };

        let req_line = if proxy_port.is_some() && scheme == "http" {
            format!("GET {} HTTP/1.1\r\n", raw_url)
        } else {
            format!("GET {} HTTP/1.1\r\n", path_q)
        };

        if scheme == "https" && proxy_port.is_some() {
            let connect_req = format!(
                "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n\r\n",
                host, port, host, port
            );
            stream.write_all(connect_req.as_bytes()).await?;
            let mut resp_buf = [0u8; 1024];
            let n = stream.read(&mut resp_buf).await?;
            let resp_head = String::from_utf8_lossy(&resp_buf[..n]);
            if !resp_head.starts_with("HTTP/1.1 200") {
                control.state.store(DL_STATE_ERROR, Ordering::SeqCst);
                return Err(OrakleError::Proxy(format!(
                    "Tunnel proxy refusé : {}",
                    resp_head
                )));
            }
            let tls_stream = crate::pulse::tls_handshake_client(stream, &host).await?;
            let mut req_str = format!("{}Host: {}\r\nConnection: close\r\n", req_line, host_header);
            if offset > 0 {
                req_str.push_str(&format!("Range: bytes={}-\r\n", offset));
            }
            req_str.push_str("\r\n");
            let mut tls_w = tls_stream;
            tls_w.write_all(req_str.as_bytes()).await?;
            let res = stream_http_response_to_part(
                &mut tls_w,
                &part_path,
                &mut offset,
                &control,
                &mut on_progress,
            )
            .await;
            match res {
                Ok(true) => break,
                Ok(false) => continue,
                Err(e) => return Err(e),
            }
        } else {
            let mut req_str = format!("{}Host: {}\r\nConnection: close\r\n", req_line, host_header);
            if offset > 0 {
                req_str.push_str(&format!("Range: bytes={}-\r\n", offset));
            }
            req_str.push_str("\r\n");
            stream.write_all(req_str.as_bytes()).await?;
            let res = stream_http_response_to_part(
                &mut stream,
                &part_path,
                &mut offset,
                &control,
                &mut on_progress,
            )
            .await;
            match res {
                Ok(true) => break,
                Ok(false) => continue,
                Err(e) => return Err(e),
            }
        }
    }

    finalize_partial_download(&part_path, final_path)?;
    control.speed_bps.store(0, Ordering::Relaxed);
    control.state.store(DL_STATE_COMPLETED, Ordering::SeqCst);
    on_progress(control.snapshot());
    Ok(())
}

async fn stream_http_response_to_part<S, F>(
    io: &mut S,
    part_path: &Path,
    offset: &mut u64,
    control: &DownloadControl,
    on_progress: &mut F,
) -> Result<bool>
where
    S: tokio::io::AsyncRead + Unpin,
    F: FnMut(DownloadStatusSnapshot),
{
    let mut head_buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 1024];
    let header_end;
    loop {
        let n = io.read(&mut tmp).await?;
        if n == 0 {
            return Err(OrakleError::Proxy("Réponse HTTP vide".into()));
        }
        head_buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = head_buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
        if head_buf.len() > 32 * 1024 {
            return Err(OrakleError::Proxy("En-têtes HTTP trop volumineux".into()));
        }
    }

    let header_text = String::from_utf8_lossy(&head_buf[..header_end]);
    let mut lines = header_text.lines();
    let status_line = lines.next().unwrap_or("");
    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if status_code != 200 && status_code != 206 {
        control.state.store(DL_STATE_ERROR, Ordering::SeqCst);
        return Err(OrakleError::Proxy(format!(
            "Erreur serveur HTTP {}",
            status_code
        )));
    }

    let mut content_len: u64 = 0;
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                content_len = v.trim().parse().unwrap_or(0);
            }
        }
    }

    let mut file: File = if status_code == 206 && *offset > 0 {
        control
            .total_bytes
            .store(*offset + content_len, Ordering::Relaxed);
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(part_path)?
    } else {
        *offset = 0;
        control.total_bytes.store(content_len, Ordering::Relaxed);
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(part_path)?
    };

    let initial_body = &head_buf[header_end..];
    if !initial_body.is_empty() {
        file.write_all(initial_body)?;
        *offset += initial_body.len() as u64;
        control.bytes_received.store(*offset, Ordering::Relaxed);
    }

    let mut last_tick = Instant::now();
    let mut bytes_in_window: u64 = initial_body.len() as u64;
    let mut buf = vec![0u8; 32 * 1024];

    loop {
        if control.cancelled.load(Ordering::SeqCst) {
            let _ = file.flush();
            return Ok(false);
        }
        if control.paused.load(Ordering::SeqCst) {
            let _ = file.flush();
            return Ok(false);
        }

        let read_fut = tokio::time::timeout(Duration::from_millis(250), io.read(&mut buf)).await;
        match read_fut {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => {
                file.write_all(&buf[..n])?;
                *offset += n as u64;
                bytes_in_window += n as u64;
                control.bytes_received.store(*offset, Ordering::Relaxed);
                let elapsed = last_tick.elapsed().as_secs_f64();
                if elapsed >= 0.2 {
                    let bps = ((bytes_in_window as f64) / elapsed).round() as u64;
                    control.speed_bps.store(bps, Ordering::Relaxed);
                    bytes_in_window = 0;
                    last_tick = Instant::now();
                    on_progress(control.snapshot());
                }
            }
            Ok(Err(e)) => {
                control.state.store(DL_STATE_ERROR, Ordering::SeqCst);
                return Err(OrakleError::Io(e));
            }
            Err(_) => {
                // Timeout court pour vérifier pause/annulation sans blocage
                on_progress(control.snapshot());
            }
        }
    }

    file.flush()?;
    let expected = control.total_bytes.load(Ordering::Relaxed);
    if expected > 0 && *offset < expected {
        return Err(OrakleError::Proxy(
            "Téléchargement interrompu avant la fin du fichier".into(),
        ));
    }
    Ok(true)
}

pub fn read_file_head_sha256_ready(path: &Path) -> Result<u64> {
    let mut f = File::open(path)?;
    let mut buf = [0u8; 4096];
    let mut total = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total += n as u64;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_filename_sanitization_and_no_traversal_or_overwrite() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("..\\..\\Windows\\cmd.exe\0"), "cmd.exe");
        assert_eq!(sanitize_filename("CON.txt"), "_CON.txt");

        let dir = tempdir().unwrap();
        let d1 = resolve_safe_destination(dir.path(), "../rapport.pdf").unwrap();
        assert_eq!(d1.file_name().unwrap().to_str().unwrap(), "rapport.pdf");
        fs::write(&d1, b"existing").unwrap();

        let d2 = resolve_safe_destination(dir.path(), "rapport.pdf").unwrap();
        assert_eq!(d2.file_name().unwrap().to_str().unwrap(), "rapport (1).pdf");

        let part2 = partial_path_for(&d2);
        fs::write(&part2, b"partial").unwrap();
        let d3 = resolve_safe_destination(dir.path(), "rapport.pdf").unwrap();
        assert_eq!(d3.file_name().unwrap().to_str().unwrap(), "rapport (2).pdf");
    }

    #[test]
    fn test_executable_risk_classification() {
        assert_eq!(
            classify_filename_risk("setup.AppImage"),
            DownloadSafetyRisk::ExecutableWarning {
                extension: "appimage".into()
            }
        );
        assert_eq!(
            classify_filename_risk("notes.pdf"),
            DownloadSafetyRisk::Safe
        );
        assert_eq!(
            classify_filename_risk("archive.tar.gz"),
            DownloadSafetyRisk::Archive
        );
    }

    #[tokio::test]
    async fn test_resumable_http_download_with_pause_and_range() {
        let payload: Vec<u8> = (0..64 * 1024).map(|i| (i % 251) as u8).collect();
        let payload_srv = payload.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            while let Ok((mut s, _)) = listener.accept().await {
                let data = payload_srv.clone();
                tokio::spawn(async move {
                    let mut req = [0u8; 1024];
                    let n = s.read(&mut req).await.unwrap_or(0);
                    let txt = String::from_utf8_lossy(&req[..n]);
                    let mut start = 0usize;
                    let mut is_range = false;
                    for line in txt.lines() {
                        if let Some(v) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                            if let Some(num) = v.split('-').next() {
                                start = num.trim().parse().unwrap_or(0);
                                is_range = true;
                            }
                        }
                    }
                    if is_range && start < data.len() {
                        let slice = &data[start..];
                        let head = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",
                            slice.len(),
                            start,
                            data.len() - 1,
                            data.len()
                        );
                        let _ = s.write_all(head.as_bytes()).await;
                        let _ = s.write_all(slice).await;
                    } else {
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            data.len()
                        );
                        let _ = s.write_all(head.as_bytes()).await;
                        // Envoyer en deux morceaux avec léger délai pour simuler flux
                        let mid = data.len() / 2;
                        let _ = s.write_all(&data[..mid]).await;
                        tokio::time::sleep(Duration::from_millis(60)).await;
                        let _ = s.write_all(&data[mid..]).await;
                    }
                });
            }
        });

        let dir = tempdir().unwrap();
        let final_dest = resolve_safe_destination(dir.path(), "big.bin").unwrap();
        let ctrl = DownloadControl::new();
        let ctrl_trigger = ctrl.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            ctrl_trigger.pause();
            tokio::time::sleep(Duration::from_millis(50)).await;
            ctrl_trigger.resume();
        });

        let url = format!("http://{}/big.bin", addr);
        download_resumable_to_path(&url, &final_dest, None, true, ctrl, |_| {})
            .await
            .unwrap();

        let saved = fs::read(&final_dest).unwrap();
        assert_eq!(saved, payload);
        assert!(!partial_path_for(&final_dest).exists());
    }
}
