//! Desktop application lifecycle notifications.
//!
//! Native aborts and process kills cannot run callbacks in the dying process.
//! The `gpui-rsc` launcher reports those failures from the parent process.

use std::{
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{Arc, Mutex},
};

use gpui::{App, Application, Entity, Render, Window, WindowId, WindowOptions};

use super::Engine;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LifecycleEvent {
    WillStart,
    DidStart,
    WillOpen,
    DidOpen { window_id: WindowId },
    OpenFailed { reason: String },
    WillClose { window_id: WindowId },
    DidClose { window_id: WindowId },
    WillQuit,
    DidQuit,
    Activated { window_id: WindowId },
    Deactivated { window_id: WindowId },
    Reopen,
    OpenUrls { urls: Vec<String> },
    WillRestart,
    WillSleep,
    DidWake,
    UnexpectedQuit { reason: String },
}

pub type LifecycleHandler = fn(LifecycleEvent);

#[derive(Default)]
struct State {
    quitting: bool,
    finished: bool,
    unexpected: bool,
    // The boolean records whether WillClose has already been delivered.
    windows: HashMap<WindowId, bool>,
}

/// One handler for app and window events. Handlers must be short and synchronous.
/// Notifications do not cancel opening, closing, or quitting.
#[derive(Clone, Default)]
pub struct Lifecycle {
    handler: Option<Arc<dyn Fn(LifecycleEvent) + Send + Sync>>,
    state: Arc<Mutex<State>>,
}

impl Lifecycle {
    pub fn new(handler: impl Fn(LifecycleEvent) + Send + Sync + 'static) -> Self {
        Self {
            handler: Some(Arc::new(handler)),
            ..Self::default()
        }
    }

    fn emit(&self, event: LifecycleEvent) {
        if let Some(handler) = &self.handler {
            // GPUI invokes some observers from native callbacks. A handler panic
            // must not unwind across that boundary or recursively call itself.
            if let Err(panic) = catch_unwind(AssertUnwindSafe(|| handler(event))) {
                eprintln!(
                    "lifecycle handler panicked: {}",
                    panic_reason(panic.as_ref())
                );
            }
        } else if let LifecycleEvent::UnexpectedQuit { reason } = event {
            eprintln!("application quit unexpectedly: {reason}");
        }
    }

    pub(crate) fn guard<R>(&self, run: impl FnOnce() -> R) -> R {
        match catch_unwind(AssertUnwindSafe(run)) {
            Ok(result) => result,
            Err(panic) => {
                self.unexpected_quit(panic_reason(panic.as_ref()));
                resume_unwind(panic);
            }
        }
    }

    fn unexpected_quit(&self, reason: String) {
        let notify = {
            let mut state = self.state.lock().expect("lifecycle lock poisoned");
            !std::mem::replace(&mut state.unexpected, true)
        };
        if notify {
            self.emit(LifecycleEvent::UnexpectedQuit { reason });
        }
    }

    fn window_opened(&self, window_id: WindowId) {
        self.state
            .lock()
            .expect("lifecycle lock poisoned")
            .windows
            .insert(window_id, false);
        self.emit(LifecycleEvent::DidOpen { window_id });
    }

    /// Call from a custom close-request handler if it replaces the default one.
    /// Call only after deciding to allow the close.
    pub fn window_will_close(&self, window_id: WindowId) {
        let notify = {
            let mut state = self.state.lock().expect("lifecycle lock poisoned");
            state
                .windows
                .get_mut(&window_id)
                .is_some_and(|closing| !std::mem::replace(closing, true))
        };
        if notify {
            self.emit(LifecycleEvent::WillClose { window_id });
        }
    }

    fn window_closed(&self, window_id: WindowId) {
        let closing = self
            .state
            .lock()
            .expect("lifecycle lock poisoned")
            .windows
            .remove(&window_id);
        if let Some(closing) = closing {
            // Programmatic removal can bypass the native close-request hook.
            if !closing {
                self.emit(LifecycleEvent::WillClose { window_id });
            }
            self.emit(LifecycleEvent::DidClose { window_id });
        }
    }

    fn will_quit(&self) {
        let windows = {
            let mut state = self.state.lock().expect("lifecycle lock poisoned");
            if state.quitting {
                return;
            }
            state.quitting = true;
            state
                .windows
                .iter_mut()
                .filter_map(|(id, closing)| (!std::mem::replace(closing, true)).then_some(*id))
                .collect::<Vec<_>>()
        };
        self.emit(LifecycleEvent::WillQuit);
        for window_id in windows {
            self.emit(LifecycleEvent::WillClose { window_id });
        }
    }

