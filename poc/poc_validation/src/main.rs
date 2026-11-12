use std::time::Instant;
use rusqlite::{params, Connection};
use adblock::engine::Engine;
use adblock::request::Request;

fn main() {
    println!("============================================================");
    println!("   ORAKLE BROWSER V2 — SUITE TECHNIQUE DE VALIDATION RUST   ");
    println!("============================================================\n");

    // 1. TEST ADBLOCK-RUST & MESURE DU TEMPS DE FILTRAGE
    test_adblock_engine();

    // 2. TEST PERSISTANCE TRANSACTIONNELLE SQLITE (ACID & PERFORMANCES)
    test_sqlite_persistence();

    // 3. MESURES DE LATENCE RÉSEAU RÉELLE (ORAKLE PULSE EN RUST PUR)
    test_real_network_pulse();

    println!("\n============================================================");
    println!("   TOUS LES TESTS DE VALIDATION SONT TERMINÉS AVEC SUCCÈS   ");
    println!("============================================================");
}

fn test_adblock_engine() {
    println!("--- [TEST 1] Moteur de Filtrage Adblock-Rust (v0.13) ---");

    // Échantillon de règles réelles issues de EasyList / uBlock Origin
    let sample_rules = vec![
        "||googleads.g.doubleclick.net^$third-party",
        "||pagead2.googlesyndication.com^",
        "||google-analytics.com/analytics.js",
        "||googletagmanager.com/gtm.js",
        "||connect.facebook.net/*/sdk.js",
        "||pixel.facebook.com^",
        "||analytics.tiktok.com^",
        "||ads-twitter.com^",
        "||criteo.com^",
        "||taboola.com^",
        "||outbrain.com^",
        "||hotjar.com^$script",
        "||clarity.ms^",
        "||adnxs.com^",
        "@@||doc.rust-lang.org^$document", // Règle d'exception explicite
    ];

    let t_parse_start = Instant::now();
    let rules_text = sample_rules.join("\n");
    let engine = Engine::new_with_list_text(rules_text);
    let parse_duration = t_parse_start.elapsed();

    println!("  [OK] Initialisation du moteur avec {} règles en {:?}", sample_rules.len(), parse_duration);

    // Cas de tests réalistes
    let test_cases = vec![
        ("https://googleads.g.doubleclick.net/pagead/ads?client=ca-pub-123", "script", "https://lemonde.fr", true, "Publicité DoubleClick"),
        ("https://www.google-analytics.com/analytics.js", "script", "https://example.com", true, "Traqueur Google Analytics"),
        ("https://connect.facebook.net/en_US/sdk.js", "script", "https://shopping.com", true, "Pixel de pistage Meta"),
        ("https://criteo.com/delivery/ajs.php", "script", "https://ecommerce.fr", true, "Régie publicitaire Criteo"),
        ("https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html", "main_frame", "https://doc.rust-lang.org", false, "Page bénigne documentation Rust"),
        ("https://doc.rust-lang.org/css/normalize.css", "stylesheet", "https://doc.rust-lang.org", false, "Feuille de style légitime"),
        ("https://crates.io/api/v1/crates/rusqlite", "xmlhttprequest", "https://crates.io", false, "Requête API légitime"),
    ];

    println!("\n  Vérification unitaire des correspondances :");
    for (url, req_type, source, expected_blocked, description) in &test_cases {
        let request = Request::new(url, source, req_type, "GET").expect("Invalid request");
        let blocker_result = engine.check_network_request(&request);
        let is_blocked = blocker_result.should_block();

        assert_eq!(
            is_blocked, *expected_blocked,
            "Échec de filtrage pour : {} (attendu: {}, obtenu: {})",
            url, expected_blocked, is_blocked
        );

        let status = if is_blocked { "BLOQUÉ [✓]" } else { "AUTORISÉ [✓]" };
        println!("    {} {} — {}", status, description, url);
    }

    // Benchmark de vitesse sur 10 000 vérifications
    println!("\n  Benchmark de performance sur 10 000 itérations :");
    let benchmark_url = "https://googleads.g.doubleclick.net/pagead/ads?client=ca-pub-123456";
    let benchmark_req = Request::new(benchmark_url, "https://lemonde.fr", "script", "GET").unwrap();

    let iterations = 10_000;
    let t_bench_start = Instant::now();
    for _ in 0..iterations {
        let _ = engine.check_network_request(&benchmark_req);
    }
    let total_elapsed = t_bench_start.elapsed();
    let avg_micros = total_elapsed.as_nanos() as f64 / (iterations as f64 * 1000.0);

    println!("    • Total : {:?} pour {} vérifications", total_elapsed, iterations);
    println!("    • Vitesse moyenne mesurée : {:.3} µs par requête", avg_micros);
    assert!(avg_micros < 50.0, "Le temps de filtrage doit être < 50 µs");
    println!("    • Objectif < 50 µs : VÉRIFIÉ ET ATTEINT ({:.3} µs)", avg_micros);
}

