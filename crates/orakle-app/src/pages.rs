use orakle_core::{sanitize_url_for_log, BookmarkEntry, HistoryEntry};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorPageKind {
    ProxyUnavailable,
    TlsInvalid,
    DnsUnreachable,
    NetworkTimeout,
    SecurityBlocked,
    RestoreUnavailable,
    GenericLoadError,
}

#[derive(Debug, Clone, Serialize)]
pub struct OmniboxSuggestion {
    pub kind: String,
    pub badge: String,
    pub title: String,
    pub subtitle: String,
    pub cmd: String,
    pub value: String,
}

fn html_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 16);
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn classify_webkit_error(message: &str, proxy_running: bool) -> ErrorPageKind {
    if !proxy_running {
        return ErrorPageKind::ProxyUnavailable;
    }
    let lower = message.to_ascii_lowercase();
    if lower.contains("cannot connect to proxy")
        || (lower.contains("connection refused") && lower.contains("proxy"))
    {
        ErrorPageKind::ProxyUnavailable
    } else if lower.contains("tls") || lower.contains("certificate") || lower.contains("ssl") {
        ErrorPageKind::TlsInvalid
    } else if lower.contains("resolve")
        || lower.contains("name or service")
        || lower.contains("dns")
    {
        ErrorPageKind::DnsUnreachable
    } else if lower.contains("timed out") || lower.contains("timeout") || lower.contains("network")
    {
        ErrorPageKind::NetworkTimeout
    } else if lower.contains("policy")
        || lower.contains("403 forbidden")
        || lower.contains("blocked")
    {
        ErrorPageKind::SecurityBlocked
    } else {
        ErrorPageKind::GenericLoadError
    }
}

