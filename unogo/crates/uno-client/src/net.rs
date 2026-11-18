use crate::browser;
use serde_json::Value;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
#[derive(Debug)]
pub struct Error {
    pub status: u16,
    pub code: String,
    pub message: String,
}
fn network() -> Error {
    Error {
        status: 0,
        code: "NETWORK".into(),
        message: "Connection lost. Reconnecting…".into(),
    }
}
struct PendingRequest {
    timer: i32,
    controller: web_sys::AbortController,
    _callback: Closure<dyn FnMut()>,
}
impl Drop for PendingRequest {
    fn drop(&mut self) {
        browser::window().clear_timeout_with_handle(self.timer);
        self.controller.abort();
    }
}
pub async fn request(method: &str, path: &str, body: Option<Value>) -> Result<Value, Error> {
    let token = browser::saved("unogo.token").unwrap_or_default();
    let controller = web_sys::AbortController::new().map_err(|_| network())?;
    let abort = controller.clone();
    let timeout = Closure::<dyn FnMut()>::new(move || abort.abort());
    let timer = browser::window()
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            timeout.as_ref().unchecked_ref(),
            8_000,
        )
        .map_err(|_| network())?;
    let _pending = PendingRequest {
        timer,
        controller: controller.clone(),
        _callback: timeout,
    };
    let result = async {
        let options = web_sys::RequestInit::new();
        options.set_method(method);
        options.set_signal(Some(&controller.signal()));
        if let Some(body) = body {
            options.set_body(&JsValue::from_str(&body.to_string()));
        }
        let req = web_sys::Request::new_with_str_and_init(path, &options).map_err(|_| network())?;
        req.headers()
            .set("Content-Type", "application/json")
            .map_err(|_| network())?;
        if !token.is_empty() {
            req.headers()
                .set("Authorization", &format!("Bearer {token}"))
                .map_err(|_| network())?;
        }
        let response = JsFuture::from(browser::window().fetch_with_request(&req))
            .await
            .map_err(|_| network())?
            .dyn_into::<web_sys::Response>()
            .map_err(|_| network())?;
        let status = response.status();
        let text = JsFuture::from(response.text().map_err(|_| network())?)
            .await
            .map_err(|_| network())?
            .as_string()
            .unwrap_or_default();
        let value: Value = serde_json::from_str(&text).map_err(|_| Error {
            status,
            code: "RESPONSE".into(),
            message: "The server returned an unexpected response. Try again.".into(),
        })?;
        if !response.ok() {
            return Err(Error {
                status,
                code: value["code"].as_str().unwrap_or("").into(),
                message: value["error"].as_str().unwrap_or("Try again.").into(),
            });
        }
        Ok(value)
    }
    .await;
    result
}
