use super::ui::{BackButton, Icon};
use crate::i18n::{t, tf};
use crate::{net, store::*};
use dioxus::prelude::*;

fn swipe_offset(offset: f64) {
    if let Some(panel) = crate::browser::document().get_element_by_id("ranking-swipe-panel") {
        let _ = panel.set_attribute("style", &format!("--swipe-offset:{offset}px"));
        let _ = panel
            .class_list()
            .toggle_with_force("swipe-dragging", offset != 0.);
    }
}

#[component]
pub fn Leaders() -> Element {
    let mut period = use_signal(|| "day");
    let mut retry = use_signal(|| 0u64);
    let gesture =
        use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(crate::swipe::Swipe::default())));
    let mut direction = use_signal(|| 0i32);
    let start = gesture.clone();
    let motion = gesture.clone();
    let finish = gesture.clone();
    let cancel = gesture.clone();
    let cleanup = gesture.clone();
    use_drop(move || {
        cleanup.borrow_mut().cancel();
    });
    // Resources cancel superseded work. The response key also hides retained data
    // during the render before a reactive restart, including retry of the same period.
    let response = use_resource(move || {
        let key = (period(), retry());
        async move { (key, net::leaderboard(key.0).await) }
    });
    let response = response.read();
    let current = response
        .as_ref()
        .filter(|(key, _)| *key == (period(), retry()));
    let mut rows = Vec::<crate::types::LeaderEntry>::new();
    let mut failed = false;
    let mut tracking_started = None;
    if let Some((_, result)) = current {
        match result {
            Ok(value) => match serde_json::from_value(value["leaderboard"].clone()) {
                Ok(parsed) => {
                    rows = parsed;
                    tracking_started = value["trackingStartedAt"]
                        .as_str()
                        .map(|date| date.split('T').next().unwrap_or(date).to_owned());
                }
                Err(_) => failed = true,
            },
            Err(_) => failed = true,
        }
    }
    let loading = current.is_none();
    rsx! { section { class: "leaders-page menu-page", BackButton {}
           div { class: "page-heading",  h1 { {t("Leaderboard")} } p { class: "muted", {t("Good instincts. Great games. Meet the players setting the pace.")} } }
           div { class: "ranking-periods", role: "group", aria_label: t("Leaderboard period"),
               for (key, label) in [("day", "Today"), ("week", "This week"), ("month", "This month"), ("all", "All time")] {
                   button { class: if period() == key { "glass-capsule selected" } else { "glass-capsule" }, aria_pressed: period() == key, onclick: move |_| { if period() != key { direction.set(if ["day","week","month","all"].iter().position(|p|*p==key)>["day","week","month","all"].iter().position(|p|*p==period()){1}else{-1}); period.set(key); retry += 1; } }, {t(label)} }
               }
           }
           p { class: "ranking-calendar-note", {t("Daily, weekly and monthly rankings use UTC. Weeks start Monday.")} }
           div {id:"ranking-swipe",class:"ranking-swipe",
               onpointerdown:move |e|{
                   if !e.is_primary(){start.borrow_mut().cancel();swipe_offset(0.);return;}
                   if !matches!(e.pointer_type().as_str(),"touch"|"pen"){return;}
                   if let Some(el)=crate::browser::document().get_element_by_id("ranking-swipe") {
                       let p=e.client_coordinates();start.borrow_mut().begin(e.pointer_id(),p.x,p.y,el.get_bounding_client_rect().width());
                       let _=el.set_pointer_capture(e.pointer_id());
                   }
               },
               onpointermove:move |e|{if !matches!(e.pointer_type().as_str(),"touch"|"pen"){return;}let p=e.client_coordinates();swipe_offset(motion.borrow_mut().motion(e.pointer_id(),p.x,p.y));},
               onpointerup:move |e|{
                   let p=e.client_coordinates();let step=finish.borrow_mut().finish(e.pointer_id(),p.x,p.y);swipe_offset(0.);
                   if let Some(step)=step {
                       let periods=["day","week","month","all"];
                       let index=periods.iter().position(|p|*p==period()).unwrap_or(3) as i32;
                       let next=index+step;
                       if (0..4).contains(&next){direction.set(step);period.set(periods[next as usize]);retry+=1;}
                   }
               },
               onpointercancel:move |_|{cancel.borrow_mut().cancel();swipe_offset(0.);},
               onlostpointercapture:move |_|{gesture.borrow_mut().cancel();swipe_offset(0.);},
           section { key:"{period()}", id:"ranking-swipe-panel", class:format!("ranking-panel glass {}",if direction()>0{"page-from-right"}else if direction()<0{"page-from-left"}else{""}), aria_busy: loading,
    div { class: "ranking-head", span { {t("PLAYER")} } span { {t("WINS")} } span { {t("WIN RATE")} } span { {t("GAMES")} } }
               if loading { p { class: "muted ranking-loading", role: "status", {t("Bringing the leaderboard to the table…")} } }
               if failed { div { class: "ranking-error", role: "alert", p { {t("Could not load this leaderboard. Try again.")} } button { class: "glass-capsule", onclick: move |_| retry += 1, {t("Retry")} } } }
               for (i, row) in rows.iter().enumerate() { div { class: if row.user_id == SESSION.read().user_id { "ranking-row your-rank" } else { "ranking-row" }, div { class: "rank-player", span { class: if i<3 { "rank-position gold" } else { "rank-position" }, if i==0 { Icon { name: "trophy" } } else { "{i+1}" } } div { strong { "{row.username}" } if row.user_id == SESSION.read().user_id { small { {t("You")} } } small { class:"rank-detail", {tf("{row.losses} losses · {row.avg_score} avg points", &[("row.losses", format!("{}", (row.losses))), ("row.avg_score", format!("{}", (row.avg_score)))])} } } } strong { "{row.wins}" } span { "{row.win_rate}%" } span { "{row.games_played}" } } }
               if !loading && !failed && rows.is_empty() { div { class: "history-empty", div { class: "empty-orbit glass-circle", Icon { name: "trophy" } } h3 { {t("The next champion could be you.")} } p { {t("No completed matches in this period yet.")} } } }
           }
           }
           if period() != "all" {
               p { class: "ranking-calendar-note", if let Some(started) = tracking_started { {tf("Dated rankings include matches tracked since {date} (UTC). Earlier matches remain in All time.", &[("date", started)])} } else { {t("Dated rankings include matches tracked since launch. Earlier matches remain in All time.")} } }
           }
           small { class: "ranking-note", {t("Ranked by wins, then win rate. Guest and bot players are not listed.")} }
       } }
}