pub fn render_error_page_html(kind: ErrorPageKind, raw_url: &str) -> String {
    let clean_url = html_escape(&sanitize_url_for_log(raw_url));
    let (badge, title, detail, recovery) = match kind {
        ErrorPageKind::ProxyUnavailable => (
            "Protection fail-closed",
            "Proxy de sécurité Orakle arrêté",
            "Par mesure de sécurité, Orakle bloque toute navigation lorsque le proxy local de filtrage est indisponible afin d'éviter un contournement silencieux des protections.",
            "Utilisez le bouton « Redémarrer le proxy » dans la bannière d'alerte en haut de la fenêtre, puis actualisez la page.",
        ),
        ErrorPageKind::TlsInvalid => (
            "Certificat bloqué",
            "Connexion chiffrée non fiable",
            "Le certificat de sécurité présenté par ce site est invalide, expiré ou ne correspond pas au domaine demandé. La connexion a été interrompue pour protéger vos données.",
            "Vérifiez la date système ou l'adresse saisie. Orakle ne contourne pas automatiquement les erreurs TLS.",
        ),
        ErrorPageKind::DnsUnreachable => (
            "Adresse introuvable",
            "Impossible de trouver ce site",
            "Le nom de domaine demandé n'a pas pu être résolu ou l'appareil est hors ligne.",
            "Vérifiez l'orthographe de l'adresse ainsi que votre connexion réseau, puis réessayez.",
        ),
        ErrorPageKind::NetworkTimeout => (
            "Délai dépassé",
            "Le site met trop de temps à répondre",
            "Le serveur distant n'a pas répondu dans le délai imparti ou la connexion a été interrompue en cours de chargement.",
            "Cliquez sur « Actualiser » dans la barre supérieure pour relancer le chargement.",
        ),
        ErrorPageKind::SecurityBlocked => (
            "Politique de sécurité",
            "Accès bloqué par Orakle",
            "Cette requête a été bloquée par Orakle Shield ou par la protection contre les accès locaux non sollicités (SSRF / schéma non autorisé).",
            "Si vous souhaitez accéder à un serveur de développement local, saisissez explicitement son adresse dans la barre d'adresse.",
        ),
        ErrorPageKind::RestoreUnavailable => (
            "Restauration incomplète",
            "Impossible de restaurer cet onglet",
            "La page enregistrée n'a pas pu être rechargée depuis le réseau ou l'état distant a expiré.",
            "Vous pouvez réessayer l'actualisation ou ouvrir une nouvelle page depuis la barre d'adresse.",
        ),
        ErrorPageKind::GenericLoadError => (
            "Chargement interrompu",
            "Cette page n'a pas pu être affichée",
            "Le site est temporairement inaccessible ou a fermé la connexion de manière inattendue.",
            "Vérifiez votre connexion et utilisez « Actualiser » pour réessayer.",
        ),
    };

    format!(
        r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>{title} — Orakle</title>
<style>
  :root {{ --bg:#0d1117; --surface:#161b22; --border:rgba(240,246,252,.12); --text:#f0f6fc; --muted:#8b949e; --accent:#0969da; --warn:#d29922; }}
  @media (prefers-color-scheme: light) {{
    :root {{ --bg:#f3f5f8; --surface:#ffffff; --border:rgba(31,35,40,.12); --text:#1f2328; --muted:#59636e; --accent:#0969da; --warn:#9a6700; }}
  }}
  body {{ margin:0; min-height:100vh; display:flex; align-items:center; justify-content:center; padding:24px; background:var(--bg); color:var(--text); font-family:-apple-system,BlinkMacSystemFont,"Plus Jakarta Sans","Inter",sans-serif; }}
  .card {{ max-width:560px; width:100%; background:var(--surface); border:1px solid var(--border); border-radius:12px; padding:28px; }}
  .badge {{ display:inline-block; font-size:11.5px; font-weight:600; padding:4px 10px; border-radius:6px; background:rgba(210,153,34,.14); color:var(--warn); margin-bottom:14px; }}
  h1 {{ font-size:20px; font-weight:600; margin:0 0 10px; }}
  p {{ font-size:14px; line-height:1.58; color:var(--muted); margin:0 0 14px; }}
  .url {{ font-family:"JetBrains Mono",ui-monospace,monospace; font-size:12px; color:var(--text); background:var(--bg); border:1px solid var(--border); border-radius:8px; padding:9px 12px; word-break:break-all; margin-bottom:14px; }}
  .hint {{ font-size:13px; color:var(--text); border-left:3px solid var(--accent); padding-left:12px; margin-top:10px; }}
</style></head>
<body><main class="card">
  <span class="badge">{badge}</span>
  <h1>{title}</h1>
  <div class="url">{clean_url}</div>
  <p>{detail}</p>
  <div class="hint">{recovery}</div>
</main></body></html>"#
    )
}

pub fn search_engine_label(search_engine: &str) -> &'static str {
    match search_engine {
        "startpage" => "Startpage",
        "qwant" => "Qwant",
        "brave" => "Brave Search",
        "ecosia" => "Ecosia",
        _ => "DuckDuckGo",
    }
}

pub fn normalize_search_engine(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "startpage" => "startpage",
        "qwant" => "qwant",
        "brave" => "brave",
        "ecosia" => "ecosia",
        _ => "duckduckgo",
    }
}

pub fn render_start_page_html(
    profile_name: &str,
    blocked_count: u64,
    theme: &str,
    search_engine: &str,
    show_tips: bool,
) -> String {
    let is_light = theme == "light";
    let (bg, surface, border, text, muted, accent, kbd_bg) = if is_light {
        (
            "#f3f5f8",
            "#ffffff",
            "rgba(31,35,40,.12)",
            "#1f2328",
            "#59636e",
            "#0969da",
            "#eef2f6",
        )
    } else {
        (
            "#0d1117",
            "#161b22",
            "rgba(240,246,252,.10)",
            "#f0f6fc",
            "#8b949e",
            "#58a6ff",
            "#21262d",
        )
    };
    let pname = html_escape(profile_name);
    let engine_label = search_engine_label(search_engine);
    let tips_section = if show_tips {
        format!(
            r#"<section class="tips-box" aria-label="Astuces rapides Orakle">
    <div class="tips-head">
      <h2>Astuces &amp; Raccourcis</h2>
      <span class="pill-small">Cliquez sur <strong>? Astuces</strong> en haut pour tout afficher</span>
    </div>
    <div class="tips-grid">
      <div class="tip"><kbd>Ctrl+T</kbd><span>Nouvel onglet</span></div>
      <div class="tip"><kbd>Ctrl+W</kbd><span>Fermer l'onglet (ou clic molette)</span></div>
      <div class="tip"><kbd>Ctrl+Shift+T</kbd><span>Rouvrir le dernier onglet fermé</span></div>
      <div class="tip"><kbd>Ctrl+L</kbd><span>Saisir une URL ou recherche ({engine_label})</span></div>
      <div class="tip"><kbd>Ctrl+\</kbd><span>Basculer de volet en Split View</span></div>
      <div class="tip"><kbd>Alt+← / →</kbd><span>Précédent / Suivant</span></div>
      <div class="tip"><kbd>F1</kbd><span>Afficher / masquer toutes les astuces</span></div>
      <div class="tip"><kbd>Clic droit onglet</kbd><span>Épingler, dupliquer, suspendre, Split</span></div>
    </div>
  </section>"#
        )
    } else {
        String::new()
    };

    format!(
        r#"<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Nouvel onglet — Orakle</title>
<style>
  body {{ margin:0; min-height:100vh; background:{bg}; color:{text}; font-family:-apple-system,BlinkMacSystemFont,"Plus Jakarta Sans","Inter",sans-serif; padding:40px 28px; box-sizing:border-box; }}
  .wrap {{ max-width:840px; margin:0 auto; }}
  .head {{ display:flex; align-items:center; justify-content:space-between; flex-wrap:wrap; gap:12px; margin-bottom:22px; }}
  h1 {{ font-size:22px; font-weight:600; margin:0; letter-spacing:-0.01em; }}
  .pills {{ display:flex; gap:8px; flex-wrap:wrap; }}
  .pill {{ font-size:12px; padding:5px 11px; border-radius:999px; background:{surface}; border:1px solid {border}; color:{muted}; }}
  .pill strong {{ color:{text}; font-weight:600; }}
  .grid {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(230px,1fr)); gap:12px; margin-top:16px; }}
  .card {{ background:{surface}; border:1px solid {border}; border-radius:10px; padding:16px; }}
  .card h2 {{ font-size:14px; font-weight:600; margin:0 0 6px; }}
  .card p {{ font-size:12.5px; color:{muted}; margin:0 0 10px; line-height:1.5; }}
  a {{ color:{accent}; text-decoration:none; font-size:12.5px; font-weight:500; }}
  a:hover {{ text-decoration:underline; }}
  code {{ font-family:"JetBrains Mono",ui-monospace,monospace; font-size:11.5px; background:{kbd_bg}; padding:2px 6px; border-radius:4px; color:{text}; }}
  .tips-box {{ margin-top:18px; background:{surface}; border:1px solid {border}; border-radius:10px; padding:16px 18px; }}
  .tips-head {{ display:flex; align-items:center; justify-content:space-between; flex-wrap:wrap; gap:8px; margin-bottom:12px; }}
  .tips-head h2 {{ font-size:14px; font-weight:600; margin:0; }}
  .pill-small {{ font-size:11.5px; color:{muted}; }}
  .tips-grid {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(240px,1fr)); gap:8px 14px; }}
  .tip {{ display:flex; align-items:center; gap:9px; font-size:12.5px; color:{muted}; }}
  kbd {{ font-family:"JetBrains Mono",ui-monospace,monospace; font-size:11px; background:{kbd_bg}; border:1px solid {border}; border-radius:5px; padding:2px 7px; color:{text}; white-space:nowrap; }}
