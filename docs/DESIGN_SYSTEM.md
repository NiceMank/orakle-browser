# Orakle Browser — Système de Design Visuel & Ergonomie

- **Version** : 2.0.0
- **Statut** : Charte et spécifications de design
- **Dernière mise à jour** : 2026-11-11

---

## 1. Philosophie et Principes Visuels

L'interface d'Orakle Browser est conçue pour des sessions d'utilisation intensive (8 heures et plus par jour). Elle obéit à quatre règles cardinales :

1. **Sobriété et Élégance** :
   Bannissement formel des dégradés néon criards, des halos lumineux envahissants, des bordures éclatantes et du vocabulaire de hacking. L'interface s'efface pour laisser le contenu Web au premier plan.
2. **Lisibilité et Contraste Strict (WCAG 2.2 AA)** :
   Chaque texte, icône et indicateur d'état respecte un ratio de contraste supérieur à 4.5:1 en taille normale et 3:1 en grande taille.
3. **Hiérarchie et Espaces Respirants** :
   Des espacements réguliers et calibrés sur un pas de 4px / 8px, des rayons de courbure discrets (6px à 12px) et des ombres légères et diffuses.
4. **Cohérence Fonctionnelle Totale** :
   Chaque élément interactif dispose d'états visuels clairs (défaut, survol, focus accessible au clavier, clic, désactivé). Aucun bouton n'est purement décoratif.

---

## 2. Palettes de Couleurs Professionnelles

Le navigateur propose 3 palettes neutres coordonnées avec des accents uniques et maîtrisés :

### Palette A : « Ardoise & Bleu Pétrole » (Défaut Recommandé)
- **Fond Principal (Sombre)** : `#0d1117` (Graphite profond)
- **Surfaces Élevées (Sombre)** : `#161b22` (Ardoise neutre)
- **Bordures Subtiles** : `rgba(255, 255, 255, 0.08)`
- **Texte Principal** : `#f0f6fc`
- **Texte Secondaire** : `#8b949e`
- **Couleur d'Accent Unique** : `#0969da` (Bleu pétrole équilibré, non saturé)
- **Fond Principal (Clair)** : `#f6f8fa` (Blanc cassé doux)
- **Surfaces Élevées (Clair)** : `#ffffff` (Blanc pur)
- **Bordures Subtiles (Clair)** : `rgba(0, 0, 0, 0.09)`
- **Texte Principal (Clair)** : `#1f2328`
- **Texte Secondaire (Clair)** : `#656d76`

### Palette B : « Ivoire & Vert Sauge » (Confort de Lecture Zen)
- **Surfaces Claires** : `#f7f6f2` (Ivoire doux) / `#ffffff`
- **Surfaces Sombres** : `#141716` (Anthracite chaud) / `#1c211f`
- **Couleur d'Accent Unique** : `#2e7d5b` (Vert sauge apaisant, non agressif)

### Palette C : « Titane & Graphite » (Monochrome Professionnel)
- **Surfaces Claires** : `#f4f4f6` / `#ffffff`
- **Surfaces Sombres** : `#121316` / `#1a1b1f`
- **Couleur d'Accent Unique** : `#4a5568` (Gris acier neutre)

---

## 3. Typographie Calibrée

- **Famille Principale UI** : `Plus Jakarta Sans`, `-apple-system`, `BlinkMacSystemFont`, `Inter`, `Segoe UI`, sans-serif.
- **Famille Monospace (URLs, Code, Métriques)** : `JetBrains Mono`, `ui-monospace`, `monospace`.
- **Échelle des Tailles** :
  - **Titre H1 / Horloge** : `48px` à `56px` (poids 700, tabular-nums).
  - **Titres de section H2** : `18px` (poids 600).
  - **Corps de texte / Omnibox** : `13.5px` ou `14px` (poids 400 et 500, interligne 1.5).
  - **Sous-titres & Onglets** : `12px` (poids 500 et 600).
  - **Badges & Métadonnées** : `10px` ou `11px` (poids 600, espacement interlettres +0.04em).

---

## 4. Échelle d'Espacement et Rayons de Courbure

| Jeton | Valeur | Usage |
|---|---|---|
| `--space-1` | `4px` | Écartement micro-éléments, padding interne d'icônes |
| `--space-2` | `8px` | Espacement entre icônes et étiquettes, gap compact |
| `--space-3` | `12px` | Padding interne des onglets et boutons |
| `--space-4` | `16px` | Marges standard des panneaux latéraux |
| `--space-6` | `24px` | Espacement principal des sections |
| `--radius-s` | `6px` | Badges, sélecteurs, champs de saisie compacts |
| `--radius-m` | `10px` | Onglets, boutons d'action, cartes légères |
| `--radius-l` | `14px` | Omnibox, fenêtres modales, volets de Split View |

---

## 5. Composants Spécifiques du Navigateur

### A. L'Omnibox
- Fond gris neutre doux avec bordure fine de 1px.
- Pas d'anneau néon : un anneau de focus discret de 2px de couleur d'accent apparaît uniquement à la sélection.
- Affichage du cadenas SSL en vert sobre `#10b981` ou gris neutre, avec nom de domaine accentué en gras pour déjouer le typosquattage.

### B. Les Onglets
- Deux modes commutables :
  1. **Horizontal classique** : disposition en haut de fenêtre, onglet actif avec liseré supérieur ou fond légèrement surélevé.
  2. **Vertical (Sidebar)** : disposition latérale ordonnée avec icône du site, titre tronqué avec élégance et bouton de fermeture discret au survol.
- Indicateur audio : icône de haut-parleur sobre pour couper le son sans fermer l'onglet.

### C. La Barre de Séparation du Split View
- Ligne fine de 2px à 4px entre les deux vues Web, facilitant le redimensionnement par glisser-déposer sans empiéter sur la surface d'affichage.
- Poignée centrale discrète.

### D. Densité d'Interface Configurable
- **Compacte** : hauteurs de barres réduites (`40px`), idéale pour les petits écrans d'ordinateurs portables (13 pouces).
- **Normale** : hauteur standard équilibrée (`52px`).
- **Aérée** : hauteurs confortables (`60px`), idéale pour les écrans 4K ou les utilisateurs privilégiant les grands espaces.
