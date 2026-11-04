//! Web timer + sleep primitives used by the client (turn ticker, guest
//! countdown, delayed reaction removal, connect backoff, copy-flip reset).

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Resolve after `ms` milliseconds (single-threaded web runtime; no Send).
pub async fn sleep_ms(ms: u64) {
    let ms = ms.min(i32::MAX as u64);
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let _ = crate::browser::window()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms as i32);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

type Job = Rc<RefCell<Option<Box<dyn FnOnce()>>>>;

type PendingTimer = (i32, Closure<dyn FnMut()>, Job);

thread_local! {
    static PENDING: RefCell<Vec<PendingTimer>> = const { RefCell::new(Vec::new()) };
}

/// Fire-once `setTimeout(fn, delay_ms)`.
pub fn set_timeout(delay_ms: u32, f: impl FnOnce() + 'static) {
    let runtime = dioxus::dioxus_core::Runtime::current();
    let scope = dioxus::prelude::ScopeId::ROOT;
    let job: Job = Rc::new(RefCell::new(Some(Box::new(move || {
        runtime.in_scope(scope, f)
    }))));
    let job_for_closure = job.clone();
    let closure = Closure::<dyn FnMut()>::new(move || {
        // Release the borrow before the task schedules another timer. At the
        // pruning threshold a nested schedule inspects these same job cells.
        let task = job_for_closure.borrow_mut().take();
        if let Some(task) = task {
            task();
        }
    });
    let id = crate::browser::window().set_timeout_with_callback_and_timeout_and_arguments_0(
        closure.as_ref().unchecked_ref(),
        delay_ms as i32,
    );
    if let Ok(id) = id {
        PENDING.with_borrow_mut(|list| {
            list.push((id, closure, job));
            // Prune already-fired entries so the vec cannot grow forever.
            if list.len() > 128 {
                list.retain(|(_, _, job)| job.borrow().is_some());
            }
        });
    }
}