</style></head>
<body><main class="wrap">
  <div class="head">
    <div><h1>Orakle Browser V2</h1><p style="color:{muted};margin:6px 0 0;font-size:13px;">Profil actif : <strong>{pname}</strong></p></div>
    <div class="pills">
      <span class="pill">Moteur : <strong>{engine_label}</strong></span>
      <span class="pill">Shield actif · <strong>{blocked_count}</strong> blocage(s)</span>
    </div>
  </div>
  <div class="grid">
    <section class="card"><h2>Raccourcis Omnibox (Bangs)</h2><p>Saisissez <code>!gh</code>, <code>!w</code>, <code>!mdn</code>, <code>!yt</code> ou <code>!ddg</code> suivi de votre recherche.</p></section>
    <section class="card"><h2>Développement Local</h2><p>Tapez <code>localhost:3000</code> ou <code>127.0.0.1:8080</code> dans la barre d'adresse. Les pages externes restent bloquées (SSRF).</p></section>
    <section class="card"><h2>Accès rapide HTTPS</h2><p><a href="https://doc.rust-lang.org/std/">Documentation Rust</a> · <a href="https://developer.mozilla.org/fr/">MDN Web Docs</a></p></section>
  </div>
  {tips_section}
</main></body></html>"#
    )
}

fn encode_query(q: &str) -> String {
    url::form_urlencoded::byte_serialize(q.as_bytes()).collect()
}

