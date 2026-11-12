# Orakle Browser — Product Requirements Document (PRD)

- **Version du document** : 2.0.0
- **Statut** : Document de référence produit
- **Dernière mise à jour** : 2026-11-11

---

## 1. Vision et Mission du Produit

Orakle Browser est un navigateur Web indépendant de nouvelle génération, conçu pour offrir aux utilisateurs :
- Une **navigation fluide, réactive et légère**, respectueuse des ressources matérielles.
- Une **protection rigoureuse de la vie privée** sans jargon obscur, avec des mécanismes réels de blocage des traqueurs et de nettoyage des paramètres de surveillance.
- Des **fonctionnalités à haute valeur d'usage** : véritable écran scindé (*Split View*), espaces de travail étanches (*Spaces*), mode lecture intelligent avec résumé local, gestionnaire de sessions de travail.
- Une **esthétique professionnelle sobre et raffinée**, conçue pour un confort de travail prolongé (8 heures et plus par jour), loin des artifices visuels fatiguants.

### Ce qu'Orakle N'EST PAS :
- Une simple copie superficielle de Chrome, Edge, Brave ou Arc.
- Un tableau de bord de hacking cyberpunk rempli de graphiques décoratifs et de chiffres fictifs.
- Une promesse trompeuse d'anonymat absolu ou de certification magique.
- Une coquille vide avec des interrupteurs d'options inactifs.

---

## 2. Utilisateurs Cibles et Cas d'Usage

| Profil Utilisateur | Besoins Principaux | Fonctionnalités Clés Orakle |
|---|---|---|
| **Professionnels du Web & Développeurs** | Consulter de la documentation en parallèle du code, auditer les en-têtes HTTP réels, tester des APIs et des flux réseau sans pollution de cookies. | • Split View 50/50 redimensionnable.<br>• Outils Dev & Audit passif des en-têtes de sécurité.<br>• Isolation des sessions de test. |
| **Chercheurs, Rédacteurs & Étudiants** | Lire des articles longs sans distraction publicitaire, synthétiser des contenus, organiser des sources par projet. | • Mode Lecture épuré avec résumé local.<br>• Espaces thématiques (*Spaces*).<br>• Snapshot de session de recherche. |
| **Utilisateurs Quotidien Soucieux de leur Confidentialité** | Naviguer rapidement sans être suivi de site en site par les régies publicitaires, refuser la télémétrie invasive. | • Orakle Shield natif (adblock-rs).<br>• Purge automatique des balises UTM/FBCLID.<br>• Mode Fantôme en mémoire vive. |

---

## 3. Périmètre Fonctionnel Détaillé

### A. Navigation Fondamentale (Indispensable)
1. **Omnibox Intelligente** :
   - Saisie unique : reconnaissance automatique d'URL valide, d'IP/localhost, ou de requête de recherche.
   - Sélection du moteur par défaut : DuckDuckGo, Brave Search, Qwant, Startpage, Google, Ecosia, ou moteur personnalisé via URL template (`https://example.com/search?q=%s`).
   - Support des raccourcis "Bangs" (`!gh`, `!w`, `!yt`, `!so`, `!mdn`).
2. **Contrôles de Navigation** :
   - Précédent, Suivant, Actualiser, Arrêter le chargement, Accueil configurable.
   - Indicateur de chargement discret (barre linéaire supérieure).
   - Gestion de l'état de sécurité SSL/TLS : affichage du cadenas, nom de domaine en gras, certificat vérifié.
3. **Multi-Onglets Robuste** :
   - Création (`Ctrl+T`), fermeture (`Ctrl+W`), réouverture du dernier onglet fermé (`Ctrl+Shift+T`).
   - Épinglage d'onglet (`Pin`), duplication, mise en sourdine audio (`Mute`).
   - Réorganisation fluide des onglets par glisser-déposer.
   - Raccourcis directs `Ctrl+1` à `Ctrl+9`.

### B. Écran Scindé (Split View Réel)
- Affichage simultané de deux pages Web indépendantes dans la même fenêtre.
- Barre de séparation ajustable à la souris (30/70, 50/50, 70/30).
- Chaque volet dispose de son historique de navigation propre, de son titre et de son bouton de fermeture.
- Raccourci clavier de bascule rapide (`Ctrl+\`).

### C. Espaces de Travail (Spaces)
- Regroupement des onglets par projet ou contexte (*Personnel*, *Travail*, *Veille*, *Projets*).
- Chaque espace possède son nom, son pictogramme discret et sa couleur d'identification sobre.
- Bascule instantanée sans rechargement brutal des pages.
- Compteur d'onglets par espace.

### D. Confidentialité & Sécurité
- **Orakle Shield** :
  - Blocage réseau des domaines de publicité, traqueurs et télémétrie intrusive.
  - Purge automatique des paramètres d'URL de pistage (`utm_source`, `utm_medium`, `fbclid`, `gclid`, `msclkid`, etc.).
  - En-têtes de protection de la vie privée émis automatiquement : `DNT: 1`, `Sec-GPC: 1`.
  - Liste blanche par domaine activable en un clic.
- **Mode Fantôme (Navigation Privée)** :
  - Partition de données distincte, stockée exclusivement en mémoire vive (`RAM`).
  - Aucun historique écrit sur disque, purge intégrale des cookies et du cache à la fermeture.
- **Gestionnaire de Données de Navigation** :
  - Consultation et suppression granulaire de l'historique.
  - Nettoyage sélectif des cookies par domaine ou effacement global.

### E. Productivité & Confort
- **Mode Lecture (Orakle Reader)** :
  - Extraction de l'article principal selon l'algorithme Readability sans scripts tiers ni bannières.
  - Choix typographique (Serif / Sans-Serif), taille de police ajustable, 3 thèmes de lecture (*Papier*, *Sépia*, *Nuit*).
  - Résumé local du texte extrait.
- **Scratchpad Persistant** :
  - Bloc-notes personnel Markdown intégré, sauvegardé localement en temps réel.
- **Recherche dans la Page** :
  - Barre flottante discrète (`Ctrl+F`) avec mise en surbrillance et décompte des occurrences.

---

## 4. Exigences Non-Fonctionnelles

1. **Performance** :
   - Temps de démarrage à froid de la fenêtre : < 400 ms.
   - Consommation mémoire au repos avec 1 onglet actif : < 80 Mo de RAM.
   - Taux de rafraîchissement de l'interface : 60 fps stable, zéro saccade lors du redimensionnement ou du défilement.
2. **Accessibilité (a11y)** :
   - Conformité WCAG 2.2 niveau AA pour tous les composants internes de l'interface.
   - Contrastes minimum de 4.5:1 pour le texte normal, 3:1 pour le texte large.
   - Navigation intégrale possible au clavier (indicateurs de focus visibles de 2px).
   - Prise en charge du réglage système de réduction des animations (`prefers-reduced-motion`).
3. **Internationalisation (i18n)** :
   - Support natif complet du français et de l'anglais dès la version 1.0.
   - Fichiers de localisation structurés en JSON/Fluent pour faciliter les traductions futures.
