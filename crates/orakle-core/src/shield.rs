use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
use std::time::Instant;

use adblock::engine::Engine;
use adblock::lists::{FilterSet, ParseOptions};
use adblock::request::Request;

use crate::error::{OrakleError, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct ShieldDecision {
    pub blocked: bool,
    pub rule: Option<String>,
    pub redirected_to: Option<String>,
    pub rewritten_url: Option<String>,
}

struct ShieldInner {
    base_rules: String,
    exceptions: BTreeSet<String>,
    engine: Engine,
    rules_count: usize,
}

impl ShieldInner {
    fn compile(base_rules: &str, exceptions: &BTreeSet<String>) -> Self {
        let mut combined = String::with_capacity(base_rules.len() + exceptions.len() * 48);
        combined.push_str(base_rules);
        combined.push('\n');

        for domain in exceptions {
            let clean = domain.trim().trim_start_matches("www.");
            if !clean.is_empty() {
                combined.push_str(&format!("@@||{}^$document\n", clean));
                combined.push_str(&format!("@@||{}^\n", clean));
            }
        }

        let count = combined
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('!'))
            .count();

        let mut filter_set = FilterSet::new(true);
        filter_set.add_filter_list(combined, ParseOptions::default());
        let engine = Engine::new_with_filter_set(filter_set);

        Self {
            base_rules: base_rules.to_string(),
            exceptions: exceptions.clone(),
            engine,
            rules_count: count,
        }
    }
}

pub struct ShieldEngine {
    enabled: AtomicBool,
    inner: RwLock<ShieldInner>,
}

