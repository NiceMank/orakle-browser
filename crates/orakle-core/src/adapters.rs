use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::error::Result;
use crate::shield::ShieldEngine;

#[derive(Debug, Clone, Serialize)]
pub struct AdapterCapabilities {
    pub platform: String,
    pub mechanism: String,
    pub status: String,
    pub supports_domain_block: bool,
    pub supports_https_subresource_url: bool,
    pub supports_resource_type: bool,
    pub supports_third_party: bool,
    pub supports_exceptions: bool,
    pub supports_cosmetic_css: bool,
    pub supports_scriptlets: bool,
    pub limitations: Vec<String>,
}

/// Matrice honnête des capacités réelles par système et couche de filtrage
pub fn platform_capability_matrix() -> Vec<AdapterCapabilities> {
    vec![
        AdapterCapabilities {
            platform: "Tous OS (Niveau 1)".into(),
            mechanism: "Proxy Loopback Rust (HTTP/CONNECT/SOCKS5)".into(),
            status: "Implémenté et testé".into(),
            supports_domain_block: true,
            supports_https_subresource_url: false,
            supports_resource_type: true, // HTTP clair uniquement
            supports_third_party: true,   // HTTP clair uniquement via Referer/Origin
            supports_exceptions: true,
            supports_cosmetic_css: false,
            supports_scriptlets: false,
            limitations: vec![
                "Sous HTTPS (CONNECT/SOCKS5), seul le domaine/port de destination est visible (Zero-MITM).".into(),
                "Les chemins URL HTTPS internes nécessitent un adaptateur OS Niveau 2.".into(),
            ],
        },
        AdapterCapabilities {
            platform: "Linux (WebKitGTK 4.1 — Niveau 2)".into(),
            mechanism: "WebKitWebExtension (liborakle_webkit_ext.so, signal send-request)".into(),
            status: "Implémenté et testé sous Linux".into(),
            supports_domain_block: true,
            supports_https_subresource_url: true,
            supports_resource_type: true,
            supports_third_party: true,
            supports_exceptions: true,
            supports_cosmetic_css: true,
            supports_scriptlets: false,
            limitations: vec![
                "Requiert le chargement de la bibliothèque partagée dans WebKitWebProcess.".into(),
                "Modification des en-têtes/corps de réponse ($csp, $removeparam) limitée dans send-request.".into(),
            ],
        },
        AdapterCapabilities {
            platform: "Windows (WebView2 — Niveau 2)".into(),
            mechanism: "CoreWebView2.WebResourceRequested -> ShieldEngine".into(),
            status: "Spécifié en Rust / non compilé ni testé sous Windows ici".into(),
            supports_domain_block: true,
            supports_https_subresource_url: true,
            supports_resource_type: true,
            supports_third_party: true,
            supports_exceptions: true,
            supports_cosmetic_css: true,
            supports_scriptlets: false,
            limitations: vec![
                "Nécessite un hôte Windows réel + WebView2 Runtime pour validation E2E.".into(),
            ],
        },
        AdapterCapabilities {
            platform: "macOS (WKWebView — Niveau 2)".into(),
            mechanism: "WKContentRuleListStore (JSON WebKit compilé)".into(),
            status: "Compilateur JSON unitaire implémenté / non testé sous macOS".into(),
            supports_domain_block: true,
            supports_https_subresource_url: true,
            supports_resource_type: true,
            supports_third_party: true,
            supports_exceptions: true,
            supports_cosmetic_css: true,
            supports_scriptlets: false,
            limitations: vec![
                "Ne prend pas en charge les règles $removeparam, $csp avancé ni les scriptlets ##+js().".into(),
                "Nécessite Xcode / macOS pour exécution WKContentRuleListStore.".into(),
            ],
        },
    ]
}

/// Écrit le fichier de règles partagé utilisé par l'extension Linux `WebKitWebExtension`
pub fn sync_rules_file_for_webkit_extension(
    shield: &ShieldEngine,
    target_path: &Path,
) -> Result<()> {
    let mut content = String::new();
    if !shield.is_enabled() {
        content.push_str("! DISABLED\n");
    } else {
        for r in shield.base_rules_lines() {
            content.push_str(&r);
            content.push('\n');
        }
        for exc in shield.exception_domains() {
            content.push_str(&format!("@@||{}^$document\n@@||{}^\n", exc, exc));
        }
    }
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = target_path.with_extension("tmp");
    fs::write(&tmp, content.as_bytes())?;
    fs::rename(&tmp, target_path)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct ContentRuleCompilationResult {
    pub json: String,
    pub compiled_count: usize,
    pub unsupported_rules: Vec<String>,
}

/// Compilateur de règles vers le format JSON Apple `WKContentRuleListStore` (sous-ensemble honnête)
pub fn compile_to_wk_content_rule_list(rules: &[String]) -> ContentRuleCompilationResult {
    let mut entries: Vec<serde_json::Value> = Vec::new();
    let mut unsupported: Vec<String> = Vec::new();

    for raw in rules {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('!') {
            continue;
        }
        if line.contains("##+js")
            || line.contains("$removeparam")
            || line.contains("$redirect")
            || line.contains("$csp")
        {
            unsupported.push(line.to_string());
            continue;
        }
        if let Some(exc) = line.strip_prefix("@@") {
            let pattern = exc
                .trim_start_matches("||")
                .trim_end_matches('^')
                .trim_end_matches("$document");
            entries.push(serde_json::json!({
                "trigger": { "url-filter": format!("^https?://([^/]+\\.)?{}", escape_regex(pattern)) },
                "action": { "type": "ignore-previous-rules" }
            }));
        } else if let Some(dom) = line.strip_prefix("||") {
            let clean = dom.split('$').next().unwrap_or(dom).trim_end_matches('^');
            let mut trigger = serde_json::json!({
                "url-filter": format!("^https?://([^/]+\\.)?{}", escape_regex(clean))
            });
            if line.contains("$third-party") {
                trigger["load-type"] = serde_json::json!(["third-party"]);
            }
            entries.push(serde_json::json!({
                "trigger": trigger,
                "action": { "type": "block" }
            }));
        } else if line.starts_with('/') && !line.contains("##") {
            entries.push(serde_json::json!({
                "trigger": { "url-filter": format!(".*{}", escape_regex(line)) },
                "action": { "type": "block" }
            }));
        } else {
            unsupported.push(line.to_string());
        }
    }

    let compiled_count = entries.len();
    let json = serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".into());
    ContentRuleCompilationResult {
        json,
        compiled_count,
        unsupported_rules: unsupported,
    }
}

fn escape_regex(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    for ch in input.chars() {
        match ch {
            '.' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            '*' => out.push_str(".*"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wk_content_rule_compiler_honest_subset() {
        let rules = vec![
            "||doubleclick.net^$third-party".to_string(),
            "/blocked-tracker.js".to_string(),
            "@@||doc.rust-lang.org^$document".to_string(),
            "example.com##+js(abort-on-property-read, ads)".to_string(),
            "||tracker.test^$removeparam=fbclid".to_string(),
        ];

        let res = compile_to_wk_content_rule_list(&rules);
        assert_eq!(res.compiled_count, 3);
        assert_eq!(res.unsupported_rules.len(), 2);
        assert!(res.json.contains("ignore-previous-rules"));
        assert!(res.json.contains("third-party"));
    }

    #[test]
    fn test_platform_matrix_honesty() {
        let matrix = platform_capability_matrix();
        assert_eq!(matrix.len(), 4);
        assert!(!matrix[0].supports_https_subresource_url);
        assert!(matrix[1].supports_https_subresource_url);
    }
}
