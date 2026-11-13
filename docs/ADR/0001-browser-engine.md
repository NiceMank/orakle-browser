# ADR 0001 : Choix du moteur de rendu et de l'architecture d'exécution d'Orakle Browser

- **Statut** : Validé en Phase 3 (Cœur Rust `orakle-core`, Hôte `orakle-app` multi-WebViews, Preuves Proxy WebKit2GTK & Chromium)
- **Date** : 2026-11-13 (révision Phase 3)
- **Auteurs** : Équipe d'architecture Orakle Browser
- **Décideurs** : Farel (Product Owner) & Antigravity (Architecte Système)

---

## 1. Contexte et Problématique

Orakle Browser a pour ambition de devenir un navigateur Web grand public et professionnel indépendant, rapide, respectueux de la vie privée, ergonomique et utilisable quotidiennement par des centaines de milliers d'utilisateurs.

Une distinction fondamentale et impérative doit être établie d'emblée :
> **Le langage applicatif hôte (Rust)** n'est pas le **moteur de rendu Web (Web Engine)**.

Développer un moteur de rendu Web complet capable d'afficher le Web moderne (moteur HTML5, moteur de style CSS3/Flex/Grid, arbre de rendu/layout, rasteriseur 2D/GPU, moteur JavaScript ECMAScript 2026, pipeline multimédia, WebGL/WebGPU, WebAssembly, WebRTC et DRM Widevine) représente un investissement colossal de centaines de millions de dollars et plusieurs décennies d'ingénierie (Chromium compte ~35 millions de lignes de code C++, Gecko ~20 millions, WebKit ~15 millions).

Le choix architectural du moteur conditionne :
1. **La compatibilité Web réelle** : l'utilisateur peut-il consulter YouTube, Google Docs, des banques en ligne, des plateformes SaaS sans dysfonctionnement ?
2. **La sécurité et la fréquence des correctifs** : les CVE zero-day du Web sont-elles corrigées en quelques jours par les mainteneurs du moteur ?
3. **Le contrôle sur le réseau et la vie privée** : le navigateur peut-il intercepter, filtrer et bloquer le pistage subressource à bas niveau ?
4. **L'empreinte mémoire et le poids de distribution** : l'application est-elle légère et rapide à installer ?
5. **La portabilité** : le comportement est-il cohérent entre Linux, Windows et macOS ?

---

## 2. Évaluation Comparative Rigoureuse des Trois Options

### Qualification Méthodologique des Métriques
Afin de bannir tout chiffre inventé ou dogmatique, chaque valeur est qualifiée selon son statut réel :

| Métrique / Affirmation | Source | Méthode & Environnement | Statut |
|---|---|---|---|
| **Vitesse de filtrage `adblock`** | `poc_validation` (release) & `orakle-core` (debug) | 10 000 req. `adblock` v0.13.5 (release) ; 5 000 req. `orakle-core` (debug, Linux 6.18 x86_64) | **Mesuré : 0,824 µs/req. (release) \| p50=52,73 µs, p95=139,92 µs (debug)** |
| **Vitesse d'insertion SQLite** | `poc_validation` (release) & `orakle-core` (debug) | 1 000 insertions en transaction WAL (`rusqlite` v0.33) | **Mesuré : 3,91 µs/entrée (release) \| 19,04 µs/entrée (debug)** |
| **Sonde Pulse réelle (5 échantillons)** | `orakle-core::pulse` (`test_real_pulse_probe_and_stats`) | `tokio::net` + `tokio-rustls` vers `https://doc.rust-lang.org/` | **Mesuré : DNS p50=5,66 ms \| TCP p50=114,00 ms \| TLS p50=206,30 ms \| TTFB p50=212,47 ms** |
| **Poids binaire `orakle-app`** | Build `--release` `crates/orakle-app` | Binaire ELF x86_64 (`cargo build -p orakle-app --release`, hors WebKitGTK OS) | **Mesuré : 8,8 Mio brut / 6,9 Mio strippé** |
| **Poids installateur Electron v1.1** | Mesure locale dépôt Orakle | Répertoire `dist-electron` généré | **Mesuré : 174 Mo compressé / 380 Mo extrait** |
| **Empreinte RAM (`orakle-app --verify`)** | `/proc/*/status` (`VmRSS`) | 3 WebViews actives (Chrome + Split 50/50) sous WebKit2GTK 2.52.6 (`xvfb-run`) | **Mesuré : 903,2 Mio total (184,9 Mio hôte + 718,3 Mio WebKitWebProcess)** |
| **Libération RAM Tab Discarding** | `orakle-app --verify` | Destruction du volet Split droit vers SQLite WAL (`VmRSS`) | **Mesuré : -181,7 Mio (-186 020 Kio) \| 721,5 Mio restants \| 919,5 Mio après restauration** |
| **Empreinte RAM Servo / CEF / Electron** | — | Non exécutés dans le même harnais de benchmark local | **Non mesuré localement** |
| **Compatibilité Web Servo / CEF / WRY** | — | Suite W3C Web Platform Tests non exécutée localement | **Non mesuré localement** |

