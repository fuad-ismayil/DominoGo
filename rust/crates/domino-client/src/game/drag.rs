//! Captured mouse/touch dragging. Pointer motion updates only DOM transforms.
use crate::types::Tile;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{prelude::*, JsCast};

type Selection = Rc<dyn Fn(Tile)>;
type Placement = Rc<dyn Fn(Tile, &str)>;
struct Active {
    tile: Tile,
    id: i32,
    x: f64,
    y: f64,
    moved: bool,
    source: web_sys::Element,
    ghost: web_sys::Element,
}
struct Inner {
    active: RefCell<Option<Active>>,
    select: Selection,
    place: Placement,
    suppress_until: Cell<f64>,
}
type Listener = (&'static str, Closure<dyn FnMut(web_sys::Event)>);
pub struct DragController {
    inner: Rc<Inner>,
    listeners: Vec<Listener>,
}
impl DragController {
    pub fn new(select: impl Fn(Tile) + 'static, place: impl Fn(Tile, &str) + 'static) -> Self {
        let runtime = dioxus::dioxus_core::Runtime::current();
        let rt = runtime.clone();
        let inner = Rc::new(Inner {
            active: RefCell::new(None),
            select: Rc::new(move |t| rt.in_scope(dioxus::prelude::ScopeId::ROOT, || select(t))),
            place: Rc::new(move |t, e| {
                runtime.in_scope(dioxus::prelude::ScopeId::ROOT, || place(t, e))
            }),
            suppress_until: Cell::new(0.),
        });
        let mut listeners = Vec::new();
        for name in [
            "pointermove",
            "pointerup",
            "pointercancel",
            "lostpointercapture",
            "keydown",
            "visibilitychange",
        ] {
            let inner = inner.clone();
            let callback =
                Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
                    if name == "keydown" {
                        if event
                            .dyn_ref::<web_sys::KeyboardEvent>()
                            .is_some_and(|e| e.key() == "Escape")
                        {
                            inner.cancel();
                        }
                        return;
                    }
                    if name == "visibilitychange" {
                        if crate::browser::document().hidden() {
                            inner.cancel();
                        }
                        return;
                    }
                    let Some(pointer) = event.dyn_ref::<web_sys::PointerEvent>() else {
                        return;
                    };
                    if name == "pointermove" {
                        inner.motion(pointer);
                    } else if name == "pointerup" {
                        inner.finish(pointer);
                    } else {
                        inner.cancel();
                    }
                });
            let _ = crate::browser::document()
                .add_event_listener_with_callback(name, callback.as_ref().unchecked_ref());
            listeners.push((name, callback));
        }
        Self { inner, listeners }
    }
    pub fn begin(&self, tile: Tile, id: i32, x: f64, y: f64, source_id: &str) {
        self.inner.cancel();
        let document = crate::browser::document();
        let Some(source) = document.get_element_by_id(source_id) else {
            return;
        };
        let Some(face) = source.first_element_child() else {
            return;
        };
        let Ok(ghost) = face.clone_node_with_deep(true) else {
            return;
        };
        let Ok(ghost) = ghost.dyn_into::<web_sys::Element>() else {
            return;
        };
        let _ = ghost.set_attribute("class", "ivory-tile drag-ghost");
        let _ = ghost.set_attribute("aria-hidden", "true");
        let _ = ghost.set_attribute("style", "visibility:hidden");
        if let Some(body) = document.body() {
            let _ = body.append_child(&ghost);
        }
        let _ = source.set_pointer_capture(id);
        self.inner.active.replace(Some(Active {
            tile,
            id,
            x,
            y,
            moved: false,
            source,
            ghost,
        }));
        (self.inner.select)(tile);
    }
    pub fn cancel(&self) {
        self.inner.cancel();
    }
    pub fn allow_click(&self) -> bool {
        crate::browser::now_ms() > self.inner.suppress_until.get()
    }
}
impl Inner {
    fn motion(&self, event: &web_sys::PointerEvent) {
        let mut active = self.active.borrow_mut();
        let Some(a) = active.as_mut() else { return };
        if event.pointer_id() != a.id {
            return;
        }
        let (x, y) = (event.client_x() as f64, event.client_y() as f64);
        if !a.moved && (x - a.x).hypot(y - a.y) < 5. {
            return;
        }
        event.prevent_default();
        a.moved = true;
        let _ = a.source.class_list().add_1("drag-source");
        let _ = a.ghost.set_attribute(
            "style",
            &format!(
                "transform:translate3d({}px,{}px,0) rotate(-5deg);visibility:visible",
                x - 25.,
                y - 54.
            ),
        );
        let target = hit_end(x, y);
        for end in ["left", "right"] {
            if let Some(el) = crate::browser::document().get_element_by_id(&format!("drop-{end}")) {
                let _ = el
                    .class_list()
                    .toggle_with_force("drag-over", target.as_deref() == Some(end));
            }
        }
    }
    fn finish(&self, event: &web_sys::PointerEvent) {
        if self
            .active
            .borrow()
            .as_ref()
            .is_none_or(|a| a.id != event.pointer_id())
        {
            return;
        }
        let Some(a) = self.active.borrow_mut().take() else {
            return;
        };
        self.suppress_until.set(crate::browser::now_ms() + 400.);
        let end = if a.moved {
            hit_end(event.client_x() as f64, event.client_y() as f64)
        } else {
            None
        };
        if let Some(end) = end {
            cleanup(&a);
            (self.place)(a.tile, &end);
        } else if a.moved {
            return_to_rack(a);
        } else {
            cleanup(&a);
        }
    }
    fn cancel(&self) {
        let active = self.active.borrow_mut().take();
        if let Some(a) = active {
            if a.moved {
                self.suppress_until.set(crate::browser::now_ms() + 400.);
            }
            if a.moved && !crate::browser::document().hidden() {
                return_to_rack(a);
            } else {
                cleanup(&a);
            }
        }
    }
}
fn return_to_rack(a: Active) {
    let r = a
        .source
        .first_element_child()
        .unwrap_or_else(|| a.source.clone())
        .get_bounding_client_rect();
    // Read the current transform before switching to the return transition.
    let _ = a.ghost.get_bounding_client_rect();
    let _ = a.ghost.class_list().add_1("drag-return");
    let _ = a.ghost.set_attribute(
        "style",
        &format!(
            "transform:translate3d({}px,{}px,0) rotate(0deg) scale({},{});visibility:visible",
            r.x() + r.width() / 2. - 25.,
            r.y() + r.height() / 2. - 50.,
            r.width() / 50.,
            r.height() / 100.
        ),
    );
    let _ = a.source.release_pointer_capture(a.id);
    let _ = a.source.class_list().remove_1("drag-source");
    let _ = a.source.class_list().add_1("tile-returning");
    clear_targets();
    crate::game::timers::set_timeout(190, move || {
        // Do not clear drop markers or capture from a newer drag.
        a.ghost.remove();
        let _ = a.source.class_list().remove_1("tile-returning");
    });
}
fn clear_targets() {
    for end in ["left", "right"] {
        if let Some(el) = crate::browser::document().get_element_by_id(&format!("drop-{end}")) {
            let _ = el.class_list().remove_1("drag-over");
        }
    }
}
fn cleanup(a: &Active) {
    a.ghost.remove();
    let _ = a.source.class_list().remove_1("drag-source");
    let _ = a.source.release_pointer_capture(a.id);
    clear_targets();
}
fn hit_end(x: f64, y: f64) -> Option<String> {
    let doc = crate::browser::document();
    if let Some(el) = doc
        .element_from_point(x as f32, y as f32)
        .and_then(|el| el.closest("[data-drop-end]").ok().flatten())
    {
        if el.get_attribute("data-enabled").as_deref() == Some("true") {
            return el.get_attribute("data-drop-end");
        }
    }
    ["left", "right"]
        .into_iter()
        .filter_map(|end| {
            let el = doc.get_element_by_id(&format!("drop-{end}"))?;
            if el.get_attribute("data-enabled").as_deref() != Some("true") {
                return None;
            }
            let r = el.get_bounding_client_rect();
            let d = ((r.x() + r.width() / 2.) - x).hypot((r.y() + r.height() / 2.) - y);
            (d < r.width().max(r.height()) / 2. + 14.).then_some((end, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(end, _)| end.into())
}
impl Drop for DragController {
    fn drop(&mut self) {
        self.cancel();
        for (name, callback) in &self.listeners {
            let _ = crate::browser::document()
                .remove_event_listener_with_callback(name, callback.as_ref().unchecked_ref());
        }
    }
}
