# AGENTS.md — Document de Continuité (Orakle Browser)

## Objectif et Périmètre
Développement d'**Orakle Browser**, navigateur Web desktop complet basé sur Electron (Chromium),
React 18, TypeScript et Tailwind CSS v4.

- **Dépôt** : `https://github.com/Nicemank/orakle-browser`
- **Version courante** : `1.1.0` — nom de code **« Aurora »**
- **Mémoire Obsidian partagée** : `Agents/Projets/orakle-browser/Orakle Browser.md`

## Décisions Confirmées
- **2026-10-30** : Architecture **Electron + React + TypeScript + Tailwind CSS** avec moteur Chromium
  embarqué (`<webview>` isolé par partitions `persist:orakle` et `orakle-incognito`).
- **2026-10-30** : Système de jetons visuels Glassmorphism iOS/Arc (`--ease-ios`, `color-mix`,
  thèmes Sombre/Clair + accents).
- **2026-10-30** : Intégration native d'**Orakle Shield** (blocage pub/traqueurs, purge `utm_*`,
  spoofing UA, audit OWASP des en-têtes).
- **2026-10-30 (v1.1)** : Refonte visuelle **« Aurora »** — nappe de halos animés + grain filmique,
  verre multi-couches (`--glass-blur` piloté par `glassDensity`), mode `data-glass="solid"`
  (`reduceTransparency`) pour préserver les performances avec plusieurs `<webview>`.
- **2026-10-30 (v1.1)** : Le chrome UI est un **composant unique** `TopChrome` qui gère les deux
  dispositions (horizontale et rail latéral) ; `App.tsx` ne fait plus que de l'orchestration.
- **2026-10-30 (v1.1)** : Toute fonctionnalité « intelligente » doit avoir un **repli 100 % local**
  (voir `electron/reader.ts` → `offlineSummarize`) — pas de dépendance cloud obligatoire.
- **2026-10-30 (v1.1)** : Aucune clé API stockée côté renderer ; elle est lue via
  `process.env.ORAKLE_AI_KEY` dans le main process.

## État Réel & Vérifications
- Typecheck (`npm run typecheck`) : **OK** (renderer + main).
- Build production (`npm run build`) : **OK** (`dist/` ≈ 305 kB JS / 63 kB CSS).
- Vérification visuelle : rendu contrôlé par captures Playwright (page d'accueil, omnibox,
  palette, réglages, confidentialité, pulse, sessions, focus, lecteur, disposition latérale,
  thème clair) — **aucune erreur console**.
- L'app Electron complète (menu contextuel natif, `<webview>`, IPC réseau) doit être testée sur
  poste avec `npm run dev` : le sandbox de CI n'a pas d'affichage.

## Pièges rencontrés (à ne pas répéter)
- Tailwind v4 + classes maison : `icon-btn`, `orakle-card`, `glass-*` définissent **eux-mêmes**
  `position: relative` / `overflow: hidden`. Pour placer un enfant en absolu, **préfixer `!`
  (`!absolute`, `!fixed`, `!relative`)** sinon le positionnement est écrasé.
- `body` a `font-feature-settings: "tnum" 0` : les horloges doivent ajouter la classe
  `tabular-nums` explicitement.
- Le mode Lecture utilise `color: inherit` dans `.prose-orakle` (sinon le texte hérite de
  `var(--text)` du thème sombre et devient invisible sur fond papier).
- Ne pas mettre `backdrop-filter` sur de grandes surfaces contenant des `<webview>` :
  utiliser `reduceTransparency` ou `bg-[var(--bg-glass-strong)]`.

## Prochaine Étape
Priorités issues de `IDEES-FONCTIONNALITES.md` :
1. Sélection → questions contextuelles + extraction de tableaux en CSV.
2. Score d'empreinte navigateur (fingerprint) + rapport de sécurité PDF exportable.
3. Timeline visuelle d'historique avec miniatures.
4. Veille de page (« Surveiller cette page ») + notifications de diff.
5. API locale (`127.0.0.1`) pour automatiser onglets / sessions depuis un script.
