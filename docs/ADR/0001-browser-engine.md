# ADR 0001 : Choix du moteur de rendu et de l'architecture d'exécution d'Orakle Browser

- **Statut** : Recommandé pour validation
- **Date** : 2026-11-11
- **Auteurs** : Équipe d'architecture Orakle Browser
- **Décideurs** : Farel (Product Owner) & Antigravity (Architecte Système)

---

## 1. Contexte et Problématique

Orakle Browser a pour ambition de devenir un navigateur Web grand public et professionnel indépendant, rapide, respectueux de la vie privée, ergonomique et utilisable quotidiennement par des centaines de milliers d'utilisateurs.

Une distinction fondamentale doit être établie d'emblée :
> **Le langage applicatif (Rust)** n'est pas le **moteur de rendu Web (Web Engine)**.

Développer un moteur de rendu Web complet capable d'afficher le Web moderne (moteur HTML5, moteur de style CSS3/Flex/Grid, arbre de rendu/layout, rasteriseur 2D/GPU, moteur JavaScript ECMAScript 2026, pipeline multimédia, WebGL/WebGPU, WebAssembly, WebRTC et DRM Widevine) représente un investissement colossal de centaines de millions de dollars et plusieurs décennies d'ingénierie (Chromium compte ~35 millions de lignes de code C++, Gecko ~20 millions, WebKit ~15 millions).

Le choix architectural du moteur conditionne :
1. **La compatibilité Web réelle** : l'utilisateur peut-il consulter YouTube, Google Docs, des banques en ligne, des plateformes SaaS sans dysfonctionnement ?
2. **La sécurité et la fréquence des correctifs** : les CVE zero-day du Web sont-elles corrigées en quelques jours ?
3. **Le contrôle sur le réseau et la vie privée** : le navigateur peut-il intercepter, filtrer et bloquer le pistage à bas niveau ?
4. **L'empreinte mémoire et le poids de distribution** : l'application est-elle légère et rapide à installer ?
5. **La portabilité** : le comportement est-il cohérent entre Linux, Windows et macOS ?

---

## 2. Évaluation Comparative des Trois Options

### Option A : Tauri v2 + WRY (Moteur WebView natif par système piloté en Rust)

*Principe* : Le processus hôte, la gestion des fenêtres, le stockage, les règles de blocage et l'orchestration sont écrits en **Rust pur**. L'affichage des pages Web s'appuie sur la bibliothèque officielle **WRY** (Webview Rendering LibrarY), qui délègue le rendu au composant système disponible :
- **Linux** : WebKitGTK (`webkit2gtk-4.1`).
- **Windows** : WebView2 (moteur Chromium/Edge Evergreen, installé nativement sur 99% des postes Windows 10 et 11).
- **macOS** : WKWebView (moteur WebKit natif Apple).

| Critère | Évaluation Option A |
|---|---|
| **Avantages** | • Binaire natif extrêmement compact (**15 à 25 Mo** vs ~150 Mo sur Electron).<br>• Empreinte RAM minimale (**30 à 60 Mo** au repos contre 300+ Mo).<br>• Sécurité mémoire totale du code hôte garantie par Rust (zéro vulnérabilité use-after-free ou buffer overflow).<br>• Compilation et packaging rapides en packages natifs Linux (.deb, .rpm, AppImage), Windows (.msi, .exe) et macOS (.dmg).<br>• Aucun runtime Node.js embarqué. |
| **Inconvénients** | • **Hétérogénéité des moteurs** : le rendu est WebKit sur Linux/macOS mais Chromium sur Windows.<br>• **Limitations d'interception réseau** : WRY permet l'enregistrement de protocoles personnalisés (`custom_protocol`), mais ne dispose pas d'une API de filtrage de requêtes globales aussi granulaire que l'API `webRequest` de Chromium/Electron sans extensions natives C++ spécifiques.<br>• **Gestion multi-onglets lourde sur Linux** : chaque instance `WebView` WebKitGTK démarre un processus `WebKitWebProcess` distinct, ce qui engendre une surcharge mémoire si l'on tente d'ouvrir 30 WebViews natives simultanées sans gestionnaire de mise en veille explicite. |
| **Risques** | Différences de rendu de certains sites web entre Linux (WebKitGTK) et Windows (WebView2) ; complexité d'implémentation d'un bloqueur de pub au niveau du flux réseau sans proxy local ou script d'injection. |
| **Coût de maintenance** | Modéré. Les mises à jour de sécurité des moteurs sont assurées par les distributions Linux (mises à jour WebKitGTK) et par Microsoft (mises à jour automatiques silencieuses de WebView2). |
| **Compatibilité Web** | **95% à 98%** (excellente sur Windows via WebView2 ; très bonne sur Linux/macOS via WebKit, mais quelques limitations sur les codecs propriétaires ou le DRM sans paquets multimédias additionnels). |
| **Difficulté de dev** | Moyenne à élevée. |

