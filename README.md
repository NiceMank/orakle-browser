# Orakle Browser

**Orakle Browser** est un navigateur Web de bureau moderne, rapide et sécurisé (Electron + Chromium + React + TypeScript + Tailwind CSS v4), doté d'un bouclier anti-pistage natif (**Orakle Shield**), d'un auditeur d'en-têtes de sécurité OWASP en direct, d'un mode écran scindé (**Split View**) et d'une esthétique Glassmorphism inspirée du design system d'Easy Learn et d'Arc Browser.

---

## Fonctionnalités Clés (V1.0.0)

1. **Moteur Multi-Onglets & Mode Fantôme (Navigation Privée Isolée)**
   - Onglets illimités avec indicateur de chargement, favicon, contrôle du son par onglet (Mute/Unmute), épinglage par double-clic (`Pin`) et restauration d'onglet fermé (`Ctrl+Shift+T`).
   - **Onglets Fantômes (`Ctrl+Shift+N`)** : isolés dans une partition mémoire éphémère (`orakle-incognito`) sans écriture dans l'historique local.
   - **Mode Écran Scindé / Split View (`Ctrl+\`)** : affiche simultanément deux onglets côte à côte.

2. **Omnibox Intelligente & Raccourcis Bangs**
   - Résolution intelligente d'URL, `localhost`, IP ou recherche web.
   - 5 moteurs intégrés commutables en un clic : **DuckDuckGo**, **Brave Search**, **Google**, **Qwant**, **Startpage**.
   - Support des **Bangs** rapides directement dans la barre d'adresse : `!yt`, `!gh`, `!w`, `!g`, `!ddg`, `!mdn`, `!npm`.

3. **Orakle Shield & Audit de Sécurité OWASP**
   - **Bloqueur réseau natif** (`webRequest.onBeforeRequest`) filtrant les régies publicitaires, traqueurs analytiques et scripts de télémétrie.
   - **Purge automatique des paramètres de pistage** (`utm_source`, `utm_medium`, `fbclid`, `gclid`, `msclkid`, `_ga`, etc.).
   - **Spoofing de User-Agent** à la volée (Orakle Linux, Chrome Windows 11, Firefox Linux, Safari macOS, iPhone 16 Pro iOS 18) + injection `DNT: 1` et `Sec-GPC: 1`.
   - **Auditeur OWASP en direct** : analyse les en-têtes HTTP/TLS (`CSP`, `HSTS`, `X-Frame-Options`, `X-Content-Type-Options`, `Referrer-Policy`, `Permissions-Policy`) et attribue un score de `A+` à `F`.

4. **Interface "Orakle Glass" & Personnalisation**
   - Thèmes **Sombre Abyssal** et **Clair Cristal** + 5 couleurs d'accentuation dynamiques (`Bleu Orakle`, `Violet Cyber`, `Émeraude`, `Ambre`, `Rose Néon`).
   - Disposition commutable entre **Barre Horizontale Classique** et **Barre Latérale Verticale (Style Arc)**.
   - **Palette de commandes (`Ctrl+K`)**, **Recherche dans la page (`Ctrl+F`)**, **Mode Zen (`F11`)**, **Favoris**, **Historique**, **Gestionnaire de téléchargements** et **Scratchpad persistant**.

---

## Commandes de Développement et Lancement

```bash
# Mode développement avec rechargement à chaud (Vite + Electron)
npm run dev

# Vérification du typage TypeScript (Renderer + Main Electron)
npm run typecheck

# Compilation de production
npm run build

# Démarrer l'application compilée
npm start
```
