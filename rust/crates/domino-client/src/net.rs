//! REST requests and Engine.IO v4 / Socket.IO v5 transport over browser WebSocket.

use crate::browser;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

fn in_browser_scope<I, O>(f: impl Fn(I) -> O + 'static) -> impl Fn(I) -> O {
    let runtime = dioxus::dioxus_core::Runtime::current();
    move |input| runtime.in_scope(dioxus::prelude::ScopeId::ROOT, || f(input))
}

const TOKEN_KEY: &str = "domino_token";
const DEBUG_NET: bool = false;

pub type AckCallback = Box<dyn Fn(Value)>;
pub type EventHandler = Box<dyn Fn(&Value)>;

pub fn is_connected() -> bool {
    NET.with_borrow(|net| net.as_ref().is_some_and(|n| n.ws_open && n.ns_connected))
}

struct NetClient {
    ws: Option<web_sys::WebSocket>,
    ws_open: bool,
    ns_connected: bool,
    token: Option<String>,
    socket_sid: Option<String>,
    next_ack_id: u64,
    pending_acks: HashMap<u64, (AckCallback, f64)>,
    outbox: Vec<String>,
    keepalive: bool,
    reconnect_delay_ms: u32,
    last_activity_ms: f64,
    ping_interval_ms: f64,
    ping_timeout_ms: f64,
}

impl Default for NetClient {
    fn default() -> Self {
        Self {
            ws: None,
            ws_open: false,
            ns_connected: false,
            token: None,
            socket_sid: None,
            next_ack_id: 0,
            pending_acks: HashMap::new(),
            outbox: Vec::new(),
            keepalive: false,
            reconnect_delay_ms: 1000,
            last_activity_ms: 0.0,
            ping_interval_ms: 25_000.0,
            ping_timeout_ms: 20_000.0,
        }
    }
}

thread_local! {
    static NET: RefCell<Option<NetClient>> = const { RefCell::new(None) };
    /// Handlers live independently of the connection so `on()` can be called
    /// before/after (re)connects.
    static HANDLERS: RefCell<HashMap<&'static str, Vec<EventHandler>>> = RefCell::new(HashMap::new());
    /// Out-of-band handoff so ack/event callbacks never run inside the
    /// `NET` RefCell borrow (they are free to `emit`).
    static READY_EVENTS: RefCell<Vec<(String, Value)>> = const { RefCell::new(Vec::new()) };
    static DISPATCHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static READY_ACKS: RefCell<Vec<(AckCallback, Value)>> = const { RefCell::new(Vec::new()) };
    /// wasm-bindgen closures for the *current* WebSocket; cleared on socket
    /// replacement (after the old socket's listeners were detached).
    static WS_CLOSURES: RefCell<Vec<ClosureHolder>> = const { RefCell::new(Vec::new()) };
}

type SocketClosures = (
    Closure<dyn FnMut(web_sys::MessageEvent)>,
    Closure<dyn FnMut(web_sys::Event)>,
    Closure<dyn FnMut(web_sys::Event)>,
    Closure<dyn FnMut(web_sys::CloseEvent)>,
);
struct ClosureHolder {
    _closures: SocketClosures,
}

fn log_net(msg: &str) {
    if DEBUG_NET {
        browser::log(&format!("[net] {}", msg));
    }
}

fn json_to_js(value: &Value) -> JsValue {
    JsValue::from_str(&value.to_string())
}

// ===========================================================================
// Public API
// ===========================================================================

/// Register an inbound event handler. the application uses exactly one handler per
/// event; multiple handlers per event are supported.
pub fn on(event: &'static str, handler: EventHandler) {
    let runtime = dioxus::dioxus_core::Runtime::current();
    let handler: EventHandler =
        Box::new(move |value| runtime.in_scope(dioxus::prelude::ScopeId::ROOT, || handler(value)));
    HANDLERS.with_borrow_mut(|m| m.entry(event).or_default().push(handler));
}

/// Persist the session token in local browser storage.
pub fn set_token(token: &str) {
    if let Ok(Some(storage)) = browser::window().local_storage() {
        let _ = storage.set_item(TOKEN_KEY, token);
    }
}

pub fn get_token() -> Option<String> {
    let storage = browser::window().local_storage().ok().flatten()?;
    storage.get_item(TOKEN_KEY).ok().flatten()
}