---

### Option B : Servo (Moteur de rendu 100% Rust)

*Principe* : Utiliser le moteur Servo, développé historiquement par Mozilla et soutenu par la Linux Foundation Europe, entièrement écrit en Rust avec parallélisme des tâches de layout et de rendu.

| Critère | Évaluation Option B |
|---|---|
| **Avantages** | • Pureté architecturale : 100% Rust du fenêtrage au moteur de rendu.<br>• Parallélisme de rendu novateur.<br>• Indépendance absolue vis-à-vis des monopoles Google (Chromium) et Apple (WebKit). |
| **Inconvénients** | • **Incompatibilité majeure avec le Web moderne** : Servo ne supporte pas encore l'intégralité des spécifications HTML5/CSS3 modernes, les formulaires complexes, WebRTC complet, WebGPU, ou les DRM vidéo (Widevine).<br>• Des sites fondamentaux comme Google Docs, YouTube, Microsoft Office 365, Discord Web ou des banques en ligne ne fonctionnent pas ou génèrent des erreurs de rendu bloquantes.<br>• Le moteur JavaScript utilisé par Servo reste SpiderMonkey (C++), nécessitant une chaîne de compilation C++ très lourde (`rust-mozjs`). |
| **Risques** | **Inutilisabilité quotidienne** pour un utilisateur grand public. Si Orakle était basé sur Servo aujourd'hui, 80% des utilisateurs désinstalleraient le navigateur dès la première heure en constatant des pages cassées. |
| **Coût de maintenance** | Extrêmement élevé : obligation de corriger soi-même des bugs de layout CSS ou des fonctions DOM manquantes. |
| **Compatibilité Web** | **40% à 55%** (inadéquat pour un navigateur grand public en 2026). |
| **Difficulté de dev** | Critique / Bloquante pour un produit de production. |

---

### Option C : Chromium Embedded Framework (CEF) / Chromium via Rust

*Principe* : Intégrer le moteur Chromium complet via CEF (C++) interfacé avec du code Rust par FFI (crates comme `cef-rs` ou wrapper C ABI sur-mesure).

| Critère | Évaluation Option C |
|---|---|
| **Avantages** | • **Compatibilité Web à 100%** garantie (YouTube, Netflix avec DRM, WebGL/WebGPU, WebRTC, PWA).<br>• Comportement identique et prévisible sur tous les systèmes d'exploitation (Linux, Windows, macOS).<br>• Puissance totale d'interception réseau (`CefRequestHandler`, `CefResourceRequestHandler`) permettant un bloqueur de pub/traqueurs d'une précision chirurgicale. |
| **Inconvénients** | • Poids de l'application très élevé : **180 à 250 Mo** de binaires embarqués.<br>• Consommation mémoire plus importante : ~200 à 400 Mo au repos.<br>• Complexité de liaison FFI Rust <-> C++ : gestion des cycles de vie des objets, boucles d'événements croisées (`Tokio` vs `CefMessageLoop`), manipulation délicate des pointeurs et des threads. |
| **Risques** | Nécessite un pipeline d'intégration continue lourd pour suivre les versions de Chromium et intégrer les correctifs de sécurité critiques dans les 72 heures. |
| **Coût de maintenance** | Élevé sur l'infrastructure de compilation, mais faible sur la compatibilité Web. |
| **Compatibilité Web** | **100%**. |
| **Difficulté de dev** | Élevée (pont C++/Rust). |

