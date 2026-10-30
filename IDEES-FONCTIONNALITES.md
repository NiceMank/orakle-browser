# Orakle Browser — Idées de fonctionnalités différenciantes

> Objectif : ne pas être « un Chrome de plus ». Un navigateur ne gagne pas sur la vitesse
> brute (Chromium la donne à tout le monde) mais sur **ce qu'il fait des pages**, sur
> **la mémoire du contexte** et sur **la confiance**.
>
> Ci-dessous : 42 pistes classées par axe, avec pour chacune le *pitch*, la raison pour
> laquelle c'est différenciant, et une estimation d'effort (🟢 facile · 🟡 moyen · 🔴 lourd).
> Les items marqués **✅ DÉJÀ FAIT** sont implémentés dans la branche `feat/aurora-ui-v1.1`.

---

## Axe 1 — Le navigateur qui comprend ce que vous lisez (assistant local)

C'est l'axe le plus défendable : Chrome a Gemini, Edge a Copilot, Arc a Max — mais tous
envoient vos pages dans le cloud. Orakle peut faire l'inverse : **traiter localement**.

| # | Fonctionnalité | Pitch | Pourquoi c'est différent | Effort |
|---|---|---|---|---|
| 1 | **Mode Lecture + Résumé** ✅ DÉJÀ FAIT | Ctrl+Shift+R extrait l'article et le résume (IA gratuite sans clé, ou 100 % local). | Aucun navigateur mainstream n'a de lecteur avec résumé intégré par défaut. | 🟢 |
| 2 | **Détecteur de contradiction / de mise à jour** | Sur un article, Orakle cherche dans votre historique les pages traitant du même sujet et signale les chiffres divergents. | Personne ne relie vos lectures entre elles. | 🟡 |
| 3 | **Extraction de tableaux → CSV** | Clic droit sur n'importe quel tableau HTML : « Extraire en CSV / presse-papier ». | Les navigateurs n'ont aucun outil d'extraction de données. | 🟢 |
| 4 | **Sélection → questions** | Sélectionnez un paragraphe, une bulle propose : résumer, traduire, expliquer, **vérifier les faits cités**. | Contextuel, ancré dans la page, pas un chatbot latéral. | 🟡 |
| 5 | **Recherche sémantique dans l'historique** | « la page avec le graphique de budget de mars » → retrouvée par contenu, pas par mots-clés. | L'historique natif est une liste plate. | 🔴 |
| 6 | **Veille de page ("Surveiller cette page")** | Orakle re-vérifie une page à intervalle donné et notifie les différences en surlignage. | Les extensions type Distill existent, jamais intégré natif. | 🟡 |

## Axe 2 — Mémoire du contexte de travail (là où Arc a gagné, mais mal)

| # | Fonctionnalité | Pitch | Pourquoi c'est différent | Effort |
|---|---|---|---|---|
| 7 | **Espaces de travail (Spaces)** ✅ DÉJÀ FAIT | Rail latéral : chaque espace = un jeu d'onglets isolé, avec compteur. | Arc l'a, Chrome non ; mais Orakle l'associe à la sécurité. | 🟢 |
| 8 | **Sessions snapshot** ✅ DÉJÀ FAIT | Capturez les N onglets ouverts en « session » nommée, restaurez plus tard. | Le « tab groups » de Chrome ne se sauvegarde pas réellement. | 🟢 |
| 9 | **Sessions = dossier consultable** | Une session n'est pas une liste d'onglets mais un mini-site : notes, citations, captures, checklist. | Transforme les onglets en livrable. | 🟡 |
| 10 | **Reprise intelligente au démarrage** | Au lancement, Orakle propose « contexte d'hier à 14h » / « ce que vous faisiez avant l'arrêt ». | Le « restaurer les onglets » est binaire et brute. | 🟡 |
| 11 | **Timeline visuelle de navigation** | Frise chronologique jour par jour, avec capture d'écran miniature des pages. | Aucun navigateur n'assume d'historique visuel. | 🟡 |
| 12 | **Détection de doublons d'onglets** | « 3 onglets pointent vers la même page, dont 2 avec des ancres différentes ». | Personne ne nettoie ça automatiquement. | 🟢 |
| 13 | **Mise en veille sélective (RAM)** ✅ DÉJÀ FAIT | Clic droit sur un onglet : « Mettre en veille », le processus de rendu est libéré. | Chrome le fait automatiquement et silencieusement ; Orakle le rend **explicite et réversible**. | 🟢 |

## Axe 3 — Sécurité visible, pas un jargon dans un menu

C'est l'ADN d'Orakle (Shield + audit OWASP). À pousser encore plus loin.