fn test_sqlite_persistence() {
    println!("\n--- [TEST 2] Persistance Transactionnelle SQLite (ACID) ---");

    // Connexion à une base en mémoire pour le test unitaire
    let mut conn = Connection::open_in_memory().expect("Échec d'ouverture de SQLite en mémoire");

    // 1. Schéma structuré avec intégrité référentielle
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;

         CREATE TABLE schema_migrations (
             version INTEGER PRIMARY KEY,
             applied_at INTEGER NOT NULL
         );

         CREATE TABLE spaces (
             id TEXT PRIMARY KEY,
             name TEXT NOT NULL,
             color TEXT NOT NULL,
             created_at INTEGER NOT NULL
         );

         CREATE TABLE tabs (
             id TEXT PRIMARY KEY,
             space_id TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
             url TEXT NOT NULL,
             title TEXT NOT NULL,
             is_pinned INTEGER NOT NULL DEFAULT 0,
             is_suspended INTEGER NOT NULL DEFAULT 0,
             position INTEGER NOT NULL,
             last_active_at INTEGER NOT NULL
         );

         CREATE TABLE history (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             url TEXT NOT NULL,
             title TEXT NOT NULL,
             visited_at INTEGER NOT NULL
         );
         CREATE INDEX idx_history_visited_at ON history(visited_at DESC);
         CREATE INDEX idx_history_url ON history(url);

         CREATE TABLE bookmarks (
             id TEXT PRIMARY KEY,
             title TEXT NOT NULL,
             url TEXT NOT NULL,
             category TEXT NOT NULL,
             created_at INTEGER NOT NULL
         );",
    ).expect("Échec de création du schéma SQLite");

    println!("  [OK] Schéma structuré V2 créé avec intégrité référentielle et index.");

    // 2. Test des transactions ACID : insertion atomique
    {
        let tx = conn.transaction().expect("Échec d'ouverture de transaction");
        tx.execute(
            "INSERT INTO spaces (id, name, color, created_at) VALUES (?1, ?2, ?3, ?4)",
            params!["space-work", "Travail & Dév", "#2f81f7", 1731300000],
        ).unwrap();

        tx.execute(
            "INSERT INTO tabs (id, space_id, url, title, is_pinned, is_suspended, position, last_active_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params!["tab-1", "space-work", "https://doc.rust-lang.org", "The Rust Book", 1, 0, 0, 1731300100],
        ).unwrap();

        tx.commit().expect("Échec du commit de transaction");
    }

    // 3. Test de rollback en cas d'erreur (atomicité garantie)
    {
        let tx = conn.transaction().expect("Échec de transaction");
        let res = tx.execute(
            "INSERT INTO tabs (id, space_id, url, title, is_pinned, is_suspended, position, last_active_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params!["tab-2", "space-inconnu", "https://bad.com", "Bad", 0, 0, 1, 1731300200], // Clé étrangère invalide !
        );
        assert!(res.is_err(), "L'intégrité de clé étrangère aurait dû rejeter l'insertion");
        let _ = tx.rollback();
        println!("  [OK] Rollback ACID vérifié : rejet des contraintes orphelines sans corruption.");
    }

    // 4. Benchmark d'insertion en masse (1 000 visites d'historique)
    let t_start = Instant::now();
    {
        let tx = conn.transaction().expect("Échec transaction batch");
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO history (url, title, visited_at) VALUES (?1, ?2, ?3)"
            ).unwrap();

            for i in 0..1000 {
                stmt.execute(params![
                    format!("https://example.com/page-{}", i),
                    format!("Example Page {}", i),
                    1731300000 + i
                ]).unwrap();
            }
        }
        tx.commit().expect("Commit batch réussi");
    }
    let duration = t_start.elapsed();
    println!("  [OK] Benchmark : 1 000 entrées d'historique insérées en {:?}", duration);
    println!("       Soit {:.2} µs par entrée d'historique dans SQLite ACID", duration.as_micros() as f64 / 1000.0);

    let count: i64 = conn.query_row("SELECT count(*) FROM history", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 1000);
}

#[tokio::main]
async fn test_real_network_pulse() {
    println!("\n--- [TEST 3] Sonde Réseau Réelle Orakle Pulse (Sans pourcentages simulés) ---");

    let targets = vec![
        ("1.1.1.1:443", "Cloudflare DNS"),
        ("8.8.8.8:53", "Google Public DNS"),
    ];

    for (addr, name) in targets {
        let t_tcp_start = Instant::now();
        match tokio::net::TcpStream::connect(addr).await {
            Ok(_stream) => {
                let tcp_duration = t_tcp_start.elapsed();
                println!("  [OK] {} ({}) : Connexion socket TCP réelle en {:.2} ms", name, addr, tcp_duration.as_secs_f64() * 1000.0);
            }
            Err(e) => {
                println!("  [WARN] {} ({}) : Échec connexion ({})", name, addr, e);
            }
        }
    }

    // Mesure réelle de résolution DNS
    let host = "doc.rust-lang.org";
    let t_dns_start = Instant::now();
    match tokio::net::lookup_host(format!("{}:443", host)).await {
        Ok(mut addrs) => {
            let dns_duration = t_dns_start.elapsed();
            if let Some(first_ip) = addrs.next() {
                println!("  [OK] Résolution DNS réelle de {} -> {} en {:.2} ms", host, first_ip, dns_duration.as_secs_f64() * 1000.0);
            }
        }
        Err(e) => {
            println!("  [WARN] Échec DNS pour {} : {}", host, e);
        }
    }
}
