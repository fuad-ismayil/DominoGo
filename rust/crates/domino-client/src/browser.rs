//! Browser access, clipboard and optional vibration support.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// `console.log` — kept for socket protocol debug logging.
pub fn log(msg: &str) {
    web_sys::console::log_1(&JsValue::from_str(msg));
}

pub fn window() -> web_sys::Window {
    web_sys::window().expect("browser window")
}

pub fn document() -> web_sys::Document {
    window().document().expect("document")
}

/// Read an options override from `window.__DOMINO_API_URL__` if present.
pub fn override_api_url() -> Option<String> {
    let v = js_sys::Reflect::get(&window(), &JsValue::from_str("__DOMINO_API_URL__"))
        .ok()?
        .as_string()?;
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

/// `navigator.clipboard.writeText(text)` — best effort (insecure contexts may
/// not expose this API).
pub fn clipboard_write_text(text: &str) {
    let nav = window().navigator();
    let Ok(clip) = js_sys::Reflect::get(&nav, &JsValue::from_str("clipboard")) else {
        return;
    };
    if let Ok(f) = js_sys::Reflect::get(&clip, &JsValue::from_str("writeText")) {
        if let Ok(f) = f.dyn_into::<js_sys::Function>() {
            let _ = f.call1(&clip, &JsValue::from_str(text));
        }
    }
}

/// `navigator.vibrate(pattern)` — pattern is a number or an array.
pub fn vibrate(pattern: &JsValue) {
    let nav = window().navigator();
    if let Ok(f) = js_sys::Reflect::get(&nav, &JsValue::from_str("vibrate")) {
        if let Ok(f) = f.dyn_into::<js_sys::Function>() {
            let _ = f.call1(&nav, pattern);
        }
    }
}

/// Milliseconds since Unix epoch, matching server deadlines.
pub fn now_ms() -> f64 {
    js_sys::Date::now()
}

/// Consume the invitation once while preserving unrelated URL parameters.
pub fn take_invite() -> Option<Result<String, crate::invite::InviteProblem>> {
    let url = web_sys::Url::new(&window().location().href().ok()?).ok()?;
    let params = url.search_params();
    let raw = params.get("join")?;
    let unique = params.get_all("join").length() == 1;
    params.delete("join");
    if let Ok(history) = window().history() {
        let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&url.href()));
    }
    Some(
        crate::invite::normalize_table_code(&raw)
            .filter(|_| unique)
            .ok_or(crate::invite::InviteProblem::Invalid),
    )
}

pub fn invite_link(code: &str) -> String {
    let location = window().location();
    format!(
        "{}{}?join={}",
        location.origin().unwrap_or_default(),
        location.pathname().unwrap_or_else(|_| "/".into()),
        code
    )
}
