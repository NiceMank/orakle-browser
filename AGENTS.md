# AGENTS.md — Document de Continuité (Orakle Browser)

## Objectif et Périmètre
Développement d'**Orakle Browser**, navigateur Web desktop complet basé sur Electron (Chromium), React 18, TypeScript et Tailwind CSS v4.

- **Emplacement du dépôt** : `/run/media/niceman/Nice/ProjetsWeb/Orakle-Browser`
- **Mémoire Obsidian partagée** : `Agents/Projets/orakle-browser/Orakle Browser.md`

## Décisions Confirmées
- **2026-10-30** : Choix de l'architecture **Electron + React + TypeScript + Tailwind CSS** avec moteur Chromium embarqué (`<webview>` isolé par partitions `persist:orakle` et `orakle-incognito`).
- **2026-10-30** : Application du système de jetons visuels Glassmorphism iOS/Arc hérité d'Easy Learn (`--ease-ios`, `color-mix`, thèmes Sombre/Clair + 5 accents).
- **2026-10-30** : Intégration native d'**Orakle Shield** (filtrage publicités/traqueurs, suppression des paramètres `utm_*`/`fbclid`/`gclid`, spoofing User-Agent et audit OWASP des en-têtes HTTP).

## État Réel & Vérifications
- Architecture complète créée (`electron/main.ts`, `electron/preload.ts`, `electron/shield.ts`, `electron/storage.ts`, `src/App.tsx`, `src/components/*`).
- Typecheck (`npm run typecheck`) et build de production (`npm run build`) configurés et vérifiés.

## Prochaine Étape
- Tester de nouveaux outils cyber avancés (ex: inspecteur de cookies/stockage, règles de filtrage personnalisées, profils multi-comptes) selon les retours de Farel.