impl ShieldEngine {
    /// Crée une nouvelle instance de moteur avec un jeu de règles texte standard (EasyList / uBlock)
    pub fn new_with_rules(rules_text: &str) -> Self {
        let inner = ShieldInner::compile(rules_text, &BTreeSet::new());
        Self {
            enabled: AtomicBool::new(true),
            inner: RwLock::new(inner),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Ajoute une exception de domaine (allowlist dynamique) et recompile le moteur
    pub fn add_exception_domain(&self, domain: &str) {
        let clean = domain.trim().to_ascii_lowercase();
        if clean.is_empty() {
            return;
        }
        let mut guard = self.inner.write().expect("shield lock");
        if guard.exceptions.insert(clean) {
            let compiled = ShieldInner::compile(&guard.base_rules, &guard.exceptions);
            *guard = compiled;
        }
    }

    /// Supprime une exception de domaine
    pub fn remove_exception_domain(&self, domain: &str) {
        let clean = domain.trim().to_ascii_lowercase();
        let mut guard = self.inner.write().expect("shield lock");
        if guard.exceptions.remove(&clean) {
            let compiled = ShieldInner::compile(&guard.base_rules, &guard.exceptions);
            *guard = compiled;
        }
    }

    pub fn exception_domains(&self) -> Vec<String> {
        self.inner
            .read()
            .expect("shield lock")
            .exceptions
            .iter()
            .cloned()
            .collect()
    }

    pub fn base_rules_lines(&self) -> Vec<String> {
        self.inner
            .read()
            .expect("shield lock")
            .base_rules
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('!'))
            .map(ToString::to_string)
            .collect()
    }

    /// Nombre de règles actives chargées
    pub fn rules_count(&self) -> usize {
        self.inner.read().expect("shield lock").rules_count
    }

    /// Vérifie si une requête réseau vers `url` doit être bloquée
    pub fn check_network_request(
        &self,
        url: &str,
        source_url: &str,
        req_type: &str,
        method: &str,
    ) -> Result<ShieldDecision> {
        if !self.is_enabled() {
            return Ok(ShieldDecision {
                blocked: false,
                rule: None,
                redirected_to: None,
                rewritten_url: None,
            });
        }

        let guard = self.inner.read().expect("shield lock");

        // Vérifie les exceptions explicites au niveau domaine (cible ou page source / document)
        for candidate in [url, source_url] {
            if let Ok(parsed) = url::Url::parse(candidate) {
                if let Some(host) = parsed.host_str() {
                    let host_lower = host.to_ascii_lowercase();
                    if guard
                        .exceptions
                        .iter()
                        .any(|exc| host_lower == *exc || host_lower.ends_with(&format!(".{}", exc)))
                    {
                        return Ok(ShieldDecision {
                            blocked: false,
                            rule: Some(format!("@@||{}^", host_lower)),
                            redirected_to: None,
                            rewritten_url: None,
                        });
                    }
                }
            }
        }

        let request = Request::new(url, source_url, req_type, method)
            .map_err(|e| OrakleError::Shield(format!("Requête invalide : {:?}", e)))?;

        let result = guard.engine.check_network_request(&request);
        let blocked = result.should_block();

        Ok(ShieldDecision {
            blocked,
            rule: result.filter.and_then(|f| f.raw_line),
            redirected_to: result.redirect,
            rewritten_url: result.rewritten_url,
        })
    }

    /// Exécute un benchmark statistique détaillé mesurant les percentiles p50, p95 et p99
    pub fn run_benchmark(&self, iterations: usize) -> BenchmarkStats {
        let test_url = "https://googleads.g.doubleclick.net/pagead/ads?client=ca-pub-123456";
        let source_url = "https://lemonde.fr";
        let req_type = "script";

        let mut latencies_micros: Vec<f64> = Vec::with_capacity(iterations);

        for _ in 0..iterations {
            let t_start = Instant::now();
            let _ = self.check_network_request(test_url, source_url, req_type, "GET");
            let elapsed = t_start.elapsed().as_nanos() as f64 / 1000.0;
            latencies_micros.push(elapsed);
        }

        latencies_micros.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let p50 = latencies_micros[iterations / 2];
        let p95 = latencies_micros[(iterations as f64 * 0.95) as usize];
        let p99 = latencies_micros[(iterations as f64 * 0.99) as usize];
        let sum: f64 = latencies_micros.iter().sum();
        let avg = sum / (iterations as f64);

        BenchmarkStats {
            iterations,
            avg_micros: avg,
            p50_micros: p50,
            p95_micros: p95,
            p99_micros: p99,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BenchmarkStats {
    pub iterations: usize,
    pub avg_micros: f64,
    pub p50_micros: f64,
    pub p95_micros: f64,
    pub p99_micros: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RULES: &str = "
||googleads.g.doubleclick.net^$third-party
||pagead2.googlesyndication.com^
||google-analytics.com/analytics.js
||googletagmanager.com/gtm.js
||connect.facebook.net/*/sdk.js
||pixel.facebook.com^
||analytics.tiktok.com^
||criteo.com^
||taboola.com^
@@||doc.rust-lang.org^$document
";

    #[test]
    fn test_shield_blocking_and_exceptions() {
        let engine = ShieldEngine::new_with_rules(SAMPLE_RULES);

        let r1 = engine
            .check_network_request(
                "https://googleads.g.doubleclick.net/pagead/ads?client=ca-pub-123",
                "https://lemonde.fr",
                "script",
                "GET",
            )
            .unwrap();
        assert!(r1.blocked);

        let r2 = engine
            .check_network_request(
                "https://doc.rust-lang.org/book/index.html",
                "https://doc.rust-lang.org",
                "main_frame",
                "GET",
            )
            .unwrap();
        assert!(!r2.blocked);

        // Test exception dynamique + désactivation globale
        engine.add_exception_domain("criteo.com");
        let r3 = engine
            .check_network_request(
                "https://criteo.com/js",
                "https://example.com",
                "script",
                "GET",
            )
            .unwrap();
        assert!(!r3.blocked);

        engine.remove_exception_domain("criteo.com");
        let r4 = engine
            .check_network_request(
                "https://criteo.com/js",
                "https://example.com",
                "script",
                "GET",
            )
            .unwrap();
        assert!(r4.blocked);

        engine.set_enabled(false);
        let r5 = engine
            .check_network_request(
                "https://criteo.com/js",
                "https://example.com",
                "script",
                "GET",
            )
            .unwrap();
        assert!(!r5.blocked);
    }

    #[test]
    fn test_shield_percentiles_benchmark() {
        let engine = ShieldEngine::new_with_rules(SAMPLE_RULES);
        let stats = engine.run_benchmark(5_000);

        println!("  [BENCHMARK ADBLOCK] 5 000 requêtes :");
        println!(
            "    • Moyenne : {:.3} µs | p50 : {:.3} µs | p95 : {:.3} µs | p99 : {:.3} µs",
            stats.avg_micros, stats.p50_micros, stats.p95_micros, stats.p99_micros
        );

        #[cfg(debug_assertions)]
        let max_p95 = 2500.0;
        #[cfg(not(debug_assertions))]
        let max_p95 = 250.0;
        assert!(
            stats.p95_micros < max_p95,
            "La latence p95 doit rester basse"
        );
    }
}