pub fn resolve_omnibox_input(raw: &str, search_engine: &str) -> (String, Option<u16>) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return ("about:blank".to_string(), None);
    }

    // Bangs rapides documentés dans PRODUCT_REQUIREMENTS.md
    if let Some((bang, rest)) = trimmed.split_once(' ') {
        let q = encode_query(rest.trim());
        match bang.to_ascii_lowercase().as_str() {
            "!gh" => return (format!("https://github.com/search?q={}", q), None),
            "!w" => {
                return (
                    format!("https://fr.wikipedia.org/w/index.php?search={}", q),
                    None,
                )
            }
            "!mdn" => {
                return (
                    format!("https://developer.mozilla.org/fr/search?q={}", q),
                    None,
                )
            }
            "!yt" => {
                return (
                    format!("https://www.youtube.com/results?search_query={}", q),
                    None,
                )
            }
            "!ddg" => return (format!("https://duckduckgo.com/?q={}", q), None),
            _ => {}
        }
    }

    let normalized = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if !trimmed.contains(' ')
        && (trimmed == "localhost"
            || trimmed.starts_with("localhost:")
            || trimmed.starts_with("localhost/")
            || trimmed == "127.0.0.1"
            || trimmed.starts_with("127.0.0.1:")
            || trimmed.starts_with("127.0.0.1/")
            || trimmed.starts_with("[::1]"))
    {
        format!("http://{}", trimmed)
    } else if !trimmed.contains(' ') && (trimmed.contains('.') || trimmed.contains(':')) {
        format!("https://{}", trimmed)
    } else {
        let encoded = encode_query(trimmed);
        let url = match search_engine {
            "startpage" => format!("https://www.startpage.com/do/search?q={}", encoded),
            "qwant" => format!("https://www.qwant.com/?q={}", encoded),
            "brave" => format!("https://search.brave.com/search?q={}", encoded),
            "ecosia" => format!("https://www.ecosia.org/search?q={}", encoded),
            _ => format!("https://duckduckgo.com/?q={}", encoded),
        };
        return (url, None);
    };

    let dev_port = url::Url::parse(&normalized).ok().and_then(|u| {
        let host = u.host_str()?;
        if orakle_core::is_host_loopback(host) {
            u.port_or_known_default()
        } else {
            None
        }
    });

    (normalized, dev_port)
}

