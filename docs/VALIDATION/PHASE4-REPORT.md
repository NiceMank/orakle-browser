# Orakle Browser V2 — Rapport de Validation Phase 4

- **Date** : 2026-11-13
- **Branche** : `feat/v2-rust-architecture` (branche locale uniquement ; le dépôt distant `origin` `git@github.com:NiceMank/orakle-browser.git` ne contient que `origin/main` et `origin/feat/aurora-ui-v1.1` ; le tag `v1.1-aurora-final` est intact)
- **Environnement de test** : Linux x86_64 (`WebKitGTK 4.1` / sysroot `2.52.6`), serveur d'affichage virtuel `xvfb-run` (`GDK_BACKEND=x11`, `1440x900x24`, `WEBKIT_DISABLE_COMPOSITING_MODE=1`, `WEBKIT_DISABLE_DMABUF_RENDERER=1`), comptabilité mémoire noyau `systemd-run --user --scope -p MemoryAccounting=yes` (cgroup v2 + `/proc/<pid>/smaps_rollup`).

---

## 1. Synthèse de Validation Multi-Composants

| Domaine | État exact | Preuve vérifiable |
|---|---|---|
| Interface native (`chrome.html` + `panel.html` + `orakle-app`) | **Implémenté et testé sous Linux** | [`crates/orakle-app/src/main.rs`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/crates/orakle-app/src/main.rs), captures PNG réelles inspectées (`docs/VALIDATION/screenshots/*.png`) |
| Split View (2 WebViews indépendantes, focus, 30/70, 50/50, 70/30, swap, fermeture, scroll sync) | **Implémenté et testé sous Linux** | Exécution `--verify` + captures `02-orakle-v2-split-shield-dark.png` & `03-orakle-v2-split-light.png` |
| Proxy Loopback Niveau 1 (`HTTP` + `CONNECT` + `SOCKS5` RFC 1928) | **Implémenté et testé** | [`crates/orakle-core/src/proxy.rs`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/crates/orakle-core/src/proxy.rs), `proxy_server_e2e.rs` (`ws://` SOCKS5, IPv6 `[::1]`, DNS-fail `502`, fail-closed post-shutdown) |
| Adaptateur natif Niveau 2 Linux (`WebKitWebExtension` `send-request`) | **Implémenté et testé sous Linux** | [`crates/orakle-webkit-ext/src/lib.rs`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/crates/orakle-webkit-ext/src/lib.rs), `--verify` : `Extension active=true`, `shield_blocked=5 (natif=5)` |
| Adaptateur natif Niveau 2 Windows (`WebView2 WebResourceRequested`) | **Spécifié / Prototype (non compilé ni testé sous Windows)** | [`crates/orakle-core/src/adapters.rs`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/crates/orakle-core/src/adapters.rs) |
| Adaptateur natif Niveau 2 macOS (`WKContentRuleListStore`) | **Compilateur JSON unitaire implémenté sans test macOS** | `compile_to_wk_content_rule_list` dans [`adapters.rs`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/crates/orakle-core/src/adapters.rs) |
| Suspension / Restauration d'onglets (SQLite WAL v2) | **Implémenté et testé sous Linux** | Destruction réelle WebView (`web_procs=4 -> 3`), restauration SQLite (`web_procs=3 -> 4`), `phase4_security_and_lifecycle.rs` |
| Profils séparés & Mode Privé (`PrivateSession`) | **Implémenté et testé** | `priv_leaked=false` dans `--verify`, `test_profile_purge_and_private_session_auto_cleanup` |
| Coffre-fort identifiants OS (`libsecret` / `DPAPI` / `Keychain`) | **Non réalisé** | Aucun gestionnaire de mots de passe intégré en Phase 4 |
| Préservation V1.1 Electron (`v1.1-aurora-final`) | **Compilé et lancé sans erreur** | `npm run typecheck && npm run build` (exit 0) + lancement Xvfb `electron . --no-sandbox` stable sur 8 s (`SIGTERM` propre, exit `124`) |

---

## 2. Captures Réelles d'Exécution (`--verify --capture-dir`)