    fn did_quit(&self) {
        let windows = {
            let mut state = self.state.lock().expect("lifecycle lock poisoned");
            if state.finished || state.unexpected {
                return;
            }
            state.finished = true;
            state.windows.drain().map(|(id, _)| id).collect::<Vec<_>>()
        };
        for window_id in windows {
            self.emit(LifecycleEvent::DidClose { window_id });
        }
        self.emit(LifecycleEvent::DidQuit);
    }

    /// Run a native GPUI app with lifecycle observers and last-window quit.
    /// Use `open_window` in the launch callback to receive window events.
    pub fn run(self, application: Application, launch: impl FnOnce(&mut App, &Self) + 'static) {
        self.run_with_engine(application, None, launch);
    }

    pub(crate) fn run_with_engine(
        self,
        application: Application,
        engine: Option<Engine>,
        launch: impl FnOnce(&mut App, &Self) + 'static,
    ) {
        self.guard(|| {
            self.emit(LifecycleEvent::WillStart);
            let reopened = self.clone();
            application.on_reopen(move |_| reopened.emit(LifecycleEvent::Reopen));
            let urls = self.clone();
            application
                .on_open_urls(move |opened| urls.emit(LifecycleEvent::OpenUrls { urls: opened }));
            let lifecycle = self.clone();
            application.run(move |cx| {
                lifecycle.install(cx, engine);
                lifecycle.guard(|| launch(cx, &lifecycle));
                lifecycle.emit(LifecycleEvent::DidStart);
            });
            let quitting = self.state.lock().expect("lifecycle lock poisoned").quitting;
            if !quitting {
                self.unexpected_quit("the UI event loop returned without a quit request".into());
            }
        });
    }

