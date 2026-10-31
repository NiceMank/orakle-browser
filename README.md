# Orakle Browser — v1.1 « Aurora »

**Orakle Browser** est un navigateur Web de bureau (Electron + Chromium + React 18 + TypeScript +
Tailwind CSS v4) pensé pour la **sécurité visible**, la **mémoire du contexte de travail** et le
**confort de lecture** — avec une interface Glassmorphism « Aurora » (halos animés, grain filmique,
verre multi-couches réglable).

---

## Nouveautés v1.1 « Aurora »

### Apparence
- **Design system Aurora** entièrement refondu : nappe de trois halos animés, grain filmique SVG,
  liseré spéculaire sur chaque surface, ombres colorées par l'accent.
- **4 densités de verre** (Léger / Standard / Épais / opacité réduite pour la performance),
  7 couleurs d'accentuation, thèmes **Abyssal** (sombre) et **Cristal** (clair).
- **Onglets en capsules** avec indicateur d'activité dégradé, menu contextuel natif par onglet.
- **Barre d'état** avec signes vitaux (mémoire du heap, charge du renderer, onglets, blocages).
- Mode **mouvement réduit** (accessibilité + perf).

### Fonctionnalités
1. **Orakle Reader — Mode Lecture** (`Ctrl+Shift+R`) : extraction d'article (algorithme type
   Readability), 3 thèmes de lecture (Papier / Sépia / Nuit), taille de police réglable,
   **résumé IA** via Pollinations (gratuit, sans clé) ou endpoint compatible OpenAI
   (`ORAKLE_AI_KEY`), avec **repli 100 % local** (extraction de phrases, aucun réseau).
2. **Espaces de travail (Spaces)** : rail latéral, chaque espace isole ses onglets, compteurs,
   édition du nom / emoji / couleur.
3. **Sessions snapshot** : capturez vos onglets en une session nommée et restaurez-les plus tard.
4. **Orakle Pulse** (`Ctrl+P`) : sonde réseau réelle — DNS, TCP, TLS, TTFB, poids, CDN, note de vitesse.
5. **Inspecteur de cookies & traces** : classification des cookies (traqueur / session), valeur
   masquable, purge par domaine et purge globale.
6. **Mode Focus** : minuteur Pomodoro (15/25/45/60) + **blocage effectif** des domaines distractants
   à la navigation.
7. **Onglets en veille** : libération explicite du processus de rendu, rechargement en un clic.
8. **Omnibox enrichie** : capsule flottante + panneau de suggestions (bang, favoris, onglets ouverts,
   historique flou, actions rapides), 16 bangs (`!gh`, `!cve`, `!so`, `!mdn`, `!yt`…), 6 moteurs
   dont **Perplexity**.
9. **Palette de commandes** (`Ctrl+K`) étendue à toutes les actions, avec recherche floue groupée.
10. **Raccourcis** : `Ctrl+1…9`, `Ctrl+Tab`, `Ctrl+\` (écran scindé), `F11` (Zen), `F12` (DevTools).

### Sécurité (hérité + consolidé)
- **Orakle Shield** : blocage réseau (pub / traqueurs / télémétrie), purge des paramètres `utm_*`,
  `fbclid`, `gclid`…, `DNT: 1` + `Sec-GPC: 1`, profils de User-Agent.
- **Audit OWASP en direct** : notation A+ → F sur CSP, HSTS, X-Frame-Options,
  X-Content-Type-Options, Referrer-Policy, Permissions-Policy.
- **Mode Fantôme** : partition éphémère `orakle-incognito`, aucun historique écrit.
- **Purge sanitaire** granulaire (cache, cookies, localStorage, IndexedDB, service workers, shaders).

---

## Installation et développement

```bash
npm install

npm run dev        # Vite + Electron avec rechargement à chaud
npm run typecheck  # Vérification TypeScript (renderer + main)
npm run build      # Compilation de production
npm start          # Lance l'application compilée
```

### Clé d'API optionnelle (résumés IA via endpoint compatible OpenAI)

```bash
export ORAKLE_AI_KEY="sk-…"
npm start
```

Sans clé, l'assistant utilise **Pollinations** (gratuit) puis bascule sur le résumé
**100 % local** si le réseau est indisponible.

---

## Architecture

```
electron/
  main.ts       # fenêtre, IPC (storage, shield, reader, pulse, cookies), menus contextuels
  preload.ts    # pont contextBridge (API window.orakle typée)
  shield.ts     # filtrage réseau + purge UTM + en-têtes de confidentialité
  storage.ts    # persistance JSON (réglages, favoris, historique, espaces, sessions)
  reader.ts     # extracteur d'article (script injecté) + résumé local extractif
  network.ts    # sonde de latence réseau + résumé IA (Pollinations / OpenAI-compatible)
src/
  App.tsx                    # orchestration (onglets, espaces, sessions, raccourcis)
  components/
    AuroraBackground.tsx     # halos + grain + grille
    TopChrome.tsx            # chrome horizontal / latéral, navigation, omnibox, actions
    Omnibox.tsx              # capsule + suggestions enrichies
    TabStrip.tsx             # onglets capsules + menu contextuel
    SpaceRail.tsx            # rail des espaces de travail
    NewTabPage.tsx           # page d'accueil (horloge, recherche, télémétrie, sessions, notes)
    SidePanel.tsx            # panneaux (favoris, historique, téléchargements, notes, réglages)
    PrivacyPanel.tsx         # inspecteur de cookies & traces
    PulsePanel.tsx           # diagnostic réseau
    SessionsPanel.tsx        # sessions snapshot
    ReaderView.tsx           # mode Lecture + résumé IA
    FocusMode.tsx            # Pomodoro + blocklist
    ShieldModal.tsx          # Orakle Shield + audit OWASP
    CommandPalette.tsx       # Ctrl+K
  styles/index.css           # design system Aurora (jetons 3 couches)
  types/browser.ts           # contrats de données + typage window.orakle
  utils/{url,vitals}.ts      # résolution omnibox, bangs, formatage, signes vitaux
```

## Feuille de route

Voir [`IDEES-FONCTIONNALITES.md`](./IDEES-FONCTIONNALITES.md) — 42 pistes classées par axe,
avec les 5 prochaines priorités et les 3 « signature features » visées.