pub fn clear_token() {
    if let Ok(Some(storage)) = browser::window().local_storage() {
        let _ = storage.remove_item(TOKEN_KEY);
    }
}

/// REST base (`window.location.origin`, or the `window.__DOMINO_API_URL__`
/// dev override when present).
pub fn rest_base() -> String {
    if let Some(v) = browser::override_api_url() {
        return v.trim_end_matches('/').to_string();
    }
    browser::window()
        .location()
        .origin()
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_string()
}

/// websocket URL for socket.io: `ws(s)://<origin>/socket.io/?EIO=4&transport=websocket`.
pub fn ws_url() -> String {
    if let Some(v) = browser::override_api_url() {
        let mapped = v.replace("http://", "ws://").replace("https://", "wss://");
        return format!(
            "{}/socket.io/?EIO=4&transport=websocket",
            mapped.trim_end_matches('/')
        );
    }
    let origin = browser::window().location().origin().unwrap_or_default();
    let (scheme, host) = if let Some(host) = origin.strip_prefix("https://") {
        ("wss", host)
    } else {
        ("ws", origin.strip_prefix("http://").unwrap_or(&origin))
    };
    format!(
        "{}://{}/socket.io/?EIO=4&transport=websocket",
        scheme,
        host.trim_end_matches('/')
    )
}

/// Connect/open the socket (no-op when already running). Reads
/// the stored session token and sends it during namespace authentication.
pub fn ensure_connected() {
    let token = get_token();
    NET.with_borrow_mut(|opt| match opt.as_mut() {
        Some(n) if n.keepalive => {}
        _ => {
            let mut n = NetClient {
                token,
                keepalive: true,
                ..Default::default()
            };
            n.connect_socket();
            *opt = Some(n);
        }
    });
}

/// Disconnect and discard credentials held by the realtime connection.
pub fn disconnect() {
    NET.with_borrow_mut(|opt| {
        if let Some(n) = opt.as_mut() {
            n.keepalive = false;
            n.reset_socket();
        }
        *opt = None;
        WS_CLOSURES.with_borrow_mut(|c| c.clear());
    });
}

/// Emit `event` with a single payload (`args = [event, payload]`), optionally
/// registering an ack callback.
pub fn emit(event: &str, payload: Value, ack: Option<AckCallback>) {
    let ack = ack.map(|cb| Box::new(in_browser_scope(cb)) as AckCallback);
    NET.with_borrow_mut(|opt| {
        if let Some(n) = opt.as_mut() {
            n.emit_inner(event, payload, ack);
        } else {
            if let Some(cb) = ack {
                READY_ACKS.with_borrow_mut(|q| {
                    q.push((
                        cb,
                        json!({"error":"Reconnecting. Try again when the table is connected."}),
                    ))
                });
            }
        }
    });
    dispatch_queued();
}

/// Watchdog tick (called from the global ticker): if no frame arrived within
/// `pingInterval + pingTimeout`, force a reconnect like the socket.io client
/// does.
pub fn keepalive_tick(now_ms: f64) {
    NET.with_borrow_mut(|opt| {
        if let Some(n) = opt.as_mut() {
            let expired: Vec<_> = n.pending_acks.iter().filter(|(_, (_, deadline))| now_ms >= *deadline).map(|(id, _)| *id).collect();
            for id in expired { if let Some((cb, _)) = n.pending_acks.remove(&id) { READY_ACKS.with_borrow_mut(|q| q.push((cb, json!({"error":"The server did not confirm that action. Rejoin to refresh the table before trying again."})))); } }
            if n.keepalive && n.ws_open {
                let limit = n.ping_interval_ms + n.ping_timeout_ms;
                if n.last_activity_ms > 0.0 && now_ms - n.last_activity_ms > limit {
                    log_net("no frames for too long — forcing reconnect");
                    n.reset_socket();
                    n.schedule_reconnect();
                }
            }
        }
    });
    dispatch_queued();
}

/// Set the token on the running client (e.g. after a login re-issue).
pub fn set_client_token(token: &str) {
    NET.with_borrow_mut(|opt| {
        if let Some(n) = opt.as_mut() {
            n.token = Some(token.to_string());
        }
    });
}

// ===========================================================================
// REST requests
// ===========================================================================

#[derive(Debug, Clone)]
pub struct RestError {
    pub status: u16,
    pub code: Option<String>,
    pub message: String,
}