    fn install(&self, cx: &App, engine: Option<Engine>) {
        let quit = self.clone();
        cx.on_app_quit(move |_| {
            quit.will_quit();
            if let Some(engine) = &engine {
                engine.shutdown();
            }
            let engine = engine.clone();
            let quit = quit.clone();
            async move {
                if let Some(engine) = engine {
                    engine.wait_for_shutdown().await;
                }
                quit.did_quit();
            }
        })
        .detach();
        let closed = self.clone();
        cx.on_window_closed(move |cx, window_id| {
            closed.window_closed(window_id);
            let quitting = closed
                .state
                .lock()
                .expect("lifecycle lock poisoned")
                .quitting;
            if !quitting && cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let restart = self.clone();
        cx.on_app_restart(move |_| restart.emit(LifecycleEvent::WillRestart))
            .detach();
        let sleep = self.clone();
        cx.on_system_sleep(move |_| sleep.emit(LifecycleEvent::WillSleep))
            .detach();
        let wake = self.clone();
        cx.on_system_wake(move |_| wake.emit(LifecycleEvent::DidWake))
            .detach();
    }

    /// Open a GPUI Kit window and report its open/close lifecycle.
    /// A custom `on_window_should_close` installed by `build` takes precedence;
    /// it should call `window_will_close` when allowing the window to close.
    pub fn open_window<V: Render>(
        &self,
        options: WindowOptions,
        cx: &mut App,
        build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    ) -> gpui::Result<(gpui::AnyWindowHandle, Entity<V>)> {
        self.emit(LifecycleEvent::WillOpen);
        let closing = self.clone();
        let activation = self.clone();
        let result = gpui_kit::open_window(options, cx, move |window, cx| {
            window.on_window_should_close(cx, move |window, cx| {
                closing.window_will_close(window.window_handle().window_id());
                #[cfg(target_os = "macos")]
                if cx.windows().len() == 1 {
                    // Keep the responder/view hierarchy alive until AppKit
                    // terminates. Closing it first can expose Touch Bar observers
                    // to AccessKit's restored NSView class during teardown.
                    cx.quit();
                    return false;
                }
                #[cfg(not(target_os = "macos"))]
                let _ = cx;
                true
            });
            let view = build(window, cx);
            view.update(cx, |_, cx| {
                cx.observe_window_activation(window, move |_, window, _| {
                    let window_id = window.window_handle().window_id();
                    activation.emit(if window.is_window_active() {
                        LifecycleEvent::Activated { window_id }
                    } else {
                        LifecycleEvent::Deactivated { window_id }
                    });
                })
                .detach();
            });
            view
        });
        match &result {
            Ok((window, _)) => self.window_opened(window.window_id()),
            Err(error) => self.emit(LifecycleEvent::OpenFailed {
                reason: error.to_string(),
            }),
        }
        result
    }

    /// Close a window programmatically, emitting WillClose before removal.
    pub fn close_window(&self, window: &mut Window, cx: &mut App) {
        self.window_will_close(window.window_handle().window_id());
        #[cfg(target_os = "macos")]
        if cx.windows().len() == 1 {
            cx.quit();
            return;
        }
        #[cfg(not(target_os = "macos"))]
        let _ = cx;
        window.remove_window();
    }
}

fn panic_reason(panic: &(dyn std::any::Any + Send)) -> String {
    if let Some(reason) = panic.downcast_ref::<String>() {
        reason.clone()
    } else if let Some(reason) = panic.downcast_ref::<&str>() {
        (*reason).to_owned()
    } else {
        "Rust panic with a non-text payload".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recorder() -> (Lifecycle, Arc<Mutex<Vec<LifecycleEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let recorded = events.clone();
        let lifecycle = Lifecycle::new(move |event| recorded.lock().unwrap().push(event));
        (lifecycle, events)
    }

    #[test]
    fn window_close_and_quit_events_are_ordered_and_delivered_once() {
        let (lifecycle, events) = recorder();
        let window_id = WindowId::from(1);
        lifecycle.window_opened(window_id);
        lifecycle.window_will_close(window_id);
        lifecycle.window_will_close(window_id);
        lifecycle.window_closed(window_id);
        lifecycle.window_closed(window_id);
        lifecycle.will_quit();
        lifecycle.will_quit();
        lifecycle.did_quit();
        lifecycle.did_quit();
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                LifecycleEvent::DidOpen { window_id },
                LifecycleEvent::WillClose { window_id },
                LifecycleEvent::DidClose { window_id },
                LifecycleEvent::WillQuit,
                LifecycleEvent::DidQuit,
            ]
        );
    }

    #[test]
    fn app_quit_closes_remaining_windows_before_did_quit() {
        let (lifecycle, events) = recorder();
        let first = WindowId::from(1);
        let second = WindowId::from(2);
        lifecycle.window_opened(first);
        lifecycle.window_opened(second);
        events.lock().unwrap().clear();
        lifecycle.will_quit();
        // A native close notification may precede the app's final cleanup.
        lifecycle.window_closed(first);
        lifecycle.did_quit();
        let events = events.lock().unwrap();
        assert_eq!(events.first(), Some(&LifecycleEvent::WillQuit));
        assert_eq!(events.last(), Some(&LifecycleEvent::DidQuit));
        for window_id in [first, second] {
            let will = events
                .iter()
                .position(|event| *event == LifecycleEvent::WillClose { window_id })
                .unwrap();
            let did = events
                .iter()
                .position(|event| *event == LifecycleEvent::DidClose { window_id })
                .unwrap();
            assert!(will < did);
            assert_eq!(
                events
                    .iter()
                    .filter(|event| **event == LifecycleEvent::DidClose { window_id })
                    .count(),
                1
            );
        }
    }

    #[test]
    fn unexpected_quit_preserves_the_panic_and_reports_it_once() {
        let (lifecycle, events) = recorder();
        let result = catch_unwind(AssertUnwindSafe(|| {
            lifecycle.guard(|| lifecycle.guard(|| panic!("UI failed")));
        }));
        assert_eq!(panic_reason(result.unwrap_err().as_ref()), "UI failed");
        lifecycle.did_quit();
        assert_eq!(
            *events.lock().unwrap(),
            vec![LifecycleEvent::UnexpectedQuit {
                reason: "UI failed".into()
            }]
        );
    }

    #[test]
    fn worker_panic_reports_to_the_same_handler() {
        let (lifecycle, events) = recorder();
        let worker = std::thread::spawn(move || lifecycle.guard(|| panic!("calculation failed")));
        assert!(worker.join().is_err());
        assert_eq!(
            *events.lock().unwrap(),
            vec![LifecycleEvent::UnexpectedQuit {
                reason: "calculation failed".into()
            }]
        );
    }

    #[test]
    fn handler_panics_do_not_escape_a_native_callback_boundary() {
        let lifecycle = Lifecycle::new(|_| panic!("handler failed"));
        lifecycle.emit(LifecycleEvent::WillStart);
        lifecycle.will_quit();
        lifecycle.did_quit();
        assert!(lifecycle.state.lock().unwrap().finished);
    }

    #[test]
    fn handlers_can_reenter_lifecycle_without_holding_its_state_lock() {
        let reference = Arc::new(Mutex::new(None::<Lifecycle>));
        let events = Arc::new(Mutex::new(Vec::new()));
        let handler_reference = reference.clone();
        let handler_events = events.clone();
        let lifecycle = Lifecycle::new(move |event| {
            handler_events.lock().unwrap().push(event.clone());
            if let LifecycleEvent::DidOpen { window_id } = event {
                handler_reference
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .window_will_close(window_id);
            }
        });
        *reference.lock().unwrap() = Some(lifecycle.clone());
        let window_id = WindowId::from(1);
        lifecycle.window_opened(window_id);
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                LifecycleEvent::DidOpen { window_id },
                LifecycleEvent::WillClose { window_id }
            ]
        );
        // Break the test handler's own reference cycle.
        reference.lock().unwrap().take();
    }
}