| # | Fonctionnalité | Pitch | Pourquoi c'est différent | Effort |
|---|---|---|---|---|
| 14 | **Orakle Shield** ✅ DÉJÀ FAIT | Blocage réseau, purge des `utm_*`/`fbclid`, DNT+GPC, spoofing UA. | Comparable à Brave, mais open source et auditable. | — |
| 15 | **Audit OWASP en direct** ✅ DÉJÀ FAIT | Note A+ → F sur les en-têtes HTTP de la page courante. | Aucun navigateur mainstream ne fait ça (c'est un outil de pentester). | — |
| 16 | **Inspecteur de cookies & traces** ✅ DÉJÀ FAIT | Liste les cookies du domaine, classe les traqueurs, purge par domaine. | Les navigateurs le cachent dans des menus de développement. | — |
| 17 | **Orakle Pulse** ✅ DÉJÀ FAIT | Mesure DNS / TCP / TLS / TTFB d'une page en un clic. | Les DevTools le font, mais personne ne l'expose en langage humain. | — |
| 18 | **Empreinte de navigateur (fingerprint)** | Score de traçabilité : canvas, WebGL, polices, timezone, UA, audio — avec mode « brouillage cohérent ». | Les anti-fingerprinting existent mais ne **mesurent** jamais l'exposition. | 🟡 |
| 19 | **Rapport de sécurité exportable** | Bouton « Générer un rapport PDF » : en-têtes, cookies, traqueurs, ressources tierces. | Transforme le navigateur en outil d'audit vendable à des clients. | 🟡 |
| 20 | **Badge de certificat enrichi** | Qui a émis le certificat, depuis quand, transparence des logs, réputation du domaine. | Chrome a retiré ces infos en 2020 — oculus de niche. | 🟢 |
| 21 | **Alerte de formulaire exfiltrant** | Si un `<form>` poste une donnée sensible vers un tiers, Orakle prévient avant l'envoi. | Aucun navigateur ne protège ce moment précis. | 🟡 |
| 22 | **Bac à sable de lien (Link Sandbox)** | Clic droit → « Ouvrir en sandbox » : la page s'ouvre sans cookies, sans JS, en proxy de lecture. | Risque zéro pour les liens douteux. | 🟡 |
| 23 | **Mode Arnaque (anti-scam)** | Détection de patterns (faux support, urgence, crypto), bandeau de mise en garde local. | Les navigateurs bloquent les URL connues, pas les techniques. | 🟡 |

## Axe 4 — Concentration & bien-être numérique

| # | Fonctionnalité | Pitch | Pourquoi c'est différent | Effort |
|---|---|---|---|---|
| 24 | **Mode Focus (Pomodoro + blocklist)** ✅ DÉJÀ FAIT | Minuteur 15/25/45/60 min, blocage des domaines distractants pendant la session. | Les navigateurs ont des « modes » cosmétiques, pas un vrai garde-fou. | — |
| 25 | **Budget de temps par espace** | « 30 min/jour sur l'espace Veille » → avertissement puis mur doux. | Chrome/Edge n'ont que du contrôle parental externe. | 🟡 |
| 26 | **Indicateur de coût carbone/CPU** | Grammes de CO₂ et ms de CPU par page visitée, cumulés dans la barre d'état. | Personne ne mesure le coût énergétique de la navigation. | 🟢 |
| 27 | **Mode Calme (déjà partiel)** ✅ DÉJÀ FAIT | Réduction des animations, du grain, des transparences. | Accessibilité intégrée, pas un thème de plus. | — |
| 28 | **Lecture en file ("Read Later")** | Empilez des articles, Orakle les rend en mode lecture sans pub, hors-ligne. | Pocket est mort, Instapaper est externe — intégration native. | 🟡 |

## Axe 5 — Interface et interaction (au-delà des onglets)

| # | Fonctionnalité | Pitch | Pourquoi c'est différent | Effort |
|---|---|---|---|---|
| 29 | **Palette de commandes universelle** ✅ DÉJÀ FAIT | Ctrl+K : onglets, outils, réglages, recherche floue. | Chrome n'en a pas, Vivaldi/Arc partiellement. | — |
| 30 | **Omnibox à suggestions riches** ✅ DÉJÀ FAIT | Bangs `!gh`, `!cve`, favoris, onglets ouverts, historique, actions. | Les bangs DDG ne sont pas natifs ailleurs. | — |
| 31 | **Nappes Aurora + verre adaptatif** ✅ DÉJÀ FAIT | Halo animé, grain filmique, 4 densités de verre, thème Abyssal/Cristal, 7 accents. | Identité visuelle forte, réglable pour la perf. | — |
| 32 | **Écran scindé** ✅ DÉJÀ FAIT | Ctrl+\ affiche deux onglets côte à côte. | Edge l'a, Chrome non ; Orakle le couple aux espaces. | — |
| 33 | **Disposition latérale (Arc-like)** ✅ DÉJÀ FAIT | Bascule instantanée barre horizontale ↔ rail vertical. | Vivaldi le fait en profondeur, Orakle en un toggle. | — |
| 34 | **Onglets « chapitres »** | Un onglet peut contenir plusieurs URLs liées (une recherche → ses 5 résultats), navigables en mini-rail. | Chrome a enterré les « tab groups » ; Arc essaie. | 🔴 |
| 35 | **Panneau de composition (snippets)** | Zone de saisie Markdown/HTML avec aperçu live, export PDF/PNG, glisser vers un site. | Aucun navigateur n'a d'éditeur sérieux intégré. | 🟡 |
| 36 | **Annotation de page** | Surligner et annoter n'importe quelle page, notes persistantes liées à l'URL (+ sélecteur CSS). | Hypothésis a disparu du marché mainstream. | 🟡 |
| 37 | **Split par espace de travail** | Un espace par écran si multi-moniteurs, avec répartition mémorisée. | Chrome a les profils, pas les fenêtres-par-espace. | 🟡 |

## Axe 6 — Automatisation & extensibilité (le vrai fossé)

| # | Fonctionnalité | Pitch | Pourquoi c'est différent | Effort |
|---|---|---|---|---|
| 38 | **Recettes (macros de navigation)** | « Ouvre les 6 sites d'audit pour ce domaine, lance 1 audit, exporte » en une commande réutilisable. | Les extensions ne composent pas entre elles ; ici c'est natif. | 🟡 |
| 39 | **Mini-scripts par site (user-scripts natifs)** | Un éditeur intégré de scripts JS par domaine, avec versionnage, activés à la volée. | Tampermonkey est une extension fragile ; ici, c'est le navigateur. | 🟡 |
| 40 | **API locale (localhost) pour automatiser Orakle** | Un endpoint HTTP local (`http://127.0.0.1:7188`) pour piloter onglets, favoris, sessions depuis un script. | Aucun navigateur grand public n'offre d'API locale documentée. | 🔴 |
| 41 | **Règles de filtrage personnalisées (Shield)** | Écrire ses propres règles `||ads.example^` + testeur de règle en direct. | uBlock Origin le fait, mais pas le navigateur lui-même. | 🟡 |
| 42 | **Rappels contextuels par domaine** | « Sur github.com, rappelle-moi de vérifier la branche » — affiché discrètement à l'ouverture. | Personne ne lie une intention au domaine. | 🟢 |

---

## Ma sélection : les 5 à faire ensuite (ordre recommandé)

1. **#4 Sélection → questions + #3 Extraction CSV** — deux fonctionnalités « waouh » peu coûteuses
   qui exploitent le Mode Lecture déjà en place.
2. **#18 Empreinte de navigateur + #19 Rapport de sécurité PDF** — c'est là que Orakle devient
   un outil professionnel, pas juste un joli navigateur. Positionnement unique.
3. **#11 Timeline visuelle d'historique** — fort impact émotionnel et démo, différenciateur immédiat.
4. **#6 Veille de page** — usage quotidien réel (prix, CVEs, offres d'emploi, docs).
5. **#40 API locale** — transforme Orakle en plateforme d'automatisation (scripts, CI, scraping encadré).

## Les 3 « signature features » qui rendraient Orakle unique

- **Le navigateur qui note le web** : à chaque visite, une note de sécurité (A+→F), de
  performance (Pulse) et de qualité de lecture — puis une note globale privée, historique,
  exportable. Personne ne fait ça.
- **Le contexte comme objet de première classe** : un « Contexte » = espaces + onglets + notes +
  sessions + règles Focus, sauvegardable, partageable (fichier `.orakle`), rechargeable.
  Le navigateur devient un gestionnaire de projets déguisé.
- **Zero-cloud par défaut** : tout ce qui est « intelligent » (résumé, extraction, classement,
  détection de traqueurs) fonctionne sans réseau. C'est l'argument que Brave et Arc ne peuvent
  pas tenir.

---

## Ce qui a été livré dans cette itération (v1.1 « Aurora »)

- ✅ Design system Aurora refondu (verre multi-couches, grain, halos, 7 accents, densité réglable).
- ✅ Omnibox capsule + panneau de suggestions (bangs `!gh !cve !so …`, favoris, onglets, historique, actions).
- ✅ Chrome refondu : onglets en capsules, menu contextuel d'onglet (épingler / split / veille / muet),
  menu « Plus d'outils », barre d'état avec signes vitaux (RAM/CPU/blocages).
- ✅ Espaces de travail (Spaces) avec rail, compteurs et édition en ligne.
- ✅ Sessions snapshot (capture / restauration / suppression).
- ✅ Mode Lecture Orakle Reader (extraction, 3 thèmes, taille de police, résumé IA gratuit sans clé
  + repli 100 % local).
- ✅ Orakle Pulse (DNS/TCP/TLS/TTFB, note de vitesse).
- ✅ Inspecteur de cookies & traces avec purge par domaine.
- ✅ Mode Focus (Pomodoro + blocklist appliquée à la navigation).
- ✅ Onglets en veille, palette de commandes enrichie, recherche dans la page avec compteur,
  raccourcis Ctrl+1…9 / Ctrl+Tab.
- ✅ 2 nouveaux moteurs de recherche (Perplexity) + 6 nouveaux bangs.
