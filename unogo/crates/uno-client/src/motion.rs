//! Direct card manipulation. Pointer frames update a floating DOM copy, not
//! authoritative game state or the whole Dioxus tree. The server commits plays.
use crate::{browser, model::Table};
use dioxus::prelude::ReadableExt;
use std::cell::{Cell, RefCell};
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Pending,
    Drag,
    Scroll,
}
struct Grab {
    bundle: Vec<u8>,
    pointer: i32,
    card: u8,
    match_id: Option<String>,
    turn: usize,
    source: HtmlElement,
    ghost: Option<HtmlElement>,
    track: HtmlElement,
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    left: i32,
    legal: bool,
    mode: Mode,
}
thread_local! {
    static GRAB: RefCell<Option<Grab>> = const { RefCell::new(None) };
    static SUPPRESS: Cell<u64> = const { Cell::new(0) };
    static LAST_FLIGHT: Cell<Option<(u8,u64)>> = const { Cell::new(None) };
}
fn element(selector: &str) -> Option<HtmlElement> {
    browser::document()
        .query_selector(selector)
        .ok()??
        .dyn_into()
        .ok()
}
fn css(el: &HtmlElement, name: &str, value: &str) {
    let _ = el.style().set_property(name, value);
}
fn class(el: &HtmlElement, name: &str, enabled: bool) {
    let _ = el.class_list().toggle_with_force(name, enabled);
}
fn table_class(name: &str, enabled: bool) {
    if let Some(el) = element(".game-board") {
        class(&el, name, enabled);
    }
}
fn reduced() -> bool {
    browser::window()
        .match_media("(prefers-reduced-motion: reduce)")
        .ok()
        .flatten()
        .is_some_and(|m| m.matches())
}
fn copy_card(source: &HtmlElement, x: f64, y: f64, width: f64, height: f64) -> Option<HtmlElement> {
    let ghost: HtmlElement = browser::document()
        .create_element("div")
        .ok()?
        .dyn_into()
        .ok()?;
    ghost.set_class_name("card-flight");
    let face = source.first_element_child()?;
    ghost
        .append_child(&face.clone_node_with_deep(true).ok()?)
        .ok()?;
    css(&ghost, "left", &format!("{x}px"));
    css(&ghost, "top", &format!("{y}px"));
    css(&ghost, "width", &format!("{width}px"));
    css(&ghost, "height", &format!("{height}px"));
    ghost.set_attribute("aria-hidden", "true").ok()?;
    let layer = element(".card-motion-layer").or_else(|| {
        let layer: HtmlElement = browser::document()
            .create_element("div")
            .ok()?
            .dyn_into()
            .ok()?;
        layer.set_class_name("card-motion-layer");
        layer.set_attribute("aria-hidden", "true").ok()?;
        browser::document().body()?.append_child(&layer).ok()?;
        Some(layer)
    })?;
    layer.append_child(&ghost).ok()?;
    Some(ghost)
}
fn lift(grab: &mut Grab) {
    if grab.mode == Mode::Drag {
        return;
    }
    let rect = grab.source.get_bounding_client_rect();
    grab.ghost = copy_card(
        &grab.source,
        rect.x(),
        rect.y(),
        rect.width(),
        rect.height(),
    );
    if let Some(ghost) = &grab.ghost {
        class(ghost, "is-held", true);
        if grab.bundle.len() > 1 {
            if let Ok(badge) = browser::document().create_element("span") {
                badge.set_class_name("bundle-count");
                badge.set_text_content(Some(&format!("×{}", grab.bundle.len())));
                let _ = ghost.append_child(&badge);
            }
        }
    }
    grab.mode = Mode::Drag;
    class(&grab.source, "is-dragging", true);
    table_class("holding-card", true);
}
pub fn start(card: u8, pointer: i32, x: f64, y: f64, legal: bool, bundle: Vec<u8>) {
    if GRAB.with(|g| g.borrow().is_some()) {
        return;
    }
    let Some(source) = element(&format!(".hand-card[data-card-id='{card}']")) else {
        return;
    };
    let _ = source.style().remove_property("animation");
    let Some(track) = element(".hand-track") else {
        return;
    };
    let Some(table) = crate::store::TABLE.read().clone() else {
        return;
    };
    let _ = source.set_pointer_capture(pointer);
    class(&source, "is-pressed", true);
    let left = track.scroll_left();
    GRAB.with(|g| {
        *g.borrow_mut() = Some(Grab {
            bundle,
            pointer,
            card,
            match_id: table.match_id,
            turn: table.game.map_or(0, |g| g.turn),
            source,
            ghost: None,
            track,
            x,
            y,
            dx: 0.,
            dy: 0.,
            left,
            legal,
            mode: Mode::Pending,
        })
    });
    dioxus::dioxus_core::spawn_forever(async move {
        browser::sleep(180).await;
        GRAB.with(|g| {
            if let Some(grab) = g.borrow_mut().as_mut() {
                if grab.pointer == pointer && grab.mode == Mode::Pending {
                    lift(grab);
                }
            }
        });
    });
}
fn over_pile(x: f64, y: f64) -> bool {
    element(".discard-pile").is_some_and(|e| {
        let r = e.get_bounding_client_rect();
        x >= r.left() - 38. && x <= r.right() + 38. && y >= r.top() - 38. && y <= r.bottom() + 38.
    })
}
pub fn move_pointer(pointer: i32, x: f64, y: f64) {
    GRAB.with(|g| {
        let mut active = g.borrow_mut();
        let Some(grab) = active.as_mut() else { return };
        if grab.pointer != pointer {
            return;
        }
        grab.dx = x - grab.x;
        grab.dy = y - grab.y;
        if grab.mode == Mode::Pending && grab.dx.abs().max(grab.dy.abs()) > 9. {
            if grab.dx.abs() > grab.dy.abs() * 1.25 {
                grab.mode = Mode::Scroll;
            } else {
                lift(grab);
            }
        }
        if grab.mode == Mode::Scroll {
            grab.track.set_scroll_left(grab.left - grab.dx as i32);
        } else if grab.mode == Mode::Drag {
            class(&grab.source, "is-dragging", true);
            if let Some(ghost) = &grab.ghost {
                css(
                    ghost,
                    "transform",
                    &format!(
                        "translate3d({}px,{}px,0) rotate({}deg) scale(1.08)",
                        grab.dx,
                        grab.dy,
                        (grab.dx / 28.).clamp(-9., 9.)
                    ),
                );
            }
            table_class("drop-ready", grab.legal && over_pile(x, y));
        }
    });
}
fn settle(ghost: HtmlElement, dx: f64, dy: f64, scale: f64, fade: bool) {
    css(
        &ghost,
        "transition",
        if reduced() {
            "opacity 80ms"
        } else {
            "transform 260ms cubic-bezier(.16,1,.3,1),opacity 220ms"
        },
    );
    dioxus::dioxus_core::spawn_forever(async move {
        browser::sleep(16).await;
        css(
            &ghost,
            "transform",
            &format!("translate3d({dx}px,{dy}px,0) rotate(0deg) scale({scale})"),
        );
        if fade {
            css(&ghost, "opacity", "0.12");
        }
        browser::sleep(280).await;
        ghost.remove();
    });
}
fn clean(grab: &Grab) {
    let _ = grab.source.release_pointer_capture(grab.pointer);
    class(&grab.source, "is-pressed", false);
    class(&grab.source, "is-dragging", false);
    table_class("holding-card", false);
    table_class("drop-ready", false);
}
pub fn finish(pointer: i32, x: f64, y: f64, cancelled: bool) -> Option<u8> {
    let matches = GRAB.with(|g| g.borrow().as_ref().is_some_and(|g| g.pointer == pointer));
    if !matches {
        return None;
    }
    let grab = GRAB.with(|g| g.borrow_mut().take())?;
    let played = grab.mode == Mode::Drag
        && !cancelled
        && grab.legal
        && over_pile(x, y)
        && if grab.bundle.is_empty() {
            playable(grab.card)
        } else {
            playable_group(&grab.bundle)
        };
    if grab.mode != Mode::Pending {
        SUPPRESS.with(|s| s.set(browser::now() + 450));
    }
    clean(&grab);
    if let Some(ghost) = grab.ghost {
        class(&ghost, "is-held", false);
        let is_wild = crate::store::TABLE
            .read()
            .as_ref()
            .and_then(|t| t.game.as_ref())
            .and_then(|g| g.hand.iter().find(|c| c.id == grab.card))
            .is_some_and(|c| c.color.is_none());
        if played && !is_wild {
            if let Some(pile) = element(".discard-pile") {
                let from = grab.source.get_bounding_client_rect();
                let to = pile.get_bounding_client_rect();
                settle(
                    ghost,
                    to.x() - from.x(),
                    to.y() - from.y(),
                    to.width() / from.width(),
                    true,
                );
                LAST_FLIGHT.with(|s| {
                    s.set(Some((
                        grab.bundle.last().copied().unwrap_or(grab.card),
                        browser::now(),
                    )))
                });
            } else {
                ghost.remove();
            }
        } else {
            settle(ghost, 0., 0., 1., false);
        }
    }
    played.then_some(grab.card)
}
pub fn cancel() {
    let pointer = GRAB.with(|g| g.borrow().as_ref().map(|g| g.pointer));
    if let Some(pointer) = pointer {
        finish(pointer, -1000., -1000., true);
    }
}
pub fn consume_click() -> bool {
    SUPPRESS.with(|s| {
        let suppress = s.get() > browser::now();
        s.set(0);
        suppress
    })
}
pub fn stop_deal(card: u8) {
    if let Some(el) = element(&format!(".hand-card[data-card-id='{card}']")) {
        let _ = el.style().remove_property("animation");
    }
}
pub fn active() -> bool {
    GRAB.with(|g| g.borrow().is_some())
}
pub fn playable_group(cards: &[u8]) -> bool {
    !*crate::store::BUSY.read()
        && *crate::store::CONNECTED.read()
        && crate::store::TABLE.read().as_ref().is_some_and(|t| {
            t.game.as_ref().is_some_and(|g| {
                g.turn == t.local_seat && uno_core::number_group_playable(g, cards)
            })
        })
}
pub fn playable(card: u8) -> bool {
    !*crate::store::BUSY.read()
        && *crate::store::CONNECTED.read()
        && crate::store::TABLE.read().as_ref().is_some_and(|t| {
            t.game.as_ref().is_some_and(|g| {
                !g.paused
                    && g.turn == t.local_seat
                    && g.playable.contains(&card)
                    && matches!(
                        g.phase,
                        uno_core::Phase::Turn | uno_core::Phase::Drawn { .. }
                    )
            })
        })
}
pub fn reconcile(table: &Table) {
    let invalid = GRAB.with(|g| {
        g.borrow().as_ref().is_some_and(|grab| {
            grab.match_id != table.match_id
                || table.game.as_ref().is_none_or(|g| {
                    g.paused || g.turn != grab.turn || !g.hand.iter().any(|c| c.id == grab.card)
                })
        })
    });
    if invalid {
        cancel();
    }
}
fn fly(source: &HtmlElement, target: &HtmlElement, back: bool, delay: i32) {
    if reduced() {
        return;
    }
    let from = source.get_bounding_client_rect();
    let to = target.get_bounding_client_rect();
    if from.width() == 0. || to.width() == 0. {
        return;
    }
    let (width, height) = if back {
        (48., 67.)
    } else {
        (from.width(), from.height())
    };
    let Some(ghost) = copy_card(source, from.x(), from.y(), width, height) else {
        return;
    };
    if back {
        ghost.set_inner_html("<div class='flying-back'>UG</div>");
    }
    dioxus::dioxus_core::spawn_forever(async move {
        browser::sleep(delay).await;
        settle(
            ghost,
            to.x() - from.x(),
            to.y() - from.y(),
            (to.width() / width).min(2.),
            true,
        );
    });
}
/// Animate only snapshot differences. Polling the same state never replays motion.
pub fn snapshot(old: Option<&Table>, table: &Table) {
    reconcile(table);
    let Some(game) = table.game.as_ref() else {
        return;
    };
    if reduced() {
        return;
    }
    if let Some(previous) = old
        .filter(|old| old.match_id == table.match_id)
        .and_then(|t| t.game.as_ref())
    {
        if previous.top.id != game.top.id {
            let recent = LAST_FLIGHT.with(|s| {
                s.get()
                    .is_some_and(|(card, time)| card == game.top.id && browser::now() - time < 1500)
            });
            if !recent {
                let origin = if previous.hand.iter().any(|c| c.id == game.top.id) {
                    element(&format!(".hand-card[data-card-id='{}']", game.top.id))
                } else {
                    element(&format!(
                        ".opponent[data-seat='{}'] .opponent-stack",
                        previous.turn
                    ))
                };
                if let (Some(source), Some(target)) = (origin, element(".discard-pile")) {
                    fly(
                        &source,
                        &target,
                        !previous.hand.iter().any(|c| c.id == game.top.id),
                        0,
                    );
                }
            }
        }
        for (seat, count) in game.counts.iter().enumerate() {
            if seat == table.local_seat {
                continue;
            }
            let added = count.saturating_sub(previous.counts.get(seat).copied().unwrap_or(*count));
            if let (Some(deck), Some(target)) = (
                element(".draw-pile"),
                element(&format!(".opponent[data-seat='{seat}'] .opponent-stack")),
            ) {
                for i in 0..added.min(4) {
                    fly(&deck, &target, true, (i * 40) as i32);
                }
            }
        }
    }
    let old_ids: Vec<u8> = old
        .filter(|old| old.match_id == table.match_id)
        .and_then(|t| t.game.as_ref())
        .map(|g| g.hand.iter().map(|c| c.id).collect())
        .unwrap_or_default();
    let added: Vec<u8> = game
        .hand
        .iter()
        .filter(|c| !old_ids.contains(&c.id))
        .map(|c| c.id)
        .collect();
    if !added.is_empty() {
        dioxus::dioxus_core::spawn_forever(async move {
            browser::sleep(32).await;
            let Some(deck) = element(".draw-pile") else {
                return;
            };
            let from = deck.get_bounding_client_rect();
            for (i, card) in added.iter().take(12).enumerate() {
                if let Some(el) = element(&format!(".hand-card[data-card-id='{card}']")) {
                    let to = el.get_bounding_client_rect();
                    if to.right() < 0.
                        || to.left()
                            > browser::window()
                                .inner_width()
                                .ok()
                                .and_then(|v| v.as_f64())
                                .unwrap_or(2000.)
                    {
                        continue;
                    }
                    css(&el, "--deal-x", &format!("{}px", from.x() - to.x()));
                    css(&el, "--deal-y", &format!("{}px", from.y() - to.y()));
                    css(
                        &el,
                        "animation",
                        &format!(
                            "card-dealt 380ms {}ms cubic-bezier(.16,1,.3,1) both",
                            (i * 28).min(200)
                        ),
                    );
                    let el = el.clone();
                    dioxus::dioxus_core::spawn_forever(async move {
                        browser::sleep(650).await;
                        let _ = el.style().remove_property("animation");
                    });
                }
            }
        });
    }
}