---

### Option A : Tauri v2 + WRY (Moteur WebView système piloté en Rust)

*Principe* : Le processus hôte, la gestion des fenêtres, le stockage, les règles de blocage et l'orchestration sont écrits en **Rust pur**. L'affichage des pages Web s'appuie sur la bibliothèque officielle **WRY**, qui délègue le rendu au composant système disponible :
- **Linux** : WebKitGTK (`webkit2gtk-4.1`).
- **Windows** : WebView2 (Chromium Edge Evergreen).
- **macOS** : WKWebView (WebKit natif Apple).

#### Analyse Détaillée des Capacités & Limitations Réelles :
- **Multi-WebViews & Split View (`tao 0.37` + `wry 0.57` dans `crates/orakle-app`)** :
  - *Faisabilité* : **Démontrée empiriquement**. L'hôte natif `orakle-app` instancie 3 WebViews simultanées dans une même fenêtre (`chrome` + `left` + `right` Split View 50/50) avec redimensionnement dynamique via `set_bounds`.
  - *Constat critique Linux (Wayland vs X11)* : Sous Linux Wayland, `WebViewBuilder::build_as_child(&window)` échoue avec `UnsupportedWindowHandle` (Xlib requis). L'implémentation retenue (`crates/orakle-app/src/main.rs`) utilise `tao::platform::unix::WindowExtUnix::default_vbox` + `gtk::Fixed` + `WebViewBuilderExtUnix::build_gtk(&fixed)` avec `webkit_web_view` encapsulé dans un conteneur dimensionnable, fonctionnel sous X11/Xvfb et Wayland.
- **Architecture de Filtrage à 3 Niveaux (Correction Phase 3)** :
  Le Proxy Loopback Rust ne prétend pas remplacer à lui seul un bloqueur de sous-ressources HTTPS complet. Orakle distingue désormais trois couches complémentaires :
  1. **Niveau 1 — Proxy Loopback Rust (`orakle-core::proxy`, `127.0.0.1:<port>`)** :
     - Intercepte les tunnels `HTTPS CONNECT` (par domaine/port, **Zero-MITM**) et les URLs complètes en HTTP clair, **avant toute résolution DNS** afin d'éviter les fuites DNS sur domaines bloqués, tout en appliquant un verrou SSRF anti-DNS-rebinding.
     - En échec ou erreur amont, le proxy agit en **fail-closed** (`502 Bad Gateway` / `403 Forbidden`, zéro fuite silencieuse vers le réseau direct).
     - *Vérification HTTP/3 & QUIC* : Test empirique mené sur `chromium --headless --proxy-server=http://127.0.0.1:<port> --log-net-log` vers `https://cloudflare-quic.com/`. Le NetLog confirme **`0` session QUIC active** (`QUIC_CONNECTION_CANCELLED`) et **8 sessions `HTTP2_SESSION_INITIALIZED` (`protocol: "h2"`)** routées à l'intérieur du tunnel `CONNECT`.
     - *Nuance WebSocket sous WebKit2GTK 4.1* : Lors de la validation `scripts/verify-webview-proxy.py`, `WebKit2GTK` (`libsoup`) rejette `ws://` lorsque seul un proxy `http://` est configuré, **sans fuite directe** vers le serveur local (`ORIGIN_A_HITS` sans `/ws`). La prise en charge complète de `ws://` sous WebKitGTK nécessitera l'ajout d'un mode **SOCKS5** dans `orakle-core::proxy` ou le passage en `wss://` (`CONNECT`).
  2. **Niveau 2 — Adaptateurs Natifs OS (Sous-ressources HTTPS & types de ressources)** :
     - **Windows (`WebView2`)** : `CoreWebView2.WebResourceRequested` relié à `adblock-rust` (URL complète + type de ressource).
     - **Linux (`WebKitGTK`)** : `WebKitWebExtension` (`.so` injectée dans le processus Web) écoutant le signal `send-request` et interrogeant le moteur `adblock-rust` (par IPC ou moteur partagé).
     - **macOS (`WKWebView`)** : Compilation vers `WKContentRuleListStore` (syntaxe WebKit Content Blocker compilée).
  3. **Niveau 3 — Règles Avancées EasyList / uBlock & Matrice de Compatibilité Honnête** :
     Toutes les règles uBlock/EasyList ne sont pas traduisibles uniformément selon le moteur cible :

     | Catégorie de règle | Proxy Rust (Niveau 1) | WebView2 `WebResourceRequested` | WebKitGTK `WebKitWebExtension` | WKWebView `WKContentRuleList` |
     |---|---|---|---|---|
     | Blocage par domaine / `CONNECT` | **Oui (TLS/HTTP)** | **Oui** | **Oui** | **Oui** |
     | Blocage chemin/URL + type (`$script`, `$image`) | HTTP uniquement (HTTPS chiffré) | **Oui** | **Oui** | **Oui** (subset WebKit) |
     | Exceptions `@@` (allowlist) | **Oui (domaine en TLS, URL en HTTP)** | **Oui** | **Oui** | **Partiel** (`ignore-previous-rules`) |
     | `$third-party` / `$domain=` | HTTP uniquement (`Referer`/`Origin`) | **Oui** | **Oui** | **Oui** (`if-domain`/`unless-domain`) |
     | Cosmétique CSS (`##.ad`, `#@#`) | **Non** | Injection JS/CSS (`initialization_script`) | `WebKitUserContentManager` (`UserStyleSheet`) | `WKUserContentController` |
     | Scriptlets uBlock (`##+js(...)`) | **Non** | Injection JS ciblée (subset validé) | `WebKitUserScript` (subset validé) | `WKUserScript` (subset validé) |
     | Modificateurs `$redirect`, `$csp`, `$removeparam` | HTTP uniquement (pas de MITM TLS) | **Partiel** (`WebResourceRequested` + en-têtes) | **Partiel** (`send-request` limité sur réponse) | **Limité / Non** (`$removeparam` via Rust/navigation) |

