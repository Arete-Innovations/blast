use std::future::Future;

use leptos::prelude::*;

use crate::meltdown::MeltDown;
use crate::structs::leptos::{LiveResource, PolledResource, ReactiveSignal};

pub fn use_resource_effect<T, F, Fut>(loader: F) -> ReactiveSignal<T>
where
    T: 'static + Send + Sync,
    F: Fn() -> Fut + 'static + Clone,
    Fut: Future<Output = Result<T, MeltDown>> + 'static,
{
    let signal: ReactiveSignal<T> = RwSignal::new(None);
    #[cfg(target_arch = "wasm32")]
    {
        Effect::new(move |_| {
            let loader = loader.clone();
            leptos::task::spawn_local(async move {
                let result = loader().await;
                signal.set(Some(result));
            });
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _loader: F = loader;
    }
    signal
}

pub fn use_polled_resource<T, F, Fut>(loader: F, interval_ms: u32) -> PolledResource<T>
where
    T: 'static + Send + Sync,
    F: Fn() -> Fut + 'static + Clone,
    Fut: Future<Output = Result<T, MeltDown>> + 'static,
{
    let signal: ReactiveSignal<T> = RwSignal::new(None);
    let refetch_trigger: RwSignal<u32> = RwSignal::new(0);

    #[cfg(target_arch = "wasm32")]
    {
        polled::wire(loader, signal, refetch_trigger, interval_ms);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _loader: F = loader;
        let _interval_ms: u32 = interval_ms;
    }

    PolledResource { signal, refetch_trigger }
}

pub fn use_live_resource<T, F, Fut>(loader: F, topic: &'static str) -> LiveResource<T>
where
    T: 'static + Send + Sync,
    F: Fn() -> Fut + 'static + Clone,
    Fut: Future<Output = Result<T, MeltDown>> + 'static,
{
    let signal: ReactiveSignal<T> = RwSignal::new(None);

    #[cfg(target_arch = "wasm32")]
    {
        live::wire(loader, signal, topic);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _loader: F = loader;
        let _topic: &'static str = topic;
    }

    LiveResource { signal }
}

#[cfg(target_arch = "wasm32")]
mod polled {
    use std::cell::RefCell;
    use std::future::Future;
    use std::rc::Rc;

    use gloo_timers::callback::Interval;
    use leptos::prelude::*;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;
    use web_sys::VisibilityState;

    use crate::meltdown::{MeltDown, MeltType};
    use crate::structs::leptos::{PolledState, ReactiveSignal, WasmDrop};

    pub(super) fn wire<T, F, Fut>(loader: F, signal: ReactiveSignal<T>, refetch_trigger: RwSignal<u32>, interval_ms: u32)
    where
        T: 'static + Send + Sync,
        F: Fn() -> Fut + 'static + Clone,
        Fut: Future<Output = Result<T, MeltDown>> + 'static,
    {
        Effect::new({
            let loader = loader.clone();
            move |_| {
                refetch_trigger.get();
                let loader = loader.clone();
                leptos::task::spawn_local(async move {
                    let result = loader().await;
                    signal.set(Some(result));
                });
            }
        });

        let state: Rc<RefCell<PolledState>> = Rc::new(RefCell::new(PolledState::default()));

        let kick = {
            let loader = loader.clone();
            move || {
                let loader = loader.clone();
                leptos::task::spawn_local(async move {
                    let result = loader().await;
                    signal.set(Some(result));
                });
            }
        };

        if !document_hidden() {
            start_interval(&state, interval_ms, kick.clone());
        }

        match web_sys::window() {
            Some(window) => match window.document() {
                Some(document) => {
                    let state_for_cb = Rc::clone(&state);
                    let kick_for_cb = kick.clone();
                    let listener = Closure::<dyn FnMut()>::new(move || {
                        if document_hidden() {
                            state_for_cb.borrow_mut().interval = None;
                        } else if state_for_cb.borrow().interval.is_none() {
                            start_interval(&state_for_cb, interval_ms, kick_for_cb.clone());
                        }
                    });
                    match document.add_event_listener_with_callback("visibilitychange", listener.as_ref().unchecked_ref()) {
                        Ok(()) => (),
                        Err(err) => MeltDown::new(MeltType::Unexpected("polled_visibility_listener".to_string()), format!("add_event_listener failed: {:?}", err)).log(),
                    }
                    state.borrow_mut().visibility_listener = Some(listener);
                }
                None => (),
            },
            None => (),
        }

        let drop_box: WasmDrop<Rc<RefCell<PolledState>>> = WasmDrop::new(state);
        on_cleanup(move || {
            drop_box.consume();
        });
    }

    fn start_interval(state: &Rc<RefCell<PolledState>>, interval_ms: u32, kick: impl Fn() + 'static + Clone) {
        let interval = Interval::new(interval_ms, move || kick());
        state.borrow_mut().interval = Some(interval);
    }

    fn document_hidden() -> bool {
        let Some(window) = web_sys::window() else {
            return false;
        };
        let Some(document) = window.document() else {
            return false;
        };
        matches!(document.visibility_state(), VisibilityState::Hidden)
    }
}

#[cfg(target_arch = "wasm32")]
mod live {
    use std::cell::Cell;
    use std::future::Future;
    use std::rc::Rc;

    use futures_util::stream::StreamExt;
    use futures_util::SinkExt;
    use gloo_net::websocket::futures::WebSocket;
    use gloo_net::websocket::Message;
    use gloo_timers::future::TimeoutFuture;
    use leptos::prelude::*;

    use crate::meltdown::{MeltDown, MeltType};
    use crate::structs::leptos::{ReactiveSignal, WasmCleanup};

    const BACKOFF_INITIAL_MS: u32 = 250;
    const BACKOFF_CAP_MS: u32 = 8_000;

    pub(super) fn wire<T, F, Fut>(loader: F, signal: ReactiveSignal<T>, topic: &'static str)
    where
        T: 'static + Send + Sync,
        F: Fn() -> Fut + 'static + Clone,
        Fut: Future<Output = Result<T, MeltDown>> + 'static,
    {
        {
            let loader = loader.clone();
            leptos::task::spawn_local(async move {
                let result = loader().await;
                signal.set(Some(result));
            });
        }

        let cancelled: Rc<Cell<bool>> = Rc::new(Cell::new(false));

        leptos::task::spawn_local({
            let loader = loader.clone();
            let cancelled = Rc::clone(&cancelled);
            async move {
                let mut backoff_ms = BACKOFF_INITIAL_MS;
                while !cancelled.get() {
                    match connect_and_pump(topic, loader.clone(), signal, Rc::clone(&cancelled)).await {
                        Ok(()) => {
                            if cancelled.get() {
                                break;
                            }
                            backoff_ms = BACKOFF_INITIAL_MS;
                        }
                        Err(err) => {
                            err.log();
                            TimeoutFuture::new(backoff_ms).await;
                            backoff_ms = (backoff_ms.saturating_mul(2)).min(BACKOFF_CAP_MS);
                        }
                    }
                }
            }
        });

        let cancelled_for_cleanup = Rc::clone(&cancelled);
        let drop_box: WasmCleanup = WasmCleanup::new(Box::new(move || cancelled_for_cleanup.set(true)));
        on_cleanup(move || drop_box.consume());
    }

    async fn connect_and_pump<T, F, Fut>(topic: &'static str, loader: F, signal: ReactiveSignal<T>, cancelled: Rc<Cell<bool>>) -> Result<(), MeltDown>
    where
        T: 'static + Send + Sync,
        F: Fn() -> Fut + 'static + Clone,
        Fut: Future<Output = Result<T, MeltDown>> + 'static,
    {
        let url = ws_url();
        let mut ws = match WebSocket::open(&url) {
            Ok(socket) => socket,
            Err(err) => return Err(MeltDown::new(MeltType::Unexpected("ws_open".to_string()), format!("WebSocket::open({}) failed: {:?}", url, err))),
        };

        let subscribe = serde_json::json!({"op": "subscribe", "topic": topic}).to_string();
        match ws.send(Message::Text(subscribe)).await {
            Ok(()) => (),
            Err(err) => return Err(MeltDown::new(MeltType::Unexpected("ws_subscribe".to_string()), format!("subscribe send failed: {}", err))),
        }

        loop {
            let frame = match ws.next().await {
                Some(f) => f,
                None => return Ok(()),
            };
            if cancelled.get() {
                match ws.close(None, None) {
                    Ok(()) => (),
                    Err(err) => MeltDown::new(MeltType::Unexpected("ws_close".to_string()), format!("close failed: {:?}", err)).log(),
                }
                return Ok(());
            }
            match frame {
                Ok(Message::Text(_)) => {
                    let loader = loader.clone();
                    leptos::task::spawn_local(async move {
                        let result = loader().await;
                        signal.set(Some(result));
                    });
                }
                Ok(Message::Bytes(_)) => {
                    let loader = loader.clone();
                    leptos::task::spawn_local(async move {
                        let result = loader().await;
                        signal.set(Some(result));
                    });
                }
                Err(err) => {
                    return Err(MeltDown::new(MeltType::Unexpected("ws_frame".to_string()), format!("ws frame error: {}", err)));
                }
            }
        }
    }

    fn ws_url() -> String {
        let Some(window) = web_sys::window() else {
            return "/ws".to_string();
        };
        let location = window.location();
        let host = match location.host() {
            Ok(h) => h,
            Err(err) => {
                MeltDown::new(MeltType::Unexpected("ws_url_host".to_string()), format!("location.host() failed: {:?}", err)).log();
                return "/ws".to_string();
            }
        };
        let scheme = match location.protocol() {
            Ok(proto) if proto == "https:" => "wss",
            Ok(proto) => {
                MeltDown::new(MeltType::Unexpected("ws_url_protocol_non_https".to_string()), format!("non-https protocol: {}", proto)).log();
                "ws"
            }
            Err(err) => {
                MeltDown::new(MeltType::Unexpected("ws_url_protocol".to_string()), format!("location.protocol() failed: {:?}", err)).log();
                "ws"
            }
        };
        if host.is_empty() {
            "/ws".to_string()
        } else {
            format!("{}://{}/ws", scheme, host)
        }
    }
}