---

## 3. Analyse de l'État Actuel du Codebase

Le prototype existant (`orakle-browser v1.1.0`) s'appuie sur **Electron 33 + Chromium + React 18 + Tailwind v4** :
- **Points forts avérés** : Navigation multi-onglets réelle, tag `<webview>` opérationnel, interception réseau active via `webRequest` dans `electron/shield.ts`, persistance locale JSON, mode lecture avec extraction Readability, sonde réseau HTTP dans `electron/network.ts`.
- **Faiblesses majeures** :
  1. Dépendance à Node.js dans le processus hôte.
  2. Binaire lourd (~180 Mo).
  3. Lenteur relative du démarrage par rapport à un binaire compilé natif en Rust.
  4. L'interface précédente souffrait de dérives graphiques (effet de verre trop envahissant, vocabulaire cyberpunk non professionnel).

---

## 4. Décision Recommandée : Stratégie en Deux Paliers (Architecture Hybride Maîtrisée)

Pour bâtir un produit stable, honnête et distribuable sans rupture de service pour les utilisateurs :

### Palier 1 : Architecture Système Rust Modulaire avec Moteur Web Découplé (Approche Recommandée)

1. **Le Cœur Métier est 100% Rust** (`crates/orakle-core`) :
   - Gestionnaire de sessions et persistance : base transactionnelle locale SQLite chiffrée via `rusqlite`.
   - Moteur de filtrage anti-pistage : crate Rust officiel `adblock` (moteur de Brave avec parsing EasyList et filtres de Bloom ultra-rapides en mémoire).
   - Moteur de sonde de latence et audit réseau : crate `reqwest` + `rustls` (TLS 1.3 natif sans OpenSSL).
   - Gestionnaire d'espaces de travail (*Spaces*), signets (*Bookmarks*), historique et profils.

2. **L'Interface Utilisateur (UI Shell)** :
   - Entièrement refondue selon la nouvelle charte graphique sobre, épurée et professionnelle (palette graphite/ardoise/blanc cassé, accent bleu pétrole unique, typographie moderne et contrastes WCAG AA).
   - Découplée de tout moteur de rendu propriétaire.

3. **Le Moteur de Rendu Web** :
   - **Phase Immédiate (V2.0)** : Adoption de **Tauri v2 + WRY** pour les builds légers (Windows WebView2 / Linux WebKitGTK / macOS WKWebView), avec architecture à deux WebViews actives (onglet courant + Split View) et mise en veille des onglets d'arrière-plan, garantissant une empreinte RAM inférieure à 60 Mo.
   - **Alternative de secours conservée** : Possibilité de router vers une runtime Chromium embarquée si un utilisateur professionnel requiert des extensions Chrome ou des codecs DRM propriétaires.

### Alternatives Rejetées

- **Servo comme moteur principal** : **Rejeté fermement**. Servo ne permet pas aujourd'hui de regarder une visioconférence, d'utiliser Google Drive ou d'afficher un site bancaire moderne. Proposer Servo comme moteur principal en promettant un usage grand public relèverait du mensonge technique.
- **Réécriture d'un moteur HTML/CSS from scratch** : **Rejeté**. Irréalisable pour une équipe sans des centaines d'ingénieurs à temps plein sur 10 ans.

---

## 5. Conséquences de la Décision

- **Positives** :
  - Orakle devient un vrai binaire natif Rust compilé, rapide et sécurisé.
  - La taille de l'installateur passe de ~160 Mo à moins de ~25 Mo.
  - La mémoire au repos chute drastiquement sous la barre des 60 Mo.
  - L'architecture est honnête : nous indiquons clairement les composants natifs Rust et le moteur de rendu utilisé.
- **Défis techniques à relever** :
  - Maîtriser le compositage du Split View sous WebKitGTK (Linux) et WebView2 (Windows).
  - Implémenter le filtrage Adblock en amont au niveau du protocole personnalisé WRY ou du proxy de boucle locale géré par Tokio.
