//! Canvas particle celebrations with no external animation dependency.
use std::cell::RefCell;

use wasm_bindgen::prelude::*;

#[derive(Clone, Copy)]
struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    size: f64,
    rot: f64,
    rot_speed: f64,
    ttl: f64,
    color: usize,
}

// Palette in the canvas-confetti default spirit, tuned to the emerald/amber
// theme so the celebration reads as "our" confetti.
const COLORS: [&str; 8] = [
    "#f59e0b", "#fbbf24", "#22c55e", "#34d399", "#3b82f6", "#ef4444", "#a855f7", "#ffffff",
];

thread_local! {
    static PARTICLES: RefCell<Vec<Particle>> = const { RefCell::new(Vec::new()) };
    static RUNNING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static RNG_STATE: std::cell::Cell<u64> = const { std::cell::Cell::new(0x9E3779B97F4A7C15) };
}

// Tiny xorshift; no external rand dep needed for a particle toy.
fn rng_f64() -> f64 {
    let mut x = RNG_STATE.get();
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    RNG_STATE.set(x);
    (x as f64 / u64::MAX as f64).abs().clamp(0.0, 1.0)
}

fn canvas_viewport() -> Option<(web_sys::HtmlCanvasElement, f64, f64)> {
    let doc = crate::browser::document();
    let el = doc.get_element_by_id("confetti-canvas")?;
    let canvas = el.dyn_into::<web_sys::HtmlCanvasElement>().ok()?;
    let w = crate::browser::window()
        .inner_width()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(800.0);
    let h = crate::browser::window()
        .inner_height()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(600.0);
    Some((canvas, w, h))
}

/// One burst. `origin_x`/`origin_y` are viewport fractions like
/// canvas-confetti; `angle_deg` = launch direction in canvas-confetti
/// convention (90 fires straight up → here: negative screen-y), `spread_deg`
/// the half-cone width.
fn burst(particle_count: usize, angle_deg: f64, spread_deg: f64, origin_x: f64, origin_y: f64) {
    let Some((_, w, h)) = canvas_viewport() else {
        return;
    };
    let (ox, oy) = (origin_x * w, origin_y * h);

    PARTICLES.with_borrow_mut(|particles| {
        for _ in 0..particle_count {
            let angle = (angle_deg + (rng_f64() * 2.0 - 1.0) * spread_deg).to_radians();
            let speed = 7.0 + rng_f64() * 9.0;
            particles.push(Particle {
                x: ox,
                y: oy,
                vx: angle.cos() * speed,
                vy: -angle.sin().abs().max(0.0).abs() * speed - 2.0,
                size: 5.0 + rng_f64() * 5.0,
                rot: rng_f64() * std::f64::consts::TAU,
                rot_speed: (rng_f64() * 2.0 - 1.0) * 0.25,
                ttl: 1.4 + rng_f64(),
                color: (rng_f64() * COLORS.len() as f64) as usize % COLORS.len(),
            });
        }
    });

    ensure_loop();
}

fn burst_left() {
    burst(40, 60.0, 55.0, 0.0, 0.7);
}

fn burst_right() {
    burst(40, 120.0, 55.0, 1.0, 0.7);
}

/// A centered burst marking a round win.
pub fn round_win() {
    burst(80, 90.0, 60.0, 0.5, 0.7);
}

/// Celebrate a match win with diagonal bursts over 1.5 seconds.
pub fn match_win() {
    dioxus::prelude::spawn(async {
        burst_left();
        let end = crate::browser::now_ms() + 1500.0;
        while crate::browser::now_ms() < end {
            crate::game::timers::sleep_ms(150).await;
            if !RUNNING.get() {
                break;
            }
            burst_left();
            burst_right();
        }
    });
}

fn step_and_draw() {
    let Some((canvas, w, h)) = canvas_viewport() else {
        return;
    };
    let _ = canvas.set_attribute("width", &w.to_string());
    let _ = canvas.set_attribute("height", &h.to_string());

    let ctx = match canvas.get_context("2d") {
        Ok(Some(ctx)) => match ctx.dyn_into::<web_sys::CanvasRenderingContext2d>() {
            Ok(ctx) => ctx,
            Err(_) => return,
        },
        _ => return,
    };
    ctx.clear_rect(0.0, 0.0, w, h);

    const GRAVITY: f64 = 0.18;
    const DT: f64 = 1.0 / 60.0;

    PARTICLES.with_borrow_mut(|particles| {
        particles.retain_mut(|p| {
            p.vy += GRAVITY;
            p.x += p.vx * DT * 60.0;
            p.y += p.vy * DT * 60.0;
            p.rot += p.rot_speed * DT * 60.0;
            p.ttl -= DT;
            p.x = p.x.clamp(-60.0, w + 60.0);
            if p.y > h + 60.0 || p.ttl <= 0.0 {
                return false;
            }

            // Wobble like a tumbling piece of paper; fade near the end.
            let wobble = p.rot.sin().abs() * 2.0;
            ctx.save();
            let _ = ctx.translate(p.x, p.y);
            let _ = ctx.rotate(p.rot);
            ctx.set_global_alpha((p.ttl / 1.2).min(1.0));
            ctx.set_fill_style_str(COLORS[p.color % COLORS.len()]);
            ctx.fill_rect(
                -(p.size + wobble) / 2.0,
                -p.size / 2.0,
                p.size + wobble,
                p.size,
            );
            ctx.restore();
            true
        });
    });

    if PARTICLES.with_borrow(|p| p.is_empty()) {
        ctx.clear_rect(0.0, 0.0, w, h);
    }
}

/// Keep driving the particle simulation while any particle is alive.
fn ensure_loop() {
    if RUNNING.get() {
        return;
    }
    RUNNING.set(true);
    dioxus::prelude::spawn(async {
        loop {
            crate::game::timers::sleep_ms(16).await;
            step_and_draw();
            if !RUNNING.get() || PARTICLES.with_borrow(|p| p.is_empty()) {
                RUNNING.set(false);
                break;
            }
        }
    });
}