1. **Accueil & Barre supérieure + Barre d'onglets (Thème Sombre)** : [`docs/VALIDATION/screenshots/01-orakle-v2-home-dark.png`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/docs/VALIDATION/screenshots/01-orakle-v2-home-dark.png)
   - Montre la barre supérieure (`Orakle`, précédent/suivant, rechargement, badge sécurité `Local`, omnibox, favori, compteur réel `Shield 2`, Split, Téléchargements, Panneau, Paramètres), l'espace `Travail`, l'onglet actif `Accueil Orakle V2` et la page HTML servie via proxy loopback + extension `liborakle_webkit_ext.so`.
2. **Split View 50/50 + Panneau Orakle Shield (Thème Sombre)** : [`docs/VALIDATION/screenshots/02-orakle-v2-split-shield-dark.png`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/docs/VALIDATION/screenshots/02-orakle-v2-split-shield-dark.png)
   - Montre les deux WebViews indépendantes (`Volet Gauche` et `Volet Droit`), la barre Split (`50% / 50%`), le panneau latéral avec télémétrie exacte (`127.0.0.1:<port>`, `4` blocages dont `4` via `WebKitWebExtension`, `0/0` SSRF/Ports, `2928 octets` relayés), l'allowlist et l'affichage transparent des limites par OS.
3. **Split View 70/30 inversé + Espaces & Sessions (Thème Clair)** : [`docs/VALIDATION/screenshots/03-orakle-v2-split-light.png`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/docs/VALIDATION/screenshots/03-orakle-v2-split-light.png)
   - Montre l'inversion Gauche/Droite, le ratio `70/30`, le focus basculé à droite (`Droite`), le thème clair persistant et le panneau `Espaces & Sessions`.

---

## 3. Mesures Réelles Mémoire (RSS / PSS / cgroup v2) et CPU

Commande reproductible :
```bash
systemd-run --user --scope -p MemoryAccounting=yes env GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 xvfb-run -a -s "-screen 0 1440x900x24" ./target/release/orakle-app --bench --runs 3
```
Données brutes complètes conservées dans [`docs/VALIDATION/phase4_bench_results.json`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/docs/VALIDATION/phase4_bench_results.json).

| Scénario (3 passes) | Total RSS médian (MiB) | Total PSS médian (MiB) | Main RSS (MiB) | WebKitWeb RSS (MiB) | WebKitNet RSS (MiB) | cgroup `memory.current` (MiB) | CPU médian (ms) |
|---|---|---|---|---|---|---|---|
| `1_webview_vide` | 394.8 (p95 394.9) | 178.2 (p95 178.2) | 164.8 | 160.6 | 69.4 | 142.8 | 2460 |
| `3_webviews_vides` | 565.4 (p95 576.9) | 214.7 (p95 220.7) | 158.3 | 340.9 | 69.4 | 185.2 | 2990 |
| `3_pages_simples` | 885.0 (p95 886.9) | 263.5 (p95 264.1) | 158.7 | 656.2 | 69.4 | 235.0 | 4130 |
| `3_pages_complexes` | 885.7 (p95 894.1) | 265.0 (p95 269.3) | 158.0 | 660.8 | 69.4 | 236.4 | 4400 |
| `10_onglets_ouverts` | 1025.8 (p95 1052.0) | 354.0 (p95 362.0) | 158.3 | 800.6 | 69.5 | 263.1 | 6000 |
| `split_view_2_panneaux` | 1001.7 (p95 1780.5) | 269.6 (p95 443.7) | 158.4 | 768.0 | 69.9 | 239.8 | 5740 |
| `apres_suspension_split` | 394.6 (p95 1522.3) | 179.1 (p95 319.9) | 158.4 | 163.3 | 69.4 | 140.6 | 4450 |
| `apres_restauration_split` | 569.1 (p95 571.3) | 215.8 (p95 215.9) | 161.2 | 336.8 | 69.5 | 173.8 | 3940 |

