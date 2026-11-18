use wasm_bindgen::{JsCast, JsValue};
pub fn window() -> web_sys::Window {
    web_sys::window().expect("browser")
}
pub fn document() -> web_sys::Document {
    window().document().expect("document")
}
pub fn focus(selector: &str) {
    if let Ok(Some(el)) = document().query_selector(selector) {
        if let Ok(el) = el.dyn_into::<web_sys::HtmlElement>() {
            let _ = el.focus();
        }
    }
}
pub fn trap_focus(e: &dioxus::prelude::KeyboardEvent) {
    use dioxus::prelude::{Key, Modifiers, ModifiersInteraction};
    if e.key() != Key::Tab {
        return;
    }
    let Ok(Some(dialog)) = document().query_selector(".modal-backdrop:last-of-type [role=dialog]")
    else {
        return;
    };
    let Ok(nodes) =
        dialog.query_selector_all("button:not(:disabled),input:not(:disabled),select,summary")
    else {
        return;
    };
    let focusable: Vec<_> = (0..nodes.length())
        .filter_map(|i| nodes.item(i)?.dyn_into::<web_sys::HtmlElement>().ok())
        .filter(|e| e.offset_width() > 0 && e.offset_height() > 0)
        .collect();
    let Some(first) = focusable.first() else {
        e.prevent_default();
        return;
    };
    let last = focusable.last().unwrap();
    let active = document().active_element();
    let backwards = e.modifiers().contains(Modifiers::SHIFT);
    if active.as_ref().is_none_or(|el| !dialog.contains(Some(el)))
        || (!backwards
            && active
                .as_ref()
                .is_some_and(|el| el.is_same_node(Some(last))))
    {
        e.prevent_default();
        let _ = first.focus();
    } else if backwards
        && active
            .as_ref()
            .is_some_and(|el| el.is_same_node(Some(first)))
    {
        e.prevent_default();
        let _ = last.focus();
    }
}
pub fn saved(key: &str) -> Option<String> {
    window()
        .local_storage()
        .ok()
        .flatten()?
        .get_item(key)
        .ok()
        .flatten()
}
pub fn save(key: &str, value: &str) {
    if let Ok(Some(s)) = window().local_storage() {
        let _ = s.set_item(key, value);
    }
}
pub fn remove(key: &str) {
    if let Ok(Some(s)) = window().local_storage() {
        let _ = s.remove_item(key);
    }
}
pub fn now() -> u64 {
    js_sys::Date::now().max(0.) as u64
}
pub async fn sleep(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}
pub async fn copy(text: &str) -> bool {
    wasm_bindgen_futures::JsFuture::from(window().navigator().clipboard().write_text(text))
        .await
        .is_ok()
}
pub fn select(id: &str) {
    if let Some(input) = document()
        .get_element_by_id(id)
        .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
    {
        input.select();
    }
}
pub fn invite_url(code: &str) -> String {
    format!(
        "{}/?join={code}",
        window().location().origin().unwrap_or_default()
    )
}
pub fn clean_query() {
    if let Ok(history) = window().history() {
        let _ = history.replace_state_with_url(&JsValue::NULL, "", Some("/"));
    }
}
thread_local! {
    static AUDIO: std::cell::RefCell<Option<web_sys::AudioContext>> = const {
        std::cell::RefCell::new(None)
    };
}
pub fn unlock_sound() {
    AUDIO.with_borrow_mut(|a| {
        if a.is_none() {
            *a = web_sys::AudioContext::new().ok();
        }
        if let Some(a) = a {
            let _ = a.resume();
        }
    });
}
pub fn card_sound() {
    if saved("unogo.sound").as_deref() == Some("off") {
        return;
    }
    let volume = saved("unogo.volume")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.3)
        .clamp(0., 1.);
    AUDIO.with_borrow(|a| {
        if let Some(a) = a {
            let Ok(o) = a.create_oscillator() else {
                return;
            };
            let Ok(g) = a.create_gain() else {
                return;
            };
            o.frequency().set_value(460.);
            g.gain().set_value(volume * 0.12);
            let _ = g
                .gain()
                .exponential_ramp_to_value_at_time(0.001, a.current_time() + 0.09);
            let _ = o.connect_with_audio_node(&g);
            let _ = g.connect_with_audio_node(&a.destination());
            let _ = o.start();
            let _ = o.stop_with_when(a.current_time() + 0.1);
        }
    });
}