impl RestError {
    pub fn of(status: u16, message: &str) -> Self {
        Self {
            status,
            code: None,
            message: message.to_string(),
        }
    }
}

/// Perform an HTTP request returning parsed JSON (Null for empty bodies).
pub async fn http_json(
    method: &str,
    path: &str,
    auth: bool,
    body: Option<Value>,
) -> Result<Value, RestError> {
    let init = web_sys::RequestInit::new();
    init.set_method(method);
    if let Some(b) = body {
        init.set_body(&json_to_js(&b));
    }

    let headers = web_sys::Headers::new().map_err(|_| RestError::of(0, "headers init failed"))?;
    let _ = headers.set("Content-Type", "application/json");
    if auth {
        if let Some(token) = get_token() {
            let _ = headers.set("Authorization", &format!("Bearer {}", token));
        }
    }
    init.set_headers(headers.as_ref());

    let url = format!("{}{}", rest_base(), path);
    let req = web_sys::Request::new_with_str_and_init(&url, &init)
        .map_err(|_| RestError::of(0, "request init failed"))?;

    let window = browser::window();
    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&req))
        .await
        .map_err(|_| RestError::of(0, "fetch failed"))?;
    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| RestError::of(0, "bad response type"))?;
    let status = resp.status();

    let text_value = wasm_bindgen_futures::JsFuture::from(
        resp.text().map_err(|_| RestError::of(status, "no body"))?,
    )
    .await
    .map_err(|_| RestError::of(status, "body read failed"))?;
    let text = text_value.as_string().unwrap_or_default();
    let value = if text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).unwrap_or(Value::Null)
    };

    if (200..300).contains(&status) {
        Ok(value)
    } else {
        let message = value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("Request failed")
            .to_string();
        Err(RestError {
            status,
            message,
            code: value["code"].as_str().map(str::to_owned),
        })
    }
}

// ===========================================================================
// Typed route wrappers
// ===========================================================================

pub async fn auth_guest(prefix: &str) -> Result<Value, RestError> {
    http_json(
        "POST",
        "/api/auth/guest",
        false,
        Some(json!({ "prefix": prefix })),
    )
    .await
}

pub async fn auth_register(username: &str, password: &str) -> Result<Value, RestError> {
    http_json(
        "POST",
        "/api/auth/register",
        false,
        Some(json!({ "username": username, "password": password })),
    )
    .await
}

pub async fn auth_login(username: &str, password: &str) -> Result<Value, RestError> {
    http_json(
        "POST",
        "/api/auth/login",
        false,
        Some(json!({ "username": username, "password": password })),
    )
    .await
}

pub async fn auth_me() -> Result<Value, RestError> {
    http_json("GET", "/api/auth/me", true, None).await
}

pub async fn auth_logout() -> Result<Value, RestError> {
    http_json("POST", "/api/auth/logout", true, None).await
}

pub async fn auth_logout_others() -> Result<Value, RestError> {
    http_json("POST", "/api/auth/logout-others", true, None).await
}

pub async fn auth_reset_password(password: &str) -> Result<Value, RestError> {
    http_json(
        "POST",
        "/api/auth/reset-password",
        true,
        Some(json!({"password": password})),
    )
    .await
}

pub async fn auth_upgrade(new_username: &str, password: &str) -> Result<Value, RestError> {
    http_json(
        "POST",
        "/api/auth/upgrade",
        true,
        Some(json!({ "newUsername": new_username, "password": password })),
    )
    .await
}

pub async fn matches_history() -> Result<Value, RestError> {
    http_json("GET", "/api/matches/history", true, None).await
}

pub async fn matches_delete(id: &str) -> Result<Value, RestError> {
    http_json("DELETE", &format!("/api/matches/{}", id), true, None).await
}

pub async fn create_lobby(
    max_players: i64,
    draw_rule: domino_core::DrawRule,
    mode: domino_core::GameMode,
    turn_timer: bool,
) -> Result<Value, RestError> {
    http_json(
        "POST",
        "/api/lobbies",
        true,
        Some(json!({ "maxPlayers": max_players, "drawRule": draw_rule, "mode": mode, "turnTimer": turn_timer })),
    )
    .await
}

pub async fn join_lobby_rest(id: &str) -> Result<Value, RestError> {
    http_json("POST", &format!("/api/lobbies/{}/join", id), true, None).await
}

