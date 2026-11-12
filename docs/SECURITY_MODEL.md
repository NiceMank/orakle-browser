# Orakle Browser — Modèle de Menace et Architecture de Sécurité

- **Version** : 2.0.0
- **Statut** : Document de référence sécurité
- **Dernière mise à jour** : 2026-11-11

---

## 1. Principes Fondamentaux de Sécurité

1. **Défense en Profondeur & Cloisonnement Strict** :
   Le contenu Web distant provenant d'Internet est considéré par définition comme **non fiable** (*untrusted*). Aucune page Web ne dispose d'un accès aux primitives du système d'exploitation, au système de fichiers local ou aux commandes Rust de l'hôte.
2. **Principe du Moindre Privilège (PoLP)** :
   Chaque module ne détient que les permissions rigoureusement indispensables à son fonctionnement. Les APIs matérielles (micro, webcam, géolocalisation) sont soumises à un consentement utilisateur explicite, révocable à tout moment.
3. **Vérité Technique et Absence de Faux Sentiments de Sécurité** :
   - Orakle n'invente **aucune fausse certification de sécurité**.
   - Le mode Navigation Privée (*Mode Fantôme*) est documenté honnêtement : il empêche l'écriture de traces sur la machine locale, mais **ne garantit pas l'anonymat sur Internet** vis-à-vis du fournisseur d'accès ou des serveurs visités.

---

## 2. Matrice du Modèle de Menace (Threat Model)

| Vecteur de Menace | Risque Potentiel | Mesure de Protection Orakle |
|---|---|---|
| **Contenu Web Malveillant (Drive-by Download, Exploit)** | Exécution de code natif non autorisé sur la machine hôte. | • Isolation totale de la WebView par bac à sable système (Sandbox natif Linux/Windows).<br>• Aucune commande IPC exposée sans passer par un schéma de validation typé et validé par Rust.<br>• `nodeIntegration: false`, `contextIsolation: true`. |
| **Pistage Publicitaire & Empreinte (Fingerprinting)** | Surveillance des habitudes de navigation et profilage publicitaire par des tiers. | • Orakle Shield : blocage réseau en amont des régies et traqueurs via `adblock-rs`.<br>• Suppression automatique des paramètres d'URL de surveillance (`utm_*`, `fbclid`, `gclid`).<br>• En-têtes `DNT: 1` et `Sec-GPC: 1` émis systématiquement. |
| **Fuite de Cookies et Sessions Cross-Site** | Vol de session utilisateur, attaques CSRF ou écoute réseau. | • Cloisonnement des contextes de données (*WebContext*) : les onglets Fantômes disposent de leur propre magasin de cookies en mémoire vive.<br>• Forçage du protocole HTTPS avec alerte explicite en cas de certificat TLS non vérifié. |
| **Téléchargements Dangereux** | Téléchargement accidentel d'exécutables piégés. | • Invite de confirmation systématique avant enregistrement sur disque.<br>• Affichage clair de l'extension et de la taille du fichier. |
| **Abus d'Autorisations Matérielles** | Accès clandestin à la webcam, au micro ou aux notifications. | • Gestionnaire centralisé de permissions par domaine : aucune autorisation accordée par défaut sans invite modale explicite. |

---

## 3. Gestion Sécurisée des Données Personnelles et Secrets

1. **Stockage Local au Repos** :
   L'historique, les favoris et les sessions sont stockés dans une base locale SQLite gérée par le module Rust `storage`.
2. **Mots de Passe & Clés d'API** :
   Le stockage des identifiants et des secrets de session s'appuie obligatoirement sur les coffres-forts natifs du système d'exploitation :
   - **Linux** : `Secret Service API` / `Freedesktop Secret` (via `libsecret`).
   - **Windows** : `DPAPI` / `Credential Locker`.
   - **macOS** : `Apple Keychain`.
   Aucun mot de passe ou jeton sensible n'est enregistré en texte clair dans un fichier de configuration JSON.

---

## 4. Politique de Divulgation des Vulnérabilités & Mises à Jour

- **Mises à Jour de Sécurité** : Les dépendances critiques (WebKitGTK, crates Rust réseau et cryptographiques) font l'objet d'une veille permanente.
- **Canal de Signalement Sécurisé** : Mise à disposition d'une adresse de sécurité dédiée (`security@orakle-browser.org` ou procédure `SECURITY.md` sur GitHub) avec chiffrement PGP pour tout signalement de vulnérabilité par les chercheurs en sécurité.
