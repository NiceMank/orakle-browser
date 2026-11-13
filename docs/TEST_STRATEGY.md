# Orakle Browser — Stratégie de Test et Critères d'Acceptation

- **Version** : 2.0.0
- **Statut** : Document de test et assurance qualité (aligné Phase 4)
- **Dernière mise à jour** : 2026-11-13

---

## 1. Principes de Vérification

1. **Aucun Résultat Fictif** :
   Toute mesure de performance (démarrage, RAM, CPU) ou résultat de test doit être reproductible et mesuré sur un environnement identifié.
2. **Pyramide des Tests** :
   - **Tests Unitaires (Rust)** : validation des algorithmes de filtrage d'URL, de purge UTM, de sérialisation et des requêtes SQLite.
   - **Tests d'Intégration** : validation des échanges IPC entre l'UI et le cœur Rust, synchronisation des états d'onglets.
   - **Tests de Bout en Bout (E2E)** : parcours de navigation réels (saisie d'URL, ouverture d'onglet, split view, favoris).

---

## 2. Niveaux de Tests et Outils

| Niveau de Test | Cible | Outils & Commandes | Critères de Réussite |
|---|---|---|---|
| **Analyse Statique & Linting** | Cœur Rust, Extension C/Rust & Interface | • `cargo clippy --workspace --all-targets`<br>• `cargo fmt --check`<br>• `npm run typecheck` | Zéro erreur Clippy, formatage conforme, TypeScript strict OK. |
| **Tests Unitaires, Sécurité & E2E Rust** | Modules `storage` v2, `shield`, `pulse`, `proxy`, `profile`, `adapters` + `phase4_security_and_lifecycle.rs` + `proxy_server_e2e.rs` | • `cargo test --workspace` | 100% de réussite (13 unitaires + 6 sécurité/cycle de vie + E2E HTTP/CONNECT/SOCKS5/IPv6/fail-closed). |
| **Harnais WebView & HTTP/3** | Routage réel `WebKit2GTK 4.1` & `Chromium` NetLog | • `xvfb-run -a python3 scripts/verify-webview-proxy.py`<br>• `chromium --headless --proxy-server=http://... --log-net-log` | HTML/JS/SVG/Fetch/302/HTTPS routés, blocages & exceptions `@@` effectifs, 0 session QUIC (`h2` dans `CONNECT`). |
| **Tests, Captures & Benchmarks `orakle-app`** | Hôte `tao` + `wry` multi-WebViews, Extension `liborakle_webkit_ext.so`, Split View, Suspension & `--bench` | • `GDK_BACKEND=x11 xvfb-run -a ./target/release/orakle-app --verify --capture-dir docs/VALIDATION/screenshots`<br>• `systemd-run --user --scope -p MemoryAccounting=yes ... --bench --runs 3` | Extension active (`natif=5`), `web_procs` 4→3→4, `priv_leaked=false`, RSS/PSS/cgroup/CPU mesurés sur 8 scénarios (`PHASE4-REPORT.md`). |
| **Non-régression V1.1** | Interface Electron héritée (`v1.1-aurora-final`) | • `npm run typecheck && npm run build`<br>• `xvfb-run -a timeout 8s ./node_modules/.bin/electron . --no-sandbox` | Compilation et lancement réel sans crash préservés. |
| **Sécurité des Dépendances** | Vulnérabilités connues (CVE) | • `cargo audit`<br>• `npm audit` | Zéro faille critique ou haute non mitigée (cible CI). |

---

## 3. Critères d'Acceptation par Fonctionnalité

Chaque fonctionnalité livrée doit satisfaire à la matrice suivante avant d'être considérée comme prête pour la distribution :

| Fonctionnalité | Critères d'Acceptation Obligatoires |
|---|---|
| **Navigation & Omnibox** | 1. Saisie d'une URL complète (`https://...`) : charge la page sans erreur.<br>2. Saisie d'un mot-clé : lance la recherche sur le moteur configuré.<br>3. Boutons Précédent/Suivant : reflètent fidèlement l'historique.<br>4. Indicateur SSL : affiche le statut HTTPS réel certifié. |
| **Gestion des Onglets** | 1. `Ctrl+T` ouvre un nouvel onglet sans blocage.<br>2. `Ctrl+W` ferme l'onglet et donne le focus à l'onglet adjacent.<br>3. `Ctrl+Shift+T` restaure le dernier onglet fermé avec son URL exacte.<br>4. L'épinglage maintient l'onglet à gauche sans bouton de fermeture accidentelle. |
| **Split View** | 1. Affiche deux pages simultanément sans interférence de défilement.<br>2. Redimensionnement fluide de la séparation centrale à la souris.<br>3. Fermeture de l'un des volets restaure la vue plein écran sans rechargement de l'autre. |
| **Orakle Shield** | 1. Intercepte les requêtes vers les domaines de pistage connus.<br>2. Supprime les paramètres `utm_*` et `fbclid` lors des redirections.<br>3. Permet de désactiver le bouclier sur un domaine en un clic sans recharger tout le navigateur. |
| **Thèmes & Accessibilité** | 1. Bascule instantanée Clair / Sombre / Système persistante après redémarrage.<br>2. Navigation au clavier fluide (`Tab`, flèches, `Enter`, `Escape`) avec outline visible.<br>3. Contraste de texte conforme WCAG 2.2 AA. |
