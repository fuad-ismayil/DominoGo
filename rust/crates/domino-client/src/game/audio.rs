//! Natural tabletop foley, decoded once and mixed through one browser audio graph.
//! Playback allocates a buffer-source node, never a media element or network request.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{AudioBuffer, AudioContext, AudioContextState, GainNode, Response};

const ENABLED_KEY: &str = "domino.audio.enabled";
const VOLUME_KEY: &str = "domino.audio.volume";
const ASSETS: [&str; 9] = [
    "place0",
    "place1",
    "place2",
    "draw-soft0",
    "draw-soft1",
    "win",
    "lose",
    "turn",
    "error",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    Place,
    Draw,
    Win,
    Lose,
    Turn,
    Error,
}

struct Mixer {
    context: AudioContext,
    gain: GainNode,
    buffers: HashMap<&'static str, AudioBuffer>,
}
thread_local! {
    static MIXER: RefCell<Option<Mixer>> = const { RefCell::new(None) };
    static INITIALIZED: Cell<bool> = const { Cell::new(false) };
    static ENABLED: Cell<bool> = const { Cell::new(true) };
    static VOLUME: Cell<f64> = const { Cell::new(0.55) };
    static LAST_DRAW: Cell<f64> = const { Cell::new(0.) };
    static NEXT_VARIANT: Cell<usize> = const { Cell::new(0) };
}

fn stored(key: &str) -> Option<String> {
    crate::browser::window()
        .local_storage()
        .ok()
        .flatten()?
        .get_item(key)
        .ok()
        .flatten()
}
fn persist(key: &str, value: &str) {
    if let Ok(Some(storage)) = crate::browser::window().local_storage() {
        let _ = storage.set_item(key, value);
    }
}

/// Prime decoded buffers without playing anything. Browser gestures explicitly
/// resume the shared context, including after a mobile browser suspends its tab.
pub fn init_audio() {
    if INITIALIZED.replace(true) {
        return;
    }
    ENABLED.set(stored(ENABLED_KEY).as_deref() != Some("false"));
    VOLUME.set(
        stored(VOLUME_KEY)
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(0.55)
            .clamp(0.0, 1.0),
    );
    let Ok(context) = AudioContext::new() else {
        return;
    };
    let Ok(gain) = context.create_gain() else {
        return;
    };
    gain.gain()
        .set_value(if enabled() { volume() as f32 } else { 0.0 });
    if gain
        .connect_with_audio_node(&context.destination())
        .is_err()
    {
        return;
    }
    MIXER.with_borrow_mut(|m| {
        *m = Some(Mixer {
            context: context.clone(),
            gain,
            buffers: HashMap::new(),
        })
    });
    for name in ASSETS {
        let context = context.clone();
        spawn_local(async move {
            match load(&context, name).await {
                Ok(buffer) => MIXER.with_borrow_mut(|m| {
                    if let Some(m) = m {
                        m.buffers.insert(name, buffer);
                    }
                }),
                Err(_) => web_sys::console::warn_1(&JsValue::from_str(&format!(
                    "Could not preload sound {name}"
                ))),
            }
        });
    }
    for event in ["pointerdown", "keydown", "touchend"] {
        let listener = Closure::<dyn FnMut(web_sys::Event)>::new(|_| unlock());
        if crate::browser::document()
            .add_event_listener_with_callback(event, listener.as_ref().unchecked_ref())
            .is_ok()
        {
            // Three application-lifetime listeners; no per-render registration.
            listener.forget();
        }
    }
}

async fn load(context: &AudioContext, name: &'static str) -> Result<AudioBuffer, JsValue> {
    let response: Response =
        JsFuture::from(crate::browser::window().fetch_with_str(&format!("/sounds/{name}.wav")))
            .await?
            .dyn_into()?;
    if !response.ok() {
        return Err(JsValue::from_str("sound request failed"));
    }
    let bytes = JsFuture::from(response.array_buffer()?)
        .await?
        .dyn_into::<js_sys::ArrayBuffer>()?;
    JsFuture::from(context.decode_audio_data(&bytes)?)
        .await?
        .dyn_into()
}

/// Must be called synchronously from a gesture; the persistent document
/// listeners do this automatically. Rejected autoplay promises are consumed.
pub fn unlock() {
    MIXER.with_borrow(|m| {
        let Some(m) = m else {
            return;
        };
        if m.context.state() != AudioContextState::Running {
            if let Ok(promise) = m.context.resume() {
                spawn_local(async move {
                    let _ = JsFuture::from(promise).await;
                });
            }
        }
    });
}

pub fn enabled() -> bool {
    ENABLED.get()
}
pub fn volume() -> f64 {
    VOLUME.get()
}
pub fn set_enabled(value: bool) {
    ENABLED.set(value);
    persist(ENABLED_KEY, if value { "true" } else { "false" });
    update_gain();
    if value {
        unlock();
    }
}
pub fn set_volume(value: f64) {
    if !value.is_finite() {
        return;
    }
    VOLUME.set(value.clamp(0.0, 1.0));
    persist(VOLUME_KEY, &volume().to_string());
    update_gain();
    unlock();
}
fn update_gain() {
    MIXER.with_borrow(|m| {
        if let Some(m) = m {
            m.gain
                .gain()
                .set_value(if enabled() { volume() as f32 } else { 0.0 });
        }
    });
}

/// Start an already-decoded recording. A suspended/blocked context is a quiet
/// no-op: old game sounds must never queue up and play after a later gesture.
pub fn play(sound: Sound) {
    if !enabled() {
        return;
    }
    if sound == Sound::Draw {
        let now = crate::browser::now_ms();
        if now - LAST_DRAW.get() < 180. {
            return;
        }
        LAST_DRAW.set(now);
    }
    let variant = NEXT_VARIANT.get();
    NEXT_VARIANT.set(variant.wrapping_add(1));
    let key = match sound {
        Sound::Place => ["place0", "place1", "place2"][variant % 3],
        Sound::Draw => ["draw-soft0", "draw-soft1"][variant % 2],
        Sound::Win => "win",
        Sound::Lose => "lose",
        Sound::Turn => "turn",
        Sound::Error => "error",
    };
    MIXER.with_borrow(|m| {
        let Some(m) = m else {
            return;
        };
        if m.context.state() != AudioContextState::Running {
            return;
        }
        let Some(buffer) = m.buffers.get(key) else {
            return;
        };
        let Ok(source) = m.context.create_buffer_source() else {
            return;
        };
        source.set_buffer(Some(buffer));
        if sound == Sound::Draw {
            if let Ok(soft) = m.context.create_gain() {
                soft.gain().set_value(0.65);
                if source.connect_with_audio_node(&soft).is_ok()
                    && soft.connect_with_audio_node(&m.gain).is_ok()
                {
                    let _ = source.start();
                }
            }
        } else if source.connect_with_audio_node(&m.gain).is_ok() {
            let _ = source.start();
        }
    });
    let pattern = match sound {
        Sound::Place => vec![12],
        Sound::Draw => vec![6],
        Sound::Turn => vec![10, 30, 10],
        Sound::Win => vec![25, 40, 25],
        Sound::Lose => vec![40],
        Sound::Error => vec![20],
    };
    let values = js_sys::Array::new();
    for ms in pattern {
        values.push(&JsValue::from_f64(ms as f64));
    }
    crate::browser::vibrate(&values.into());
}