**Note d'interprétation méthodologique :**
1. La somme brute des `RSS` multi-processus (`1025.8 MiB` à 10 onglets) surcompte les bibliothèques partagées (`libwebkit2gtk-4.1.so`, GTK, ICU) mappées en lecture seule dans chaque `WebKitWebProcess`. Le **`PSS` (Proportional Set Size)** (`354.0 MiB`) et `memory.current` cgroup v2 (`263.1 MiB`) mesurent l'empreinte physique effective.
2. En passe 1 sur `split_view_2_panneaux` et `apres_suspension_split`, WebKitGTK détruit certains processus enfants de `10_onglets_ouverts` de façon asynchrone (d'où le pic p95 `1780.5` / `1522.3 MiB`), avant retour à `394.6 MiB` RSS / `179.1 MiB` PSS / `140.6 MiB` cgroup dès convergence GLib.
3. Sous `WEBKIT_DISABLE_COMPOSITING_MODE=1` (requis sous `Xvfb` sans DRI3), `WebKitGPUProcess` n'est pas lancé (`0.0 MiB`). Les valeurs GPU sur session bureau Wayland/Xorg accélérée matériellement peuvent différer.

---

## 4. Matrice Honnête des Capacités Réelles (Shield & Plateformes)

| Couche / Plateforme | Statut | Blocage Domaine | URL complètes sous HTTPS | Types & 3rd-party | Cosmétique CSS | Scriptlets `##+js()` & `$removeparam` |
|---|---|---|---|---|---|---|
| Tous OS — Proxy Loopback (`HTTP`/`CONNECT`/`SOCKS5`) | **Implémenté et testé** | Oui | HTTP clair uniquement (HTTPS = domaine/port Zero-MITM) | HTTP clair | Non | Non |
| Linux — `WebKitWebExtension` (`liborakle_webkit_ext.so`) | **Implémenté et testé sous Linux** | Oui | Oui (`send-request`) | Oui | Support partiel côté UI/extension | Non (`$csp` / `$removeparam` corps non réécrits) |
| Windows — `WebView2` (`WebResourceRequested`) | **Spécifié / Prototype (non compilé ni testé sous Windows)** | Prêt API | Prêt API | Prêt API | Injecteur JS requis | Non |
| macOS — `WKWebView` (`WKContentRuleListStore`) | **Compilateur JSON testé unitairement (non exécuté sur macOS)** | Oui (compilé) | Oui (règles compatibles) | `load-type` | Oui (`css-display-none`) | Non (signalés dans `unsupported_rules`) |

---

## 5. Garanties Réelles de Suspension & Restauration d'Onglets

- **Ce qui est conservé dans SQLite WAL (`orakle.sqlite`, schéma v2)** : `id`, `space_id`, `group_id`, `url`, `title`, `is_pinned`, `is_suspended`, `is_incognito` (exclu de la persistance), `position`, `scroll_y`, `last_active_at`.
- **Ce qui N'EST PAS conservé lors de la destruction de la WebView** : état volatile en mémoire JavaScript/DOM (formulaires non soumis, lecteurs vidéo/audio en cours, variables `window.*` ou état WebSocket). Lors de la restauration, Orakle recrée une WebView propre et recharge l'URL enregistrée (comme un navigateur multi-processus standard sans sérialisation de tas JS).

---

## 6. Sécurité & Suite Automatisée

- **Séparation UI / Web distante vérifiée** :
  - `chrome_wv` et `panel_wv` restreignent `with_navigation_handler(|u| u == "about:blank")` et exposent uniquement le pont `with_ipc_handler`.
  - Chaque WebView d'onglet (`create_webview_for_tab`) est créée **sans `with_ipc_handler`**, impose `validate_navigation_url` (blocage `javascript:`, `file:`, `data:`, `ftp:`, `localhost`, `*.localhost`, loopback/RFC1918/link-local/CGNAT/ULA), refuse les permissions matérielles (`connect_permission_request` → `req.deny()`), et route via `ProxyConfig::Http` sur `SecureLoopbackProxy` (fail-closed sans repli direct).
- **Tests exécutés après les dernières modifications applicatives** :
  - `cargo clippy --workspace --all-targets` : `0` erreur (avertissements uniquement dans l'exemple et test `proxy_server_e2e` existants).
  - `cargo test --workspace` : `13/13` tests unitaires `orakle-core`, `6/6` tests [`phase4_security_and_lifecycle.rs`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/crates/orakle-core/tests/phase4_security_and_lifecycle.rs), `1/1` test [`proxy_server_e2e.rs`](file:///run/media/niceman/Nice/ProjetsWeb/Orakle-Browser/crates/orakle-core/tests/proxy_server_e2e.rs) (`100%` réussis).
  - `npm run typecheck && npm run build` + lancement réel `electron . --no-sandbox` : succès préservé sur la V1.1.
