#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
export PKG_CONFIG_PATH="${PKG_CONFIG_PATH:-/home/niceman/.local/webkit-sysroot/usr/lib/x86_64-linux-gnu/pkgconfig:/home/niceman/.local/webkit-sysroot/usr/share/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig}"

LOCAL_BIN="${HOME}/.local/bin"
LOCAL_EXT_DIR="${HOME}/.local/lib/orakle-browser/extensions"
LOCAL_APPS="${HOME}/.local/share/applications"
LOCAL_ICONS="${HOME}/.local/share/icons/hicolor/scalable/apps"

echo "[1/4] Compilation Release d'orakle-webkit-ext et orakle-app..."
cd "${ROOT_DIR}"
cargo build --release -p orakle-webkit-ext -p orakle-app

APP_BIN="${ROOT_DIR}/target/release/orakle-app"
EXT_SO="${ROOT_DIR}/target/release/liborakle_webkit_ext.so"

if [[ ! -x "${APP_BIN}" || ! -f "${EXT_SO}" ]]; then
    echo "Erreur : binaires Release introuvables (${APP_BIN} ou ${EXT_SO})" >&2
    exit 1
fi

echo "[2/4] Installation utilisateur dans ${HOME}/.local ..."
mkdir -p "${LOCAL_BIN}" "${LOCAL_EXT_DIR}" "${LOCAL_APPS}" "${LOCAL_ICONS}"

install -m 0755 "${APP_BIN}" "${LOCAL_BIN}/orakle-app"
install -m 0644 "${EXT_SO}" "${LOCAL_EXT_DIR}/liborakle_webkit_ext.so"
install -m 0644 "${ROOT_DIR}/packaging/orakle-browser.svg" "${LOCAL_ICONS}/orakle-browser.svg"

cat > "${LOCAL_BIN}/orakle-browser" <<WRAP
#!/usr/bin/env bash
export ORAKLE_WEBKIT_EXTENSION_DIR="\${ORAKLE_WEBKIT_EXTENSION_DIR:-${LOCAL_EXT_DIR}}"
export WEBKIT_DISABLE_DMABUF_RENDERER="\${WEBKIT_DISABLE_DMABUF_RENDERER:-1}"
exec "${LOCAL_BIN}/orakle-app" "\$@"
WRAP
chmod 0755 "${LOCAL_BIN}/orakle-browser"

echo "[3/4] Installation du lanceur bureau..."
sed "s|^Exec=orakle-browser %u\$|Exec=${LOCAL_BIN}/orakle-browser %u|" \
    "${ROOT_DIR}/packaging/linux/orakle-browser.desktop" > "${LOCAL_APPS}/orakle-browser.desktop"
chmod 0644 "${LOCAL_APPS}/orakle-browser.desktop"

echo "[4/4] Mise à jour des caches bureau et icônes..."
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${LOCAL_APPS}" || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "${HOME}/.local/share/icons/hicolor" || true
fi

echo "Orakle Browser V2 installé :"
echo "  - Lanceur  : ${LOCAL_BIN}/orakle-browser"
echo "  - Binaire  : ${LOCAL_BIN}/orakle-app"
echo "  - Ext. N2  : ${LOCAL_EXT_DIR}/liborakle_webkit_ext.so"
echo "  - Desktop  : ${LOCAL_APPS}/orakle-browser.desktop"
echo "  - Icône    : ${LOCAL_ICONS}/orakle-browser.svg"
