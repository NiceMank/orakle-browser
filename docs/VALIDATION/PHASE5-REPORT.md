# Orakle Browser V2 — Rapport de Validation Phase 5 (Product Hardening, Sécurité, Performance & Packaging)

- **Date** : 2026-11-13
- **Branche** : `feat/v2-rust-architecture`
- **Tag de restauration Phase 4** : `v2.0-phase4-baseline` (`cff508b`)
- **Tag V1.1 conservé intact** : `v1.1-aurora-final` (`main` à `396e9dd`)
- **Environnement mesuré** : Linux `x86_64`, `tao 0.37.2`, `wry 0.57.2`, `WebKit2GTK 4.1`, `rusqlite 0.33`, `adblock 0.13.5` (`liborakle_webkit_ext.so` Niveau 2)

---

## 1. Synthèse des Livrables Phase 5

1. **Application utilisable de bout en bout (`orakle-app`)** :
   - **Omnibox professionnelle** (`crates/orakle-app/src/chrome.html` + `main.rs`) : résolution d'URL complètes, domaines sans protocole, recherche configurable, Bangs (`!gh`, `!w`, `!yt`, `!so`, `!mdn`), suggestions superposées multi-sources (Favoris SQLite, Historique SQLite, Onglets ouverts).
   - **Téléchargements résumables** (`crates/orakle-core/src/downloads.rs`) : support HTTP `Range` / `206`, pause/reprise/annulation, assainissement des noms de fichiers (`sanitize_filename`, anti-path-traversal), classification des extensions à risque (`DownloadSafetyRisk`) **avec mention honnête d'absence de moteur antivirus local**, persistance SQLite.
   - **Pages d'erreur WebView natives & politique Fail-Closed** (`crates/orakle-app/src/pages.rs` + `main.rs`) : gestion explicite des erreurs de chargement (`GenericLoadError`, `DnsError`, `TlsError`, `ProxyUnavailable`, `RestoreUnavailable`) via `load_html` différé (`idle_add_local_once`) synchronisé sur `proxy_running_flag` (`Arc<AtomicBool>`), plus bannière d'arrêt du proxy dans l'UI Chrome avec redémarrage sans fuite directe.
   - **Profils isolés & Mode Privé éphémère** (`crates/orakle-core/src/profile.rs`) : bases SQLite séparées par profil (`ProfileRegistry` / `ProfileContext`) et session privée `PrivateSession` à purge disque automatique à la fermeture.
   - **Correctif cycle de vie WebKit** (`crates/orakle-app/src/main.rs`, `bench.rs`) : appel explicite à `gwv.terminate_web_process()` et retrait `fixed.remove(&gwv)` lors de la fermeture/suspension et entre itérations de benchmark, éliminant l'accumulation de processus `WebKitWebProcess` observée initialement.

