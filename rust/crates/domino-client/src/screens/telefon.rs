use crate::{
    game::tile::BoardTile,
    i18n::{t, tf},
    placement::PendingPlacement,
    store::SESSION,
    types::SGameState,
};
use dioxus::prelude::*;
use domino_core::End;

#[component]
pub fn TelefonBoard(
    state: SGameState,
    selected: Option<[u8; 2]>,
    busy: bool,
    placement: Option<PendingPlacement>,
    onplace: EventHandler<String>,
) -> Element {
    let dimensions = use_signal(|| (700., 300.));
    let observer = use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(None::<BoardResize>)));
    let cleanup = observer.clone();
    use_drop(move || {
        cleanup.borrow_mut().take();
    });
    let attach = move |_| {
        use wasm_bindgen::JsCast;
        let Some(el) = crate::browser::document().get_element_by_id("telefon-board") else {
            return;
        };
        let r = el.get_bounding_client_rect();
        let mut measured = dimensions;
        measured.set((r.width().max(1.), r.height().max(1.)));
        let runtime = dioxus::dioxus_core::Runtime::current();
        let element = el.clone();
        let callback = wasm_bindgen::closure::Closure::<
            dyn FnMut(js_sys::Array, web_sys::ResizeObserver),
        >::new(move |_, _| {
            let r = element.get_bounding_client_rect();
            let next = (r.width().max(1.), r.height().max(1.));
            runtime.in_scope(dioxus::prelude::ScopeId::ROOT, || {
                let mut dimensions = dimensions;
                if dimensions() != next {
                    dimensions.set(next);
                }
            });
        });
        if let Ok(resize) = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) {
            resize.observe(&el);
            *observer.borrow_mut() = Some(BoardResize {
                observer: resize,
                _callback: callback,
            });
        }
    };
    let window = crate::browser::window();
    let w = window
        .inner_width()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(1000.);
    let h = window
        .inner_height()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(600.);
    let unit = if w < 600. && h > w { 32. } else { 34. };
    let layout = crate::telefon_layout::fit(&state.telefon, dimensions().0, dimensions().1, unit);
    let slots = &layout.slots;
    let me = SESSION.read().user_id.clone();
    let moves = crate::board::placements(&state, &me);
    rsx! {div {id:"telefon-board",class:"snake-board",onmounted:attach,aria_label:t("5-lik · Telefon"),
            svg {class:"snake-svg",style:"position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);width:min(100%,{layout.width*unit}px);height:min(100%,{layout.height*unit}px)",view_box:"0 0 {layout.width} {layout.height}",preserve_aspect_ratio:"xMidYMid meet",
                defs {
                    linearGradient {id:"ivory-face",x1:"0",y1:"0",x2:"1",y2:"1",stop {offset:"0%",stop_color:"#fffef6"}stop {offset:"100%",stop_color:"#ded0ae"}}
                    radialGradient {id:"pip-ink",stop {offset:"0%",stop_color:"#233a2c"}stop {offset:"100%",stop_color:"#102319"}}
                }
                for (i,node) in state.telefon.iter().enumerate() {
                    {let slot=slots[i];let (w,h)=slot.size();let tile=node.tile;let last=state.last_played_tile.is_some_and(|t|t==tile||t==[tile[1],tile[0]]);let finish=state.round_finishing_tile.is_some_and(|t|t==tile||t==[tile[1],tile[0]]);rsx!{
                        g {key:"telefon-tile-{i}",class:if finish{"placed-domino round-finisher"}else if last{"placed-domino last-played"}else{"placed-domino"},transform:"translate({slot.x-w/2.} {slot.y-h/2.})",
                            BoardTile {value:tile,vertical:slot.vertical,reversed:slot.direction==2||slot.direction==3}
                            if last||finish {rect {x:"0",y:"0",width:"{w}",height:"{h}",rx:".12",fill:"none",stroke:if finish{"#35baad"}else{"#f8d384"},stroke_width:".07",pointer_events:"none"}}
                            title {"{tile[0]}–{tile[1]}"}
                        }
                    }}
                }
                if state.status=="playing" {
                    for &(end,target) in &layout.targets {
                        {let (x,y)=(target.x,target.y);let pip=match end {End::Telefon(n,p)=>state.telefon[n as usize].tile[if p==1 {1}else{0}],_=>0};let token=end.token();let click_token=token.clone();let key_token=token.clone();let enabled=!busy&&selected.is_some_and(|tile|moves.iter().any(|(t,e)|*t==tile&&*e==end));let submitting=placement.is_some_and(|p|p.end==end);let label=if slots.is_empty(){t("START")}else{pip.to_string()};let bonus=selected.and_then(|tile|domino_core::telefon::placement_points(&state.telefon,tile,end)).filter(|p|*p>0);let hint=tf("Play at end {pip}",&[("pip",pip.to_string())]);rsx!{
                            g {key:"{token}",class:if enabled{"snake-drop drop-enabled"}else{"snake-drop"},"data-drop-end":token,"data-enabled":enabled.to_string(),role:"button",tabindex:if enabled{"0"}else{"-1"},"aria-disabled":(!enabled).to_string(),"aria-label":hint,transform:"translate({x-0.7} {y-0.7})",
                                onclick:move |_|if enabled{onplace.call(click_token.clone())},onkeydown:move |e|if enabled&&(e.key()==Key::Enter||e.key()==Key::Character(" ".into())){e.prevent_default();onplace.call(key_token.clone());},
                                rect {width:"1.4",height:"1.4",rx:".2",fill:"#112f25",stroke:"currentColor",stroke_width:".05",stroke_dasharray:".12 .08"}
                                text {x:".7",y:".86",text_anchor:"middle",font_size:".4",fill:"currentColor",if submitting {"…"}else{"{label}"}}
                                if enabled {if let Some(points)=bonus {text {x:".7",y:"1.2",text_anchor:"middle",font_size:".25",fill:"#f8d384","+{points}"}}}
                            }
                        }}
                    }
                }
            }
    }}
}

struct BoardResize {
    observer: web_sys::ResizeObserver,
    _callback: wasm_bindgen::closure::Closure<dyn FnMut(js_sys::Array, web_sys::ResizeObserver)>,
}
impl Drop for BoardResize {
    fn drop(&mut self) {
        self.observer.disconnect();
    }
}
