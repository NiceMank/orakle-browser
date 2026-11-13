use adblock::engine::Engine;
use adblock::lists::{FilterSet, ParseOptions};
use adblock::request::Request;
use crate::error::{OrakleError, Result};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub struct ShieldDecision {
    pub blocked: bool,
    pub rule: Option<String>,
    pub redirected_to: Option<String>,
    pub rewritten_url: Option<String>,
}

pub struct ShieldEngine {
    engine: Engine,
    rules_count: usize,
}

impl ShieldEngine {
    /// Crée une nouvelle instance de moteur avec un jeu de règles texte standard (EasyList / uBlock)
    pub fn new_with_rules(rules_text: &str) -> Self {
        let lines: Vec<&str> = rules_text.lines().collect();
        let count = lines.iter().filter(|l| !l.trim().is_empty() && !l.starts_with('!')).count();

        let mut filter_set = FilterSet::new(true);
        filter_set.add_filter_list(rules_text.to_string(), ParseOptions::default());
        let engine = Engine::new_with_filter_set(filter_set);

        Self {
            engine,
            rules_count: count,
        }
    }

    /// Nombre de règles actives chargées
    pub fn rules_count(&self) -> usize {
        self.rules_count
    }

    /// Vérifie si une requête réseau vers `url` doit être bloquée
    pub fn check_network_request(
        &self,
        url: &str,
        source_url: &str,
        req_type: &str,
        method: &str,
    ) -> Result<ShieldDecision> {
        let request = Request::new(url, source_url, req_type, method)
            .map_err(|e| OrakleError::Shield(format!("Requête invalide : {:?}", e)))?;

        let result = self.engine.check_network_request(&request);
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

        // Bloqué
        let r1 = engine.check_network_request(
            "https://googleads.g.doubleclick.net/pagead/ads?client=ca-pub-123",
            "https://lemonde.fr",
            "script",
            "GET",
        ).unwrap();
        assert!(r1.blocked);

        // Exception
        let r2 = engine.check_network_request(
            "https://doc.rust-lang.org/book/index.html",
            "https://doc.rust-lang.org",
            "main_frame",
            "GET",
        ).unwrap();
        assert!(!r2.blocked);
    }

    #[test]
    fn test_shield_percentiles_benchmark() {
        let engine = ShieldEngine::new_with_rules(SAMPLE_RULES);
        let stats = engine.run_benchmark(5_000);

        println!("  [BENCHMARK ADBLOCK] 5 000 requêtes :");
        println!("       • Moyenne : {:.3} µs", stats.avg_micros);
        println!("       • Médiane p50 : {:.3} µs", stats.p50_micros);
        println!("       • Percentile p95 : {:.3} µs", stats.p95_micros);
        println!("       • Percentile p99 : {:.3} µs", stats.p99_micros);

        #[cfg(not(debug_assertions))]
        {
            assert!(stats.avg_micros < 50.0);
            assert!(stats.p95_micros < 100.0);
        }
        #[cfg(debug_assertions)]
        {
            assert!(stats.avg_micros < 500.0);
            assert!(stats.p95_micros < 1000.0);
        }
    }
}