2. **Durcissement Sécurité (`phase5_security_and_profiles.rs`)** :
   - Validation anti-SSRF / anti-DNS-rebinding (`is_forbidden_private_ip`, CRLF/injection d'URL), blocage de redirections HTTP `3xx Location:` vers `127.0.0.1`/réseaux privés, blocage anti-CSRF `Origin`/`Referer` vers loopback, tunnel WebSocket via `CONNECT` + comportement fail-closed après arrêt du proxy.

3. **Packaging & Préparation Distribution (`packaging/`)** :
   - **Linux `.deb` compilé et vérifié localement** (`packaging/linux/build-deb.sh`) : produit `packaging/dist/orakle-browser_0.2.0_amd64.deb` (`3 865 592` octets, `Installed-Size: 16695` KiB, SHA-256 `c519c12c53b77574b6dba895945c8d81c3402ecd56b5c920a934889bbb7e9d53` vérifié via `dpkg-deb --info`, `dpkg-deb --contents` et `sha256sum -c`). Contient `/usr/bin/orakle-app`, `/usr/bin/orakle-browser` (wrapper `ORAKLE_WEBKIT_EXTENSION_DIR`), `/usr/lib/orakle-browser/extensions/liborakle_webkit_ext.so`, `.desktop` et icône SVG [`orakle-browser.svg`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/packaging/orakle-browser.svg).
   - **AppImage (`packaging/linux/build-appimage.sh`)** : `AppDir` assemblé avec succès dans `packaging/dist/Orakle-Browser.AppDir`, génération `.AppImage` finale **non exécutée** (`appimagetool` absent sur la machine hôte).
   - **Windows (`packaging/windows/orakle.wxs`) & macOS (`packaging/macos/package-macos.sh`)** : manifestes et scripts préparés comme **modèles non compilés ni testés** sur Windows/macOS dans cette session Linux.
   - **Signature & Mises à jour (`packaging/SIGNING-AND-UPDATES.md`, `packaging/update-manifest.example.json`)** : procédure et modèle documentés sans aucune clé ni signature simulée.

---

## 2. Vérifications Qualité, Tests & Non-Régression

| Vérification | Commande | Résultat observé |
|---|---|---|
| Formatage Rust | `cargo fmt --check` | **Succès** (`exit 0`, aucune divergence) |
| Analyse statique Rust | `cargo clippy --workspace --all-targets` | **Succès** (`exit 0`, 0 erreur ; avertissements résiduels non bloquants dans `orakle-app`, `webview_proxy_harness` et tests) |
| Suite de tests Rust | `cargo test --workspace` | **29 passed, 0 failed** (`17` unitaires `orakle_core` + `6` `phase4_security_and_lifecycle` + `5` `phase5_security_and_profiles` + `1` `proxy_server_e2e`) |
| Compilation Release | `cargo build --release --workspace` | **Succès** (`Finished release profile [optimized] in 3m 05s`) |
| Non-régression V1.1 (`v1.1-aurora-final`) | `npm run typecheck && npm run build` | **Succès** (`tsc --noEmit`, `tsc -p tsconfig.electron.json --noEmit`, Vite `dist/assets/index-DLSskh2O.js` en `35.37s`) ; lancement Xvfb `--no-sandbox --disable-gpu` arrêté au timeout (`exit=124`, `GPU process isn't usable` sous Xvfb sans GPU) |
| Scénario utilisateur E2E Phase 5 | `xvfb-run ... ./target/release/orakle-app --verify-phase5 --capture-dir docs/VALIDATION/screenshots_p5` | **Succès** : 7 étapes (`tabs_open=3`, `tabs_db=3`, `bookmarks=1`, `history=9`, `downloads=1`, `sessions=1`, `profiles=2`, page d'erreur native HTTP inaccessible) |
| Compatibilité Wayland native | `env -u DISPLAY WAYLAND_DISPLAY=wayland-0 GDK_BACKEND=wayland ./target/release/orakle-app --verify --no-sandbox` | **Succès** (`Extension active=true`, Split View `web_procs=4 → 3 → 4`, `natif=5`, `priv_leaked=false`) |
| HiDPI (`GDK_SCALE=2`) & `1920x1080` | `GDK_SCALE=2 xvfb-run ... 2880x1800x24` + `1920x1080x24` `--verify-phase5` | **Succès** : mise à l'échelle UI + Split View + panneau droit sans troncature ni chevauchement |

---

## 3. Captures d'Écran Vérifiées (`docs/VALIDATION/screenshots_p5/`)

Toutes les captures ont été produites en mode Release et inspectées visuellement :
- [`p5-01-dark-home.png`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/docs/VALIDATION/screenshots_p5/p5-01-dark-home.png) : écran d'accueil sombre, compteur `Shield 2`.
- [`p5-02-omnibox-dropdown.png`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/docs/VALIDATION/screenshots_p5/p5-02-omnibox-dropdown.png) : menu superposé d'autocomplétion Omnibox (`[Local Dev]` / `[Onglet]`).
- [`p5-03-split-shield.png`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/docs/VALIDATION/screenshots_p5/p5-03-split-shield.png) : Split View 70/30 avec deux pages indépendantes + panneau latéral Shield.
- [`p5-04-light-history.png`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/docs/VALIDATION/screenshots_p5/p5-04-light-history.png) : thème clair, densité compacte et panneau Historique réel.
- [`p5-05-downloads.png`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/docs/VALIDATION/screenshots_p5/p5-05-downloads.png) : gestionnaire de téléchargements (`download-test.bin` 256/256 KiB + avertissement absence d'antivirus local).
- [`p5-06-proxy-outage-profiles.png`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/docs/VALIDATION/screenshots_p5/p5-06-proxy-outage-profiles.png) : bannière rouge Fail-Closed lors de l'arrêt du proxy + gestionnaire de profils.
- [`p5-07-error-page.png`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/docs/VALIDATION/screenshots_p5/p5-07-error-page.png) : page d'erreur native *« Chargement interrompu — Cette page n'a pas pu être affichée »* (`https://127.0.0.1:39037/unreachable`) après redémarrage du proxy.

---

## 4. Benchmark Réel Mémoire, PSS, cgroup v2, CPU & Latences (30 passes mesurées + 1 warmup)

Exécuté dans un cgroup v2 isolé via `systemd-run --user --scope -p MemoryAccounting=yes` (source complète : [`phase5_bench_results.json`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/docs/VALIDATION/phase5_bench_results.json)) :

- **Démarrage à froid fenêtre Chrome** : `startup_window_ms = 271,48 ms`
- **Première WebView prête** : `cold_first_webview_ms = 603,80 ms`
- **Swap consommé (tous scénarios)** : `0,0 MiB` (p50 et max)
- **Zéro fuite `WebKitWebProcess`** : sur `1_webview_vide`, `3_webviews_vides`, `3_pages_simples`, `10_onglets_ouverts` et `apres_restauration_split`, le nombre de processus `WebKitWebProcess` reste strictement borné (`[1,1]`, `[3,3]`, `[10,10]`, `[2,2]`) sur les 30 passes.

| Scénario | RSS p50 (p90 / p95) MiB | PSS p50 (p90 / p95) MiB | cgroup p50 (p95 / max) MiB | Latence action p50 / p95 (ms) | CPU % p50 (p95) |
|---|---|---|---|---|---|
| `1_webview_vide` | 310,6 (331,0 / 337,0) | 156,6 (172,7 / 175,9) | 99,7 (117,5 / 118,4) | 14,4 / 21,0 | 0,0% (0,0%) |
| `3_webviews_vides` | 414,9 (425,9 / 428,0) | 169,5 (181,2 / 183,5) | 121,2 (138,7 / 138,8) | 22,7 / 29,9 | 19,7% (32,9%) |
| `3_pages_simples` | 273,5 (285,0 / 285,9) | 143,3 (154,6 / 157,3) | 90,8 (108,3 / 108,6) | 22,7 / 33,3 | 0,0% (0,0%) |
| `3_pages_complexes` | 413,8 (428,2 / 429,5) | 169,4 (183,0 / 184,4) | 118,9 (138,7 / 139,4) | 25,2 / 45,0 | 25,5% (544,3%) |
| `10_onglets_ouverts` | 404,1 (821,4 / 828,2) | 199,0 (241,8 / 252,1) | 123,5 (209,6 / 216,3) | 114,8 / 226,4 | 280,5% (383,9%) |
| `split_view_2_panneaux` | 339,7 (410,5 / 549,1) | 145,2 (167,8 / 217,2) | 166,8 (201,8 / 207,5) | 33,4 / 108,4 | 0,0% (0,0%) |
| `apres_suspension_split` | 283,8 (393,6 / 409,7) | 140,7 (180,5 / 196,7) | 126,3 (163,2 / 171,9) | 10,1 / 22,6 | 260,9% (316,3%) |
| `apres_restauration_split` | 570,1 (581,4 / 583,1) | 220,2 (231,6 / 233,1) | 134,6 (150,9 / 151,7) | 4,4 / 9,3 | 124,4% (146,6%) |

Ventilation cgroup observée sur le dernier échantillon de `split_view_2_panneaux` (`memory.stat`) : `anon = 177 604 KiB`, `file = 8 100 KiB`, `shmem = 8 044 KiB`, `kernel = 19 316 KiB` (`memory.current = 206 684 KiB`).

---

## 5. Limites Restantes Documentées Honnêtement

- **Coffres-forts OS (`libsecret` / `DPAPI` / `Keychain`)** : non intégrés pour les mots de passe à ce stade (aucun stockage de secret en clair dans SQLite).
- **Cibles Windows & macOS** : non compilées ni exécutées dans cet environnement Linux (scripts/WiX fournis comme bases de travail).
- **AppImage finale** : `AppDir` validé, création binaire `.AppImage` dépendante de l'installation d'`appimagetool`.
- **Signature de paquets & CVE CI** : aucune clé GPG/Authenticode générée localement ; audit `cargo-audit` / CI à brancher côté workflow de release.
