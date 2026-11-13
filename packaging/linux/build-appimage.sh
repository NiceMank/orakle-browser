#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION="${ORAKLE_VERSION:-0.2.0}"
DIST_DIR="${ROOT_DIR}/packaging/dist"
APPDIR="${DIST_DIR}/Orakle-Browser.AppDir"

export PKG_CONFIG_PATH="${PKG_CONFIG_PATH:-/home/niceman/.local/webkit-sysroot/usr/lib/x86_64-linux-gnu/pkgconfig:/home/niceman/.local/webkit-sysroot/usr/share/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig}"

cd "${ROOT_DIR}"
cargo build --release -p orakle-webkit-ext -p orakle-app

rm -rf "${APPDIR}"
mkdir -p "${APPDIR}/usr/bin" \
         "${APPDIR}/usr/lib/orakle-browser/extensions" \
         "${APPDIR}/usr/share/applications" \
         "${APPDIR}/usr/share/icons/hicolor/scalable/apps"

install -m 0755 "${ROOT_DIR}/target/release/orakle-app" "${APPDIR}/usr/bin/orakle-app"
install -m 0644 "${ROOT_DIR}/target/release/liborakle_webkit_ext.so" "${APPDIR}/usr/lib/orakle-browser/extensions/liborakle_webkit_ext.so"
install -m 0644 "${ROOT_DIR}/packaging/linux/orakle-browser.desktop" "${APPDIR}/orakle-browser.desktop"
install -m 0644 "${ROOT_DIR}/packaging/linux/orakle-browser.desktop" "${APPDIR}/usr/share/applications/orakle-browser.desktop"
install -m 0644 "${ROOT_DIR}/packaging/orakle-browser.svg" "${APPDIR}/orakle-browser.svg"
install -m 0644 "${ROOT_DIR}/packaging/orakle-browser.svg" "${APPDIR}/usr/share/icons/hicolor/scalable/apps/orakle-browser.svg"

cat > "${APPDIR}/AppRun" <<'APPRUN'
#!/usr/bin/env bash
HERE="$(dirname "$(readlink -f "$0")")"
export ORAKLE_WEBKIT_EXTENSION_DIR="${HERE}/usr/lib/orakle-browser/extensions"
exec "${HERE}/usr/bin/orakle-app" "$@"
APPRUN
chmod 0755 "${APPDIR}/AppRun"

if ! command -v appimagetool >/dev/null 2>&1; then
    echo "AppDir préparé dans ${APPDIR}, mais 'appimagetool' n'est pas installé sur cette machine."
    echo "Installer appimagetool puis exécuter : ARCH=x86_64 appimagetool ${APPDIR} ${DIST_DIR}/Orakle-Browser-${VERSION}-x86_64.AppImage"
    exit 0
fi

APPIMAGE_OUT="${DIST_DIR}/Orakle-Browser-${VERSION}-x86_64.AppImage"
ARCH=x86_64 appimagetool "${APPDIR}" "${APPIMAGE_OUT}"
sha256sum "${APPIMAGE_OUT}" > "${APPIMAGE_OUT}.sha256"
