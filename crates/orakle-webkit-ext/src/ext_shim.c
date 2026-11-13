#include <webkit2/webkit-web-extension.h>
#include <gio/gio.h>
#include <stdbool.h>

extern void orakle_ext_init(const char *rules_path);
extern bool orakle_ext_should_block(const char *uri, const char *page_uri, const char *method);

static gboolean on_send_request(
    WebKitWebPage *web_page,
    WebKitURIRequest *request,
    WebKitURIResponse *redirected_response,
    gpointer user_data
) {
    (void)redirected_response;
    (void)user_data;

    const char *uri = webkit_uri_request_get_uri(request);
    const char *page_uri = webkit_web_page_get_uri(web_page);
    const char *method = webkit_uri_request_get_http_method(request);

    if (orakle_ext_should_block(uri, page_uri, method)) {
        WebKitUserMessage *msg = webkit_user_message_new(
            "orakle-shield-blocked",
            g_variant_new_string(uri ? uri : "")
        );
        webkit_web_page_send_message_to_view(web_page, msg, NULL, NULL, NULL);
        return TRUE;
    }

    return FALSE;
}

static void on_page_created(
    WebKitWebExtension *extension,
    WebKitWebPage *web_page,
    gpointer user_data
) {
    (void)extension;
    (void)user_data;
    g_signal_connect(web_page, "send-request", G_CALLBACK(on_send_request), NULL);
}

void orakle_shim_initialize(
    WebKitWebExtension *extension,
    const GVariant *user_data
) {
    const gchar *rules_path = NULL;
    if (user_data && g_variant_is_of_type((GVariant *)user_data, G_VARIANT_TYPE_STRING)) {
        rules_path = g_variant_get_string((GVariant *)user_data, NULL);
    }
    if (!rules_path || rules_path[0] == '\0') {
        rules_path = g_getenv("ORAKLE_SHIELD_RULES_FILE");
    }
    orakle_ext_init(rules_path);
    g_signal_connect(extension, "page-created", G_CALLBACK(on_page_created), NULL);
}
