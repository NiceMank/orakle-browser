# Signature cryptographique & Pipeline de mises à jour — Orakle Browser V2

## 1. Intégrité et signatures des artefacts

### Linux (`.deb` et AppImage)
- Chaque artefact publié est accompagné d'une empreinte **SHA-256** (`.sha256`).
- Les paquets de release officiels doivent être signés par clé **GPG Ed25519 dédiée** (dépôt APT `Release.gpg` + signature détachée `.asc` pour AppImage / tarballs).
- Exemple de vérification attendue côté release CI :
  ```bash
  sha256sum -c orakle-browser_0.2.0_amd64.deb.sha256
  gpg --detach-sign --armor -o orakle-browser_0.2.0_amd64.deb.asc orakle-browser_0.2.0_amd64.deb
  ```
- **État actuel de cette machine** : la construction `.deb` et son empreinte SHA-256 sont exécutées localement ; la signature GPG CI/CD n'est pas émise sur le poste local (aucune clé privée de production stockée dans le dépôt).

### Windows (`MSIX / WiX MSI` — **NON COMPILÉ NI TESTÉ SUR WINDOWS**)
- Signature **Authenticode** requise via certificat EV / Azure Trusted Signing (`signtool sign /fd SHA256 /tr ...`).
- WebView2 Evergreen Runtime Bootstrapper vérifié ou embarqué selon le profil réseau cible.

### macOS (`.app / .dmg` — **NON COMPILÉ NI TESTÉ SUR MACOS**)
- Signature Apple **Developer ID Application** + option `--options runtime` + **notarisation Apple (`xcrun notarytool submit`)**.

## 2. Manifeste de mise à jour (Update Channel)

Format de manifeste JSON vérifiable (voir `update-manifest.example.json`) :
- `version`: version SemVer cible.
- `channel`: `stable` | `beta`.
- `min_os`: configuration minimale (`linux-webkit2gtk-4.1`, `win10-webview2`, `macos-13-wkwebview`).
- `artifacts.<target>.url`, `sha256`, `size_bytes`, `signature_ed25519`.
- Le client de mise à jour (à intégrer en phase distribution CI) doit :
  1. Télécharger le manifeste en HTTPS strict (TLS 1.2+, vérification certificat système).
  2. Vérifier la signature **Ed25519** du manifeste avec la clé publique embarquée dans le binaire.
  3. Télécharger l'archive binaire via le proxy ou client sécurisé, vérifier le SHA-256 **avant** toute installation.