pub async fn leave_lobby_rest(id: &str) -> Result<Value, RestError> {
    http_json("POST", &format!("/api/lobbies/{}/leave", id), true, None).await
}

pub async fn add_bot_rest(id: &str) -> Result<Value, RestError> {
    http_json("POST", &format!("/api/lobbies/{}/add-bot", id), true, None).await
}

pub async fn kick_rest(id: &str, target_user_id: &str) -> Result<Value, RestError> {
    http_json(
        "POST",
        &format!("/api/lobbies/{}/kick", id),
        true,
        Some(json!({ "targetUserId": target_user_id })),
    )
    .await
}

pub async fn start_game_rest(id: &str) -> Result<Value, RestError> {
    http_json("POST", &format!("/api/lobbies/{}/start", id), true, None).await
}

pub async fn friends_list() -> Result<Value, RestError> {
    http_json("GET", "/api/friends", true, None).await
}

pub async fn friend_request(username: &str) -> Result<Value, RestError> {
    http_json(
        "POST",
        "/api/friends/request",
        true,
        Some(json!({ "username": username })),
    )
    .await
}

pub async fn friend_accept(friendship_id: i64) -> Result<Value, RestError> {
    http_json(
        "POST",
        "/api/friends/accept",
        true,
        Some(json!({ "friendshipId": friendship_id })),
    )
    .await
}

pub async fn friend_reject(friendship_id: i64) -> Result<Value, RestError> {
    http_json(
        "POST",
        "/api/friends/reject",
        true,
        Some(json!({ "friendshipId": friendship_id })),
    )
    .await
}

pub async fn friend_cancel(friendship_id: i64) -> Result<Value, RestError> {
    http_json(
        "POST",
        "/api/friends/cancel",
        true,
        Some(json!({ "friendshipId": friendship_id })),
    )
    .await
}

pub async fn friend_remove(user_id: &str) -> Result<Value, RestError> {
    http_json("DELETE", &format!("/api/friends/{}", user_id), true, None).await
}

pub async fn player_stats(user_id: &str) -> Result<Value, RestError> {
    http_json("GET", &format!("/api/stats/{}", user_id), false, None).await
}

pub async fn leaderboard(period: &str) -> Result<Value, RestError> {
    http_json(
        "GET",
        &format!("/api/leaderboard?period={period}"),
        false,
        None,
    )
    .await
}

// ===========================================================================
// Socket implementation
// ===========================================================================

impl NetClient {
    fn connect_socket(&mut self) {
        self.last_activity_ms = browser::now_ms();
        let url = ws_url();
        log_net(&format!("connecting {}", url));
        match web_sys::WebSocket::new(&url) {
            Ok(ws) => {
                ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
                self.attach_closures(&ws);
                self.ws = Some(ws);
                self.ws_open = false;
            }
            Err(_) => {
                log_net("ws create failed");
                // keep open flag false so sends queue
                self.ws_open = false;
                self.schedule_reconnect();
            }
        }
    }

