use std::ffi::{c_char, c_void, CStr};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use orakle_core::shield::ShieldEngine;

struct ExtState {
    rules_path: Option<PathBuf>,
    last_modified: Option<SystemTime>,
    shield: ShieldEngine,
}

static EXT_STATE: OnceLock<Mutex<ExtState>> = OnceLock::new();

fn state() -> &'static Mutex<ExtState> {
    EXT_STATE.get_or_init(|| {
        Mutex::new(ExtState {
            rules_path: None,
            last_modified: None,
            shield: ShieldEngine::new_with_rules(""),
        })
    })
}

fn reload_if_changed(st: &mut ExtState) {
    let Some(path) = st.rules_path.as_ref() else {
        return;
    };
    let Ok(meta) = fs::metadata(path) else {
        return;
    };
    let modified = meta.modified().ok();
    if modified.is_some() && modified == st.last_modified {
        return;
    }
    if let Ok(content) = fs::read_to_string(path) {
        if content.starts_with("! DISABLED") {
            st.shield.set_enabled(false);
        } else {
            st.shield = ShieldEngine::new_with_rules(&content);
            st.shield.set_enabled(true);
        }
        st.last_modified = modified;
    }
}

/// # Safety
/// `rules_path` doit être nul ou pointer vers une chaîne C terminée par `\0`.
#[no_mangle]
pub unsafe extern "C" fn orakle_ext_init(rules_path: *const c_char) {
    let mut guard = state().lock().expect("ext state lock");
    if !rules_path.is_null() {
        if let Ok(s) = unsafe { CStr::from_ptr(rules_path) }.to_str() {
            if !s.is_empty() {
                guard.rules_path = Some(PathBuf::from(s));
                guard.last_modified = None;
                reload_if_changed(&mut guard);
            }
        }
    }
}

/// # Safety
/// Les pointeurs non nuls doivent référencer des chaînes C valides terminées par `\0`.
#[no_mangle]
pub unsafe extern "C" fn orakle_ext_should_block(
    uri: *const c_char,
    page_uri: *const c_char,
    method: *const c_char,
) -> bool {
    if uri.is_null() {
        return false;
    }
    let Ok(url_str) = unsafe { CStr::from_ptr(uri) }.to_str() else {
        return false;
    };
    if !(url_str.starts_with("http://") || url_str.starts_with("https://")) {
        return false;
    }
    let source_str = if page_uri.is_null() {
        url_str
    } else {
        unsafe { CStr::from_ptr(page_uri) }
            .to_str()
            .unwrap_or(url_str)
    };
    let method_str = if method.is_null() {
        "GET"
    } else {
        unsafe { CStr::from_ptr(method) }.to_str().unwrap_or("GET")
    };

    let mut guard = state().lock().expect("ext state lock");
    reload_if_changed(&mut guard);

    if let Ok(decision) = guard
        .shield
        .check_network_request(url_str, source_str, "other", method_str)
    {
        return decision.blocked;
    }
    false
}

extern "C" {
    fn orakle_shim_initialize(extension: *mut c_void, user_data: *const c_void);
}

/// # Safety
/// Appelé par WebKitWebProcess à l'initialisation de l'extension avec ses pointeurs natifs GObject/GVariant.
#[no_mangle]
pub unsafe extern "C" fn webkit_web_extension_initialize_with_user_data(
    extension: *mut c_void,
    user_data: *const c_void,
) {
    unsafe {
        orakle_shim_initialize(extension, user_data);
    }
}

/// # Safety
/// Appelé par WebKitWebProcess lorsque l'initialisation sans user_data est utilisée.
#[no_mangle]
pub unsafe extern "C" fn webkit_web_extension_initialize(extension: *mut c_void) {
    unsafe {
        orakle_shim_initialize(extension, std::ptr::null());
    }
}
