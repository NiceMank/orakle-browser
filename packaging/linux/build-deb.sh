#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION="${ORAKLE_VERSION:-0.2.0}"
ARCH="amd64"
PKG_NAME="orakle-browser_${VERSION}_${ARCH}"
DIST_DIR="${ROOT_DIR}/packaging/dist"
STAGE_DIR="${DIST_DIR}/${PKG_NAME}"

export PKG_CONFIG_PATH="${PKG_CONFIG_PATH:-/home/niceman/.local/webkit-sysroot/usr/lib/x86_64-linux-gnu/pkgconfig:/home/niceman/.local/webkit-sysroot/usr/share/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig}"

echo "[1/4] Compilation Release d'orakle-app et liborakle_webkit_ext.so..."
cd "${ROOT_DIR}"
cargo build --release -p orakle-webkit-ext -p orakle-app

APP_BIN="${ROOT_DIR}/target/release/orakle-app"
EXT_SO="${ROOT_DIR}/target/release/liborakle_webkit_ext.so"

if [[ ! -x "${APP_BIN}" || ! -f "${EXT_SO}" ]]; then
    echo "Erreur : artefacts Release introuvables (${APP_BIN} ou ${EXT_SO})" >&2
    exit 1
fi

echo "[2/4] Préparation arborescence Debian dans ${STAGE_DIR}..."
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/DEBIAN" \
         "${STAGE_DIR}/usr/bin" \
         "${STAGE_DIR}/usr/lib/orakle-browser/extensions" \
         "${STAGE_DIR}/usr/share/applications" \
         "${STAGE_DIR}/usr/share/icons/hicolor/scalable/apps" \
         "${STAGE_DIR}/usr/share/doc/orakle-browser"

install -m 0755 "${APP_BIN}" "${STAGE_DIR}/usr/bin/orakle-app"
cat > "${STAGE_DIR}/usr/bin/orakle-browser" <<'WRAP'
#!/usr/bin/env bash
export ORAKLE_WEBKIT_EXTENSION_DIR="${ORAKLE_WEBKIT_EXTENSION_DIR:-/usr/lib/orakle-browser/extensions}"
exec /usr/bin/orakle-app "$@"
WRAP
chmod 0755 "${STAGE_DIR}/usr/bin/orakle-browser"

install -m 0644 "${EXT_SO}" "${STAGE_DIR}/usr/lib/orakle-browser/extensions/liborakle_webkit_ext.so"
install -m 0644 "${ROOT_DIR}/packaging/linux/orakle-browser.desktop" "${STAGE_DIR}/usr/share/applications/orakle-browser.desktop"
install -m 0644 "${ROOT_DIR}/packaging/orakle-browser.svg" "${STAGE_DIR}/usr/share/icons/hicolor/scalable/apps/orakle-browser.svg"
cat > "${STAGE_DIR}/usr/share/doc/orakle-browser/README.Debian" <<'DOC'
Orakle Browser V2 (Native Rust + WebKit2GTK)
- Binaire principal : /usr/bin/orakle-browser -> /usr/bin/orakle-app
- Extension WebKit filtrage Niveau 2 : /usr/lib/orakle-browser/extensions/liborakle_webkit_ext.so
DOC

INSTALLED_KB="$(du -sk "${STAGE_DIR}/usr" | awk '{print $1}')"

cat > "${STAGE_DIR}/DEBIAN/control" <<CTRL
Package: orakle-browser
Version: ${VERSION}
Section: web
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${INSTALLED_KB}
Maintainer: Orakle Engineering <engineering@orakle.local>
Depends: libc6, libgtk-3-0, libwebkit2gtk-4.1-0
Description: Orakle Browser V2 native Rust web browser
 Navigateur multi-WebView basé sur Rust, wry, WebKit2GTK, adblock-rust
 et proxy loopback strict fail-closed.
CTRL

echo "[3/4] Construction du paquet .deb avec dpkg-deb..."
DEB_OUT="${DIST_DIR}/${PKG_NAME}.deb"
dpkg-deb --build --root-owner-group "${STAGE_DIR}" "${DEB_OUT}"

echo "[4/4] Vérification et empreinte SHA-256..."
dpkg-deb --info "${DEB_OUT}"
sha256sum "${DEB_OUT}" > "${DEB_OUT}.sha256"
cat "${DEB_OUT}.sha256"
