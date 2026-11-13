# Orakle Browser — Modèle de Menace et Architecture de Sécurité

- **Version** : 2.0.0
- **Statut** : Document de référence sécurité (aligné Phase 4)
- **Dernière mise à jour** : 2026-11-13

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
| **Contenu Web Malveillant (Drive-by Download, Exploit)** | Exécution de code natif non autorisé sur la machine hôte. | • Isolation totale de la WebView par bac à sable système (`with_ipc_handler` absent des WebViews distantes).<br>• Validation stricte d'URL (`validate_navigation_url` : rejet `javascript:`, `file:`, `data:`, `localhost`, RFC1918/metadata). |
| **Pistage Publicitaire & Empreinte (Fingerprinting)** | Surveillance des habitudes de navigation et profilage publicitaire par des tiers. | • Orakle Shield + `SecureLoopbackProxy` (`HTTP`/`CONNECT`/`SOCKS5` pré-DNS, Zero-MITM) + extension native Linux `liborakle_webkit_ext.so` (`send-request` Niveau 2, sans réécriture de corps). |
| **SSRF, DNS Rebinding & Fuites Réseau Proxy** | Accès au réseau local (`127.0.0.0/8`, `169.254.0.0/16`, `100.64.0.0/10`, `::ffff:...`, RFC1918, ULA) ou contournement silencieux lors d'erreurs proxy. | • Blocage strict des IP privées/loopback/métadonnées (`is_forbidden_private_ip`) et des ports non HTTP/HTTPS (`80`/`443` par défaut).<br>• Résolution DNS validée sur toutes les adresses IP avant ouverture de socket (anti-DNS-rebinding).<br>• Politique **fail-closed** (`403` / `502`, zéro connexion directe de repli). |
| **Fuite de Cookies et Sessions Cross-Site** | Vol de session utilisateur, attaques CSRF ou écoute réseau. | • Cloisonnement via `with_incognito(true)` dans `orakle-app` (store non persistant en RAM, validé sur le Split View droit).<br>• Zéro interception MITM TLS sur `CONNECT` (la vérification de certificats reste assurée nativement par la WebView et `rustls`/`webpki-roots`). |
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
