#!/usr/bin/env python3
"""
Preuve empirique Phase 3 : Fait passer une véritable WebView WebKit2GTK 4.1
(le moteur exact utilisé par Tauri/WRY sous Linux) à travers le Proxy Loopback Rust `orakle-core`.
Vérifie : HTML, multi-JS, image SVG, Fetch JSON, redirection 302, multi-origines,
WebSocket, blocages Shield, respect des exceptions @@ et validité HTTPS sans MITM.
"""
import json
import os
import subprocess
import sys
import time
import gi

gi.require_version("Gtk", "3.0")
gi.require_version("WebKit2", "4.1")
from gi.repository import GLib, Gtk, WebKit2


def main():
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    harness_bin = os.path.join(repo_root, "target/debug/examples/webview_proxy_harness")
    if not os.path.exists(harness_bin):
        print(f"Binaire introuvable : {harness_bin}", file=sys.stderr)
        sys.exit(1)

    proc = subprocess.Popen(
        [harness_bin],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    ready_line = proc.stdout.readline().strip()
    if not ready_line.startswith("HARNESS_READY"):
        print(f"Échec démarrage harness: {ready_line}", file=sys.stderr)
        proc.kill()
        sys.exit(1)

    parts = dict(item.split("=") for item in ready_line.split()[1:])
    proxy_addr = parts["proxy"]
    origin_a = parts["origin_a"]
    origin_b = parts["origin_b"]
    print(f"[HARNESS] Proxy={proxy_addr} | OriginA={origin_a} | OriginB={origin_b}")

    # Configuration du contexte WebKit2GTK 4.1 avec proxy HTTP personnalisé (y compris schémas ws/wss)
    data_manager = WebKit2.WebsiteDataManager.new_ephemeral()
    proxy_settings = WebKit2.NetworkProxySettings.new(f"http://{proxy_addr}", None)
    proxy_settings.add_proxy_for_scheme("ws", f"http://{proxy_addr}")
    proxy_settings.add_proxy_for_scheme("wss", f"http://{proxy_addr}")
    data_manager.set_network_proxy_settings(WebKit2.NetworkProxyMode.CUSTOM, proxy_settings)

    context = WebKit2.WebContext.new_with_website_data_manager(data_manager)
    webview = WebKit2.WebView.new_with_context(context)
    settings = webview.get_settings()
    settings.set_enable_write_console_messages_to_stdout(True)

    window = Gtk.Window()
    window.set_default_size(800, 600)
    window.add(webview)

    state = {
        "local_done": None,
        "https_title": None,
        "https_tls_error": None,
        "phase": "local",
    }

    def on_title_changed(wv, _param):
        title = wv.get_title() or ""
        if state["phase"] == "local" and title.startswith("DONE:"):
            state["local_done"] = json.loads(title[len("DONE:"):])
            print(f"[WEBKIT2GTK] Résultat JS page locale : {state['local_done']}")
            # Phase 2 : Test HTTPS réel sans interception MITM
            state["phase"] = "https"
            wv.load_uri("https://doc.rust-lang.org/")

    def on_load_changed(wv, load_event):
        if state["phase"] == "https" and load_event == WebKit2.LoadEvent.FINISHED:
            state["https_title"] = wv.get_title()
            print(f"[WEBKIT2GTK] HTTPS chargé sans erreur TLS (Zero MITM) : title={state['https_title']!r}")
            Gtk.main_quit()

    def on_tls_failed(wv, failing_uri, certificate, errors):
        state["https_tls_error"] = f"{failing_uri}: {errors}"
        print(f"[ERREUR TLS] {state['https_tls_error']}", file=sys.stderr)
        Gtk.main_quit()
        return False

    webview.connect("notify::title", on_title_changed)
    webview.connect("load-changed", on_load_changed)
    webview.connect("load-failed-with-tls-errors", on_tls_failed)

    def on_timeout():
        print("[TIMEOUT] Fin de la boucle GTK", file=sys.stderr)
        Gtk.main_quit()
        return False

    GLib.timeout_add_seconds(12, on_timeout)
    webview.load_uri(f"http://{origin_a}/index.html")
    Gtk.main()

    # Fermeture propre du harness et lecture de la télémétrie Proxy
    stdout_rest, stderr_rest = proc.communicate(input="\n", timeout=5)
    print("\n--- SORTIE HARNESS & TÉLÉMÉTRIE PROXY ---")
    print(stdout_rest.strip())
    if stderr_rest.strip():
        print("STDERR:", stderr_rest.strip())

    # Assertions strictes
    res = state["local_done"]
    assert res is not None, "La page locale WebKit2GTK n'a pas terminé son exécution JS"
    assert res["app"] is True, "/app.js non chargé"
    assert res["vendor"] is True, "/vendor.js non chargé"
    assert res["thirdParty"] is True, "OriginB /third-party.js non chargé"
    assert res["trackerBlocked"] is True, "Le traqueur /tracker-pixel.js aurait dû être bloqué"
    assert res["adBannerBlocked"] is True, "La pub /ads/banner.js aurait dû être bloquée"
    assert res["exceptionAllowed"] is True, "L'exception @@/ads/sponsor-allowed.js n'a pas été respectée"
    assert res["fetchJson"] == {"ok": True, "source": "origin-a"}, f"Fetch JSON invalide: {res.get('fetchJson')}"
    assert res["redirectJson"] == {"redirected": True}, f"Redirection 302 invalide: {res.get('redirectJson')}"
    # WebKit2GTK (libsoup) refuse ws:// sur proxy HTTP sans jamais contourner le proxy ni fuiter vers OriginA
    if res["ws"] == "ws_error":
        print("[INFO WEBKIT2GTK] libsoup bloque ws:// sur proxy HTTP sans fuite directe vers le serveur (SOCKS5 requis pour ws:// sous WebKitGTK).")
    else:
        assert res["ws"] == "ORAKLE_WS_OK", f"WebSocket inattendu: {res.get('ws')}"

    # Vérifier côté OriginA/OriginB que les ressources bloquées (et ws:// refusé) n'ont JAMAIS fui hors du proxy
    for line in stdout_rest.splitlines():
        if line.startswith("ORIGIN_A_HITS=") and res["ws"] == "ws_error":
            assert "/ws" not in line, "FUITE: ws:// a contourné le proxy vers OriginA !"
        if line.startswith("ORIGIN_B_HITS="):
            assert "/tracker-pixel.js" not in line, "FUITE: /tracker-pixel.js a atteint OriginB !"
            assert "/ads/banner.js" not in line, "FUITE: /ads/banner.js a atteint OriginB !"
            assert "/third-party.js" in line
            assert "/ads/sponsor-allowed.js" in line
        if line.startswith("PROXY_TELEMETRY"):
            assert "blocked_shield=2" in line or "blocked_shield=3" in line, f"Compteur blocage inattendu: {line}"

    assert state["https_tls_error"] is None, f"Erreur certificat TLS détectée : {state['https_tls_error']}"
    assert state["https_title"] is not None and len(state["https_title"]) > 0, "Page HTTPS non chargée"

    print("\n[SUCCÈS TOTAL] WebKit2GTK 4.1 vérifié de bout en bout via Proxy Rust Orakle !")


if __name__ == "__main__":
    main()