pub fn build_omnibox_suggestions(
    query: &str,
    search_engine: &str,
    open_tabs: &[(String, String, String, bool)],
    bookmarks: &[BookmarkEntry],
    history: &[HistoryEntry],
) -> Vec<OmniboxSuggestion> {
    let q = query.trim().to_ascii_lowercase();
    let mut out = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();

    if q.is_empty() {
        out.push(OmniboxSuggestion {
            kind: "shortcut".into(),
            badge: "Bang".into(),
            title: "!gh · !w · !mdn · !yt · !ddg".into(),
            subtitle: "Raccourcis de recherche directe dans l'Omnibox".into(),
            cmd: "hint".into(),
            value: "!mdn ".into(),
        });
    } else {
        let (target, _) = resolve_omnibox_input(query, search_engine);
        out.push(OmniboxSuggestion {
            kind: "action".into(),
            badge: if query.trim().starts_with('!') {
                "Bang".into()
            } else if target.starts_with("http://localhost")
                || target.starts_with("http://127.0.0.1")
            {
                "Local Dev".into()
            } else if query.contains('.') && !query.contains(' ') {
                "Aller".into()
            } else {
                "Rechercher".into()
            },
            title: query.trim().to_string(),
            subtitle: sanitize_url_for_log(&target),
            cmd: "navigate".into(),
            value: query.trim().to_string(),
        });
        seen_urls.insert(target);
    }

    for (tab_id, title, url, is_suspended) in open_tabs {
        if out.len() >= 7 {
            break;
        }
        if !q.is_empty()
            && !title.to_ascii_lowercase().contains(&q)
            && !url.to_ascii_lowercase().contains(&q)
        {
            continue;
        }
        if !url.is_empty() && url != "about:blank" {
            out.push(OmniboxSuggestion {
                kind: "tab".into(),
                badge: if *is_suspended {
                    "Onglet (veille)"
                } else {
                    "Onglet"
                }
                .into(),
                title: if title.is_empty() {
                    url.clone()
                } else {
                    title.clone()
                },
                subtitle: sanitize_url_for_log(url),
                cmd: "select_tab".into(),
                value: tab_id.clone(),
            });
            seen_urls.insert(url.clone());
        }
    }

    for b in bookmarks {
        if out.len() >= 8 {
            break;
        }
        if !q.is_empty()
            && !b.title.to_ascii_lowercase().contains(&q)
            && !b.url.to_ascii_lowercase().contains(&q)
        {
            continue;
        }
        if seen_urls.insert(b.url.clone()) {
            out.push(OmniboxSuggestion {
                kind: "bookmark".into(),
                badge: "Favori".into(),
                title: if b.title.is_empty() {
                    b.url.clone()
                } else {
                    b.title.clone()
                },
                subtitle: sanitize_url_for_log(&b.url),
                cmd: "navigate".into(),
                value: b.url.clone(),
            });
        }
    }

    for h in history {
        if out.len() >= 8 {
            break;
        }
        if !q.is_empty()
            && !h.title.to_ascii_lowercase().contains(&q)
            && !h.url.to_ascii_lowercase().contains(&q)
        {
            continue;
        }
        if seen_urls.insert(h.url.clone()) {
            out.push(OmniboxSuggestion {
                kind: "history".into(),
                badge: "Historique".into(),
                title: if h.title.is_empty() {
                    h.url.clone()
                } else {
                    h.title.clone()
                },
                subtitle: sanitize_url_for_log(&h.url),
                cmd: "navigate".into(),
                value: h.url.clone(),
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_search_engines_all_supported() {
        let (ddg, _) = resolve_omnibox_input("rust gtk3", "duckduckgo");
        assert_eq!(ddg, "https://duckduckgo.com/?q=rust+gtk3");
        let (qw, _) = resolve_omnibox_input("rust gtk3", "qwant");
        assert_eq!(qw, "https://www.qwant.com/?q=rust+gtk3");
        let (sp, _) = resolve_omnibox_input("rust gtk3", "startpage");
        assert_eq!(sp, "https://www.startpage.com/do/search?q=rust+gtk3");
        let (br, _) = resolve_omnibox_input("rust gtk3", "brave");
        assert_eq!(br, "https://search.brave.com/search?q=rust+gtk3");
        let (ec, _) = resolve_omnibox_input("rust gtk3", "ecosia");
        assert_eq!(ec, "https://www.ecosia.org/search?q=rust+gtk3");
    }

    #[test]
    fn normalize_and_render_start_page_with_tips() {
        assert_eq!(normalize_search_engine("Brave"), "brave");
        assert_eq!(normalize_search_engine("invalid"), "duckduckgo");
        let with_tips = render_start_page_html("Farel", 42, "dark", "qwant", true);
        assert!(with_tips.contains("Moteur : <strong>Qwant</strong>"));
        assert!(with_tips.contains("Astuces &amp; Raccourcis"));
        let without_tips = render_start_page_html("Farel", 0, "light", "ecosia", false);
        assert!(without_tips.contains("Ecosia"));
        assert!(!without_tips.contains("Astuces &amp; Raccourcis"));
    }
}