- **Gestion des sessions & Profils** :
  - Chaque profil dispose d'un `data_directory` isolé. Le mode incognito dans `orakle-app` utilise `with_incognito(true)` (store non persistant en RAM purgé à la fermeture, validé sur le volet droit Split View).
- **Gestion de la mémoire & suspension des onglets** :
  - Destruction réelle de l'instance `WebView` inactive (et de son widget GTK sous Linux) pour libérer le processus de rendu (`WebKitWebProcess`). Dans `orakle-app --verify`, la suspension d'un volet Split View vers SQLite WAL a réduit la RAM `VmRSS` totale de **903,2 Mio à 721,5 Mio (-181,7 Mio / -186 020 Kio)**, puis restauré le volet à **919,5 Mio**. L'état d'onglet est sérialisé dans SQLite WAL (`crates/orakle-core/src/storage.rs`).

| Avantages | Inconvénients |
|---|---|
| • Binaire hôte léger mesuré (**8,8 Mio brut / 6,9 Mio strippé**).<br>• Isolation multi-processus système + libération RSS mesurée (**-181,7 Mio** à la suspension d'un volet).<br>• Mises à jour de sécurité des moteurs assurées par l'OS (Apple, Microsoft, distributions Linux).<br>• Cœur métier 100% Rust memory-safe (`Send + Sync`, zéro MITM TLS). | • Rendu hétérogène (WebKit sous Linux/macOS, WebView2 Chromium sous Windows).<br>• Empreinte RAM initiale réelle de **WebKit2GTK** élevée avec 3 WebViews actives (**903,2 Mio RSS** mesuré en harnais `xvfb-run`).<br>• Nécessite des adaptateurs OS natifs (Niveau 2) pour le filtrage par chemin URL/type sous HTTPS et un support SOCKS5 pour `ws://` sous WebKitGTK. |

---

### Option B : Servo (Moteur de rendu 100% Rust)

*Principe* : Moteur expérimental de rendu HTML/CSS entièrement écrit en Rust, soutenu par la Linux Foundation Europe.

#### Analyse Détaillée au 12 Novembre 2026 :
- **État d'avancement réel** :
  - Excellentes avancées sur le parallélisme de layout (Stylo) et WebGL de base.
  - Cependant, Servo utilise toujours **SpiderMonkey (`mozjs`) en C++** pour JavaScript, ce qui infirme l'affirmation d'une pureté 100% Rust du moteur.
  - **Limitations bloquantes pour un usage quotidien** :
    1. *Absence de DRM (Widevine / EME)* : Aucun service de streaming protégé (Netflix, Spotify, flux YouTube protégés) ne fonctionne.
    2. *WebRTC incomplet* : Visioconférences (Google Meet, Discord, Zoom Web) impossibles.
    3. *APIs Web modernes manquantes* : Échec d'affichage ou crashs sur des applications SaaS complexes (Google Docs, Figma, suites bureautiques Web).
    4. *Gestion de sessions et profils* : L'API d'embarquement (`libservo`) n'est pas encore conçue pour gérer un navigateur complet multi-onglets avec conteneurs isolés.
- **Verdict** : **Rejeté pour la production grand public**. Baser Orakle sur Servo en 2026 rendrait le navigateur inutilisable pour 80% des tâches quotidiennes des utilisateurs. Servo est conservé en veille technologique R&D.

---

### Option C : Chromium Embedded Framework (CEF)

*Principe* : Moteur Chromium complet (Blink + V8 + Chromium Network Stack) interfacé en C++/Rust.

#### Analyse Détaillée :
- **Forces** :
  - **100% de compatibilité Web** : Tous les sites, applications et extensions du Web fonctionnent sans anomalie.
  - **Contrôle réseau total** : `CefResourceRequestHandler::OnBeforeResourceLoad` permet de brancher `adblock-rust` nativement sur 100% des requêtes réseau sur tous les OS.
  - **Gestion native des profils** : `CefRequestContext` gère des conteneurs isolés avec support natif du mode privé en mémoire vive.
- **Inconvénients critiques** :
  - **Poids massif** : Binaires Chromium de 180 à 250 Mo à distribuer.
  - **Consommation RAM** : ~200 à 450 Mo dès le démarrage.
  - **Complexité FFI C++/Rust** : Risque de fuites mémoire ou d'instabilités au niveau de la synchronisation des boucles de messages (`Tokio` vs `CefMessageLoop`).
- **Verdict** : **Solution de référence et de réserve**. Si une déclinaison d'Orakle destinée aux entreprises ou aux utilisateurs exigeant des extensions Chrome ou des DRM propriétaires est requise, CEF sera déployé sous forme de runtime alternatif.

---

## 3. Décision Recommandée : Architecture V2 Hybride Maîtrisée (Piste B : `orakle-core` + `tao`/`wry`)

À la lumière des preuves empiriques apportées en Phases 1–3 :

1. **Socle Système Orakle Core (`crates/orakle-core`, Rust)** :
   - Bibliothèque modulaire thread-safe (`Send + Sync`, sans feature `single-thread`) :
   - Module `storage` : persistance SQLite WAL transactionnelle (`rusqlite` 0.33, validée à **3,91 µs/entrée** release / **19,04 µs/entrée** debug sur 1 000 entrées).
   - Module `shield` : moteur `adblock` v0.13.5 (**0,824 µs/requête** release sur 10k req. ; `p50=52,73 µs` / `p95=139,92 µs` debug sur 5k req.).
   - Module `pulse` : sonde réelle DNS/TCP/TLS/TTFB via `tokio::net` + `tokio-rustls` (`ring`), sans pourcentages simulés (5 échantillons `doc.rust-lang.org` : Total `p50=613,22 ms`, `p95=902,07 ms`).
   - Module `proxy` : Proxy Loopback `127.0.0.1` fail-closed anti-SSRF / anti-DNS-rebinding (ordre de validation IP littérale → Shield avant DNS → résolution + vérification IP avant connect) + télémétrie atomique (`ProxyTelemetry`).

2. **Hôte Natif Multi-WebViews (`crates/orakle-app`, `tao 0.37` + `wry 0.57`)** :
   - Binaire hôte release mesuré à **8,8 Mio brut / 6,9 Mio strippé**.
   - 3 WebViews simultanées (Chrome UI + Split View 50/50 avec volet droit incognito) reliées au proxy HTTP local, avec suspension SQLite WAL vérifiée à **-181,7 Mio `VmRSS`**.
   - Évolutif vers les adaptateurs Niveau 2 (`WebView2`, `WebKitWebExtension`, `WKContentRuleListStore`) et un support **SOCKS5** pour `ws://` sous WebKitGTK.

3. **Garantie d'Honnêteté Technique** :
   - Zéro note ou fausse certification "OWASP AAA" : restitution factuelle des en-têtes HTTP de sécurité réels sans notation arbitraire.
   - Zéro compteur d'octets économisés basé sur une constante arbitraire (+28,5 Ko) : suppression définitive de l'indicateur décoratif au profit des compteurs exacts `ProxyTelemetry`.
   - Zéro fraction mathématique du TTFB pour Pulse : restitution des percentiles réels `p50`/`p95` mesurés par phase.