    fn attach_closures(&mut self, ws: &web_sys::WebSocket) {
        let onmessage =
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(|e: web_sys::MessageEvent| {
                if let Some(txt) = e.data().as_string() {
                    process_frame(&txt);
                }
            });
        let onopen = Closure::<dyn FnMut(web_sys::Event)>::new(|_: web_sys::Event| {
            NET.with_borrow_mut(|opt| {
                if let Some(n) = opt.as_mut() {
                    n.ws_open = true;
                }
            });
            log_net("ws open");
        });
        let onerror = Closure::<dyn FnMut(web_sys::Event)>::new(|_: web_sys::Event| {
            log_net("WebSocket connection failed; waiting for reconnect");
        });
        let onclose = Closure::<dyn FnMut(web_sys::CloseEvent)>::new(in_browser_scope(
            |e: web_sys::CloseEvent| {
                log_net(&format!("ws closed {} {}", e.code(), e.reason()));
                NET.with_borrow_mut(|opt| {
                    if let Some(n) = opt.as_mut() {
                        n.on_closed();
                    }
                });
                dispatch_queued();
            },
        ));

        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));

        WS_CLOSURES.with_borrow_mut(|store| {
            store.clear();
            store.push(ClosureHolder {
                _closures: (onmessage, onopen, onerror, onclose),
            });
        });
    }

    fn detach_closures(&mut self) {
        if let Some(ws) = self.ws.as_ref() {
            ws.set_onmessage(None);
            ws.set_onopen(None);
            ws.set_onerror(None);
            ws.set_onclose(None);
        }
        // Retain callback storage until next attachment so an executing onclose is not dropped.
    }

    /// Clear socket state; `keepalive` decides whether a reconnect follows.
    fn reset_socket(&mut self) {
        if self.ws.is_some() {
            self.detach_closures();
            if let Some(ws) = self.ws.take() {
                let _ = ws.close();
            }
        }
        self.ws = None;
        self.ws_open = false;
        self.ns_connected = false;
        self.outbox.clear();
        for (_, (cb, _)) in self.pending_acks.drain() {
            if self.keepalive {
                READY_ACKS.with_borrow_mut(|q| {
                    q.push((cb, json!({"error":"Connection lost. Rejoining the table…"})))
                });
            }
        }
        if self.keepalive {
            READY_EVENTS.with_borrow_mut(|q| q.push(("disconnect".into(), Value::Null)));
        }
    }

    fn on_closed(&mut self) {
        let was_keepalive = self.keepalive;
        self.reset_socket();
        if was_keepalive {
            self.schedule_reconnect();
        }
    }

    fn schedule_reconnect(&mut self) {
        if !self.keepalive {
            return;
        }
        let delay = self.reconnect_delay_ms.max(1000);
        self.reconnect_delay_ms = (delay * 2).min(5_000);
        log_net(&format!("reconnect in {}ms", delay));
        crate::game::timers::set_timeout(delay, move || {
            NET.with_borrow_mut(|opt| {
                if let Some(n) = opt.as_mut() {
                    if n.keepalive && n.ws.is_none() {
                        n.connect_socket();
                    }
                }
            });
        });
    }

    fn send_raw(&mut self, frame: &str) {
        let sent = self
            .ws
            .as_ref()
            .is_some_and(|ws| ws.send_with_str(frame).is_ok());
        if !sent {
            self.reset_socket();
            self.schedule_reconnect();
        }
    }

    fn emit_inner(&mut self, event: &str, payload: Value, ack: Option<AckCallback>) {
        if !(self.ns_connected && self.ws_open) && event != "join_lobby" {
            if let Some(cb) = ack {
                READY_ACKS.with_borrow_mut(|q| q.push((cb, json!({"error":"Reconnecting. Your move was not sent; try again once connected."}))));
            }
            return;
        }
        let mut frame = String::from("42");
        if let Some(cb) = ack {
            self.next_ack_id += 1;
            let id = self.next_ack_id;
            frame.push_str(&id.to_string());
            self.pending_acks
                .insert(id, (cb, browser::now_ms() + 12_000.0));
        }
        let args = if payload.is_null() {
            vec![json!(event)]
        } else {
            vec![json!(event), payload]
        };
        frame.push_str(&serde_json::to_string(&args).unwrap_or_default());
        log_net(&format!("→ {}", frame));
        if self.ns_connected && self.ws_open {
            self.send_raw(&frame);
        } else {
            self.outbox.push(frame);
        }
    }

    fn flush_outbox(&mut self) {
        let queued = std::mem::take(&mut self.outbox);
        for frame in queued {
            if self.ns_connected && self.ws_open {
                log_net(&format!("flushing {}", frame));
                self.send_raw(&frame);
            } else {
                self.outbox.push(frame);
                break;
            }
        }
    }

    fn handle_engine_frame(&mut self, frame: &str) {
        self.last_activity_ms = browser::now_ms();
        let bytes = frame.as_bytes();
        match bytes.first().copied() {
            // engine OPEN (`0{"sid":...,"pingInterval":25000,"pingTimeout":20000}`)
            Some(b'0') => {
                if let Ok(v) = serde_json::from_str::<Value>(&frame[1..]) {
                    self.socket_sid = v.get("sid").and_then(Value::as_str).map(String::from);
                    if let Some(pi) = v.get("pingInterval").and_then(Value::as_f64) {
                        self.ping_interval_ms = pi;
                    }
                    if let Some(pt) = v.get("pingTimeout").and_then(Value::as_f64) {
                        self.ping_timeout_ms = pt;
                    }
                }
                // Authenticate the Socket.IO namespace.
                let connect = match &self.token {
                    Some(t) if !t.is_empty() => format!("40{}", json!({ "token": t })),
                    _ => "40".to_string(),
                };
                self.send_raw(&connect);
            }
            // engine PING (`2`) → reply engine PONG (`3`)
            Some(b'2') => self.send_raw("3"),
            // engine PONG (`3`, unsolicited from our side)
            Some(b'3') => {}
            // engine CLOSE (`1`)
            Some(b'1') => {
                log_net("engine close");
            }
            // engine MESSAGE (`4`) → socket packet follows
            Some(b'4') => self.handle_socket_packet(&frame[1..]),
            _ => {
                let head: String = frame.chars().take(24).collect();
                log_net(&format!("ignoring frame {}", head));
            }
        }
    }

    /// Handle the Socket.IO v5 packet that follows the engine `4` prefix.
    fn handle_socket_packet(&mut self, packet: &str) {
        match packet.as_bytes().first().copied() {
            // namespace CONNECT ack (`40{...}`)
            Some(b'0') => {
                self.ns_connected = true;
                self.reconnect_delay_ms = 1000;
                log_net("namespace connected");
                self.flush_outbox();
                READY_EVENTS.with_borrow_mut(|q| q.push(("connect".into(), Value::Null)));
            }
            // namespace disconnect (`41`)
            Some(b'1') => {
                self.ns_connected = false;
                log_net("namespace closed by server");
            }
            // EVENT (`42...`) — payload is the args array `["event", data?]`
            Some(b'2') => {
                let parsed: Vec<Value> = match serde_json::from_str(&packet[1..]) {
                    Ok(v) => v,
                    Err(e) => {
                        log_net(&format!("bad event json: {}", e));
                        return;
                    }
                };
                let name = parsed
                    .first()
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let arg = parsed.get(1).cloned().unwrap_or(Value::Null);
                READY_EVENTS.with_borrow_mut(|q| q.push((name, arg)));
            }
            // ACK (`43<ackId><payload-json>`)
            Some(b'3') => {
                let rest = &packet[1..];
                let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
                if digits == 0 {
                    log_net("ack without id ignored");
                    return;
                }
                let Ok(id) = rest[..digits].parse::<u64>() else {
                    return;
                };
                let body = &rest[digits..];
                let raw = if body.is_empty() {
                    Value::Null
                } else {
                    serde_json::from_str::<Value>(body).unwrap_or(Value::Null)
                };
                // Server sends an args list as the ACK payload; deliver the
                // single arg the application would receive from its callback.
                let value = match raw {
                    Value::Array(items) => {
                        if items.len() == 1 {
                            items.into_iter().next().unwrap_or(Value::Null)
                        } else if items.is_empty() {
                            Value::Null
                        } else {
                            Value::Array(items)
                        }
                    }
                    other => other,
                };
                if let Some((cb, _)) = self.pending_acks.remove(&id) {
                    READY_ACKS.with_borrow_mut(|q| q.push((cb, value)));
                }
            }
            // CONNECT_ERROR (`44{...}`)
            Some(b'4') => {
                log_net(&format!("connect error: {}", packet));
                self.keepalive = false;
                READY_EVENTS.with_borrow_mut(|q| {
                    q.push((
                        "connect_error".into(),
                        serde_json::from_str(&packet[1..]).unwrap_or(Value::Null),
                    ))
                });
            }
            _ => {}
        }
    }
}

/// Entry point for the WebSocket message closure: process one frame, then
/// dispatch everything it queued *outside* the NET borrow.
fn process_frame(frame: &str) {
    if frame.is_empty() {
        return;
    }
    NET.with_borrow_mut(|opt| {
        if let Some(n) = opt.as_mut() {
            n.handle_engine_frame(frame);
        }
    });

    dispatch_queued();
}

/// Drain outside NET borrows; recursive emits are handled by the outer drain.
fn dispatch_queued() {
    if DISPATCHING.with(|flag| flag.replace(true)) {
        return;
    }
    loop {
        let events = READY_EVENTS.with(RefCell::take);
        let acks = READY_ACKS.with(RefCell::take);
        if events.is_empty() && acks.is_empty() {
            break;
        }
        for (name, payload) in events {
            HANDLERS.with_borrow_mut(|m| {
                if let Some(list) = m.get_mut(name.as_str()) {
                    for handler in list.iter_mut() {
                        handler(&payload);
                    }
                }
            });
        }
        for (cb, value) in acks {
            cb(value);
        }
    }
    DISPATCHING.with(|flag| flag.set(false));
}
