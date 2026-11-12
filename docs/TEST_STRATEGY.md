# Orakle Browser — Stratégie de Test et Critères d'Acceptation

- **Version** : 2.0.0
- **Statut** : Document de test et assurance qualité
- **Dernière mise à jour** : 2026-11-11

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
| **Analyse Statique & Linting** | Cœur Rust & Interface | • `cargo clippy --all-targets -- -D warnings`<br>• `cargo fmt --check`<br>• `npm run typecheck` | Zéro avertissement, formatage conforme. |
| **Tests Unitaires Rust** | Modules `storage`, `shield`, `network`, `tabs` | • `cargo test --workspace` | 100% de réussite sur les cas nominaux et d'erreur. |
| **Sécurité des Dépendances** | Vulnérabilités connues (CVE) | • `cargo audit`<br>• `npm audit` | Zéro faille critique ou haute non mitigée. |
| **Tests d'Intégration UI** | Composants React & Navigation | • Vitest / React Testing Library | Vérification des interactions clavier et états accessibles. |
| **Mesures de Performance Réelles** | Processus en exécution | • Mesure de la mémoire via `/proc/$PID/status` (VmRSS)<br>• Outil de profilage natif | • Démarrage à froid < 400 ms.<br>• Mémoire au repos (1 onglet) < 80 Mo. |

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
