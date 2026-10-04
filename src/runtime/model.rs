//! Generic worker state and component binding primitives.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, Weak,
        mpsc::{self, Receiver, Sender},
    },
    thread,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f32),
    Text(String),
    Arguments(Vec<Value>),
}

impl Value {
    pub fn number(&self) -> Option<f32> {
        if let Self::Number(v) = self {
            Some(*v)
        } else {
            None
        }
    }
    pub fn text(&self) -> String {
        match self {
            Self::Number(v) if v.fract().abs() < 0.01 => format!("{v:.0}"),
            Self::Number(v) => format!("{v:.1}"),
            Self::Text(v) => v.clone(),
            Self::Arguments(v) => format!("{} arguments", v.len()),
        }
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Self::Number(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Number(value as f32)
    }
}

macro_rules! number_value_from {
    ($($number:ty),* $(,)?) => {
        $(impl From<$number> for Value {
            fn from(value: $number) -> Self {
                Self::Number(value as f32)
            }
        })*
    };
}

number_value_from!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Text(value.to_string())
    }
}

struct SignalState {
    value: Mutex<Value>,
    subscribers: Mutex<Vec<Sender<()>>>,
    engines: Mutex<HashMap<(usize, String), Engine>>,
}

/// Shared component state that notifies every view using it when its value changes.
#[derive(Clone)]
pub struct Signal {
    state: Arc<SignalState>,
}

impl std::fmt::Debug for Signal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("Signal").field(&self.get()).finish()
    }
}

impl Signal {
    pub fn new(value: impl Into<Value>) -> Self {
        Self {
            state: Arc::new(SignalState {
                value: Mutex::new(value.into()),
                subscribers: Mutex::new(Vec::new()),
                engines: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn get(&self) -> Value {
        self.state
            .value
            .lock()
            .expect("signal lock poisoned")
            .clone()
    }

    pub fn set(&self, value: impl Into<Value>) {
        let value = value.into();
        if self.replace_value(value.clone()) {
            self.notify_subscribers();
            let engines = self
                .state
                .engines
                .lock()
                .expect("signal engine bindings lock poisoned");
            for ((_, key), engine) in engines.iter() {
                engine.set(key.clone(), value.clone());
            }
        }
    }

    fn replace_value(&self, value: Value) -> bool {
        let mut current = self.state.value.lock().expect("signal lock poisoned");
        if *current == value {
            false
        } else {
            *current = value;
            true
        }
    }

    fn notify_subscribers(&self) {
        self.state
            .subscribers
            .lock()
            .expect("signal subscribers lock poisoned")
            .retain(|subscriber| subscriber.send(()).is_ok());
    }

    fn sync_from_engine(signal: &Weak<SignalState>, value: Value) {
        if let Some(state) = signal.upgrade() {
            let signal = Self { state };
            if signal.replace_value(value) {
                signal.notify_subscribers();
            }
        }
    }

    pub(crate) fn bind_engine(&self, key: String, engine: Engine) {
        let identity = Arc::as_ptr(&engine.shared) as usize;
        engine.bind_signal(key.clone(), self);
        self.state
            .engines
            .lock()
            .expect("signal engine bindings lock poisoned")
            .insert((identity, key), engine);
    }

    pub(crate) fn subscribe(&self) -> Arc<Mutex<Receiver<()>>> {
        let (sender, receiver) = mpsc::channel();
        self.state
            .subscribers
            .lock()
            .expect("signal subscribers lock poisoned")
            .push(sender);
        Arc::new(Mutex::new(receiver))
    }
}

/// Create a reactive value for use in a component template.
pub fn signal(value: impl Into<Value>) -> Signal {
    Signal::new(value)
}

/// Binding direction is relative to the UI component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    In,
    Out,
    InOut,
}

impl Direction {
    pub fn reads(self) -> bool {
        matches!(self, Self::In | Self::InOut)
    }
    pub fn writes(self) -> bool {
        matches!(self, Self::Out | Self::InOut)
    }
}

type Getter = Arc<dyn Fn(&Snapshot) -> Option<Value> + Send + Sync>;
type Setter = Arc<dyn Fn(&Engine, Value) + Send + Sync>;

#[derive(Clone)]
pub struct Binding {
    pub name: &'static str,
    pub direction: Direction,
    pub data_key: Option<&'static str>,
    getter: Option<Getter>,
    setter: Option<Setter>,
    signal: Option<Signal>,
}

impl Binding {
    pub fn parameter(name: &'static str, direction: Direction) -> Self {
        Self {
            name,
            direction,
            data_key: None,
            getter: None,
            setter: None,
            signal: None,
        }
    }
    pub fn read_key(name: &'static str, key: &'static str) -> Self {
        Self::read_with(name, move |snapshot| snapshot.get(key).cloned())
    }
    pub fn read_with(
        name: &'static str,
        getter: impl Fn(&Snapshot) -> Option<Value> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name,
            direction: Direction::In,
            data_key: None,
            getter: Some(Arc::new(getter)),
            setter: None,
            signal: None,
        }
    }
    pub fn write_with(
        name: &'static str,
        setter: impl Fn(&Engine, Value) + Send + Sync + 'static,
    ) -> Self {
        Self {
            name,
            direction: Direction::Out,
            data_key: None,
            getter: None,
            setter: Some(Arc::new(setter)),
            signal: None,
        }
    }
    pub fn two_way_key(name: &'static str, key: &'static str) -> Self {
        Self::two_way_with(
            name,
            Some(key),
            move |snapshot| snapshot.get(key).cloned(),
            move |engine, value| engine.set(key, value),
        )
    }
    pub fn two_way_with(
        name: &'static str,
        data_key: Option<&'static str>,
        getter: impl Fn(&Snapshot) -> Option<Value> + Send + Sync + 'static,
        setter: impl Fn(&Engine, Value) + Send + Sync + 'static,
    ) -> Self {
        Self {
            name,
            direction: Direction::InOut,
            data_key,
            getter: Some(Arc::new(getter)),
            setter: Some(Arc::new(setter)),
            signal: None,
        }
    }
    pub fn signal(name: &'static str, signal: Signal) -> Self {
        let getter_signal = signal.clone();
        let setter_signal = signal.clone();
        Self {
            name,
            direction: Direction::InOut,
            data_key: None,
            getter: Some(Arc::new(move |_| Some(getter_signal.get()))),
            setter: Some(Arc::new(move |_, value| setter_signal.set(value))),
            signal: Some(signal),
        }
    }
    pub(crate) fn signal_at(name: &'static str, key: String, signal: Signal) -> Self {
        let setter_signal = signal;
        Self {
            name,
            direction: Direction::InOut,
            data_key: None,
            getter: Some(Arc::new(move |snapshot| snapshot.get(&key).cloned())),
            setter: Some(Arc::new(move |_, value| setter_signal.set(value))),
            // The engine publishes these values with a completed snapshot. Subscribing to
            // the signal itself would render a new control value with old calculations.
            signal: None,
        }
    }
    pub fn constant(name: &'static str, value: Value) -> Self {
        Self::read_with(name, move |_| Some(value.clone()))
    }
    pub fn alias(&self, name: &'static str, direction: Direction) -> Result<Self, String> {
        if direction.reads() && self.getter.is_none() {
            return Err(format!("{name} needs a readable source"));
        }
        if direction.writes() && self.setter.is_none() {
            return Err(format!("{name} needs a writable source"));
        }
        Ok(Self {
            name,
            direction,
            data_key: self.data_key,
            getter: if direction.reads() {
                self.getter.clone()
            } else {
                None
            },
            setter: if direction.writes() {
                self.setter.clone()
            } else {
                None
            },
            signal: self.signal.clone(),
        })
    }
    pub fn get(&self, snapshot: &Snapshot) -> Option<Value> {
        self.getter.as_ref().and_then(|getter| getter(snapshot))
    }
    pub fn set(&self, engine: &Engine, value: Value) {
        if let Some(setter) = &self.setter {
            let setter = Arc::clone(setter);
            let target = engine.clone();
            engine.dispatch(move || setter(&target, value));
        }
    }
    pub(crate) fn subscribe_signal(&self) -> Option<Arc<Mutex<Receiver<()>>>> {
        self.signal.as_ref().map(Signal::subscribe)
    }
}

#[macro_export]
macro_rules! in_binding {
    ($name:literal => $key:literal) => {
        $crate::runtime::Binding::read_key($name, $key)
    };
    ($name:literal, $getter:expr) => {
        $crate::runtime::Binding::read_with($name, $getter)
    };
}
#[macro_export]
macro_rules! out_binding {
    ($name:literal => $command:literal) => {
        $crate::runtime::Binding::write_with($name, |engine, _| engine.invoke($command))
    };
    ($name:literal, $setter:expr) => {
        $crate::runtime::Binding::write_with($name, $setter)
    };
}
#[macro_export]
macro_rules! in_out_binding {
    ($name:literal => $key:literal) => {
        $crate::runtime::Binding::two_way_key($name, $key)
    };
    ($name:literal, data_key = $key:expr, get = $getter:expr, set = $setter:expr) => {
        $crate::runtime::Binding::two_way_with($name, $key, $getter, $setter)
    };
}
#[macro_export]
macro_rules! in_param {
    ($name:literal) => {
        $crate::runtime::Binding::parameter($name, $crate::runtime::Direction::In)
    };
}
#[macro_export]
macro_rules! out_param {
    ($name:literal) => {
        $crate::runtime::Binding::parameter($name, $crate::runtime::Direction::Out)
    };
}
#[macro_export]
macro_rules! in_out_param {
    ($name:literal) => {
        $crate::runtime::Binding::parameter($name, $crate::runtime::Direction::InOut)
    };
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub values: HashMap<String, Value>,
    pub reset_epoch: u64,
}
impl Snapshot {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }
}

enum Message {
    Set(String, Value),
    Invoke(String),
    Reset,
    Dispatch(Box<dyn FnOnce() + Send>),
    Shutdown,
}

#[derive(Clone)]
pub struct Engine {
    sender: Sender<Message>,
    shared: Arc<Mutex<Snapshot>>,
    subscribers: Arc<Mutex<Vec<Sender<Snapshot>>>>,
    signals: Arc<Mutex<HashMap<String, Weak<SignalState>>>>,
}

impl Engine {
    pub fn start(
        defaults: HashMap<String, Value>,
        calculate: fn(&HashMap<String, Value>, u64) -> Snapshot,
        on_change: Option<fn(&mut HashMap<String, Value>, &str, &Value)>,
    ) -> Self {
        let (engine, run) = Self::prepare(defaults, calculate, on_change);
        thread::Builder::new()
            .name("rsc-calculation-worker".into())
            .spawn(run)
            .expect("failed to start calculation worker");
        engine
    }

    /// Build the dispatcher without spawning it; the caller owns its event loop.
    pub(crate) fn prepare(
        defaults: HashMap<String, Value>,
        calculate: fn(&HashMap<String, Value>, u64) -> Snapshot,
        on_change: Option<fn(&mut HashMap<String, Value>, &str, &Value)>,
    ) -> (Self, impl FnOnce() + Send + 'static) {
        let shared = Arc::new(Mutex::new(calculate(&defaults, 0)));
        let (sender, receiver) = mpsc::channel();
        let subscribers = Arc::new(Mutex::new(Vec::<Sender<Snapshot>>::new()));
        let signals = Arc::new(Mutex::new(HashMap::<String, Weak<SignalState>>::new()));
        let shared_worker = Arc::clone(&shared);
        let subscribers_worker = Arc::clone(&subscribers);
        let signals_worker = Arc::clone(&signals);
        let run = move || {
            let mut values = defaults.clone();
            let mut reset_epoch = 0;
            while let Ok(message) = receiver.recv() {
                match message {
                    Message::Shutdown => break,
                    Message::Dispatch(callback) => {
                        callback();
                        continue;
                    }
                    Message::Set(key, value) => {
                        values.insert(key.clone(), value.clone());
                        if let Some(on_change) = on_change {
                            on_change(&mut values, &key, &value);
                        }
                    }
                    Message::Invoke(key) => {
                        if let Some(on_change) = on_change {
                            on_change(&mut values, &key, &Value::Arguments(Vec::new()));
                        }
                    }
                    Message::Reset => {
                        values = defaults.clone();
                        reset_epoch += 1;
                    }
                }
                let signal_updates = signals_worker
                    .lock()
                    .expect("engine signal registry lock poisoned")
                    .iter()
                    .filter_map(|(key, signal)| {
                        values
                            .get(key)
                            .cloned()
                            .map(|value| (signal.clone(), value))
                    })
                    .collect::<Vec<_>>();
                for (signal, value) in signal_updates {
                    Signal::sync_from_engine(&signal, value);
                }
                let next = calculate(&values, reset_epoch);
                *shared_worker.lock().expect("snapshot lock poisoned") = next.clone();
                subscribers_worker
                    .lock()
                    .expect("snapshot subscribers lock poisoned")
                    .retain(|subscriber| subscriber.send(next.clone()).is_ok());
            }
        };
        (
            Self {
                sender,
                shared,
                subscribers,
                signals,
            },
            run,
        )
    }

    /// Queue work on the state dispatcher. Callbacks run serially outside state locks.
    /// Keep callbacks short, or spawn a worker for long independent jobs.
    pub fn dispatch(&self, callback: impl FnOnce() + Send + 'static) {
        let _ = self.sender.send(Message::Dispatch(Box::new(callback)));
    }

    pub(crate) fn shutdown(&self) {
        let _ = self.sender.send(Message::Shutdown);
    }

    fn bind_signal(&self, key: String, signal: &Signal) {
        self.signals
            .lock()
            .expect("engine signal registry lock poisoned")
            .insert(key, Arc::downgrade(&signal.state));
    }
    pub fn set(&self, key: impl Into<String>, value: Value) {
        let _ = self.sender.send(Message::Set(key.into(), value));
    }
    pub fn invoke(&self, key: impl Into<String>) {
        let _ = self.sender.send(Message::Invoke(key.into()));
    }
    pub fn reset(&self) {
        let _ = self.sender.send(Message::Reset);
    }
    pub fn subscribe(&self) -> Arc<Mutex<Receiver<Snapshot>>> {
        let (sender, receiver) = mpsc::channel();
        self.subscribers
            .lock()
            .expect("snapshot subscribers lock poisoned")
            .push(sender);
        Arc::new(Mutex::new(receiver))
    }
    pub fn snapshot(&self) -> Snapshot {
        self.shared.lock().expect("snapshot lock poisoned").clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_binding_dispatches_to_the_owner_thread_and_publishes_state() {
        use std::time::Duration;

        fn calculate(values: &HashMap<String, Value>, reset_epoch: u64) -> Snapshot {
            Snapshot {
                values: values.clone(),
                reset_epoch,
            }
        }
        let owner = thread::current().id();
        let (engine, run) = Engine::prepare(HashMap::new(), calculate, None);
        let updates = engine.subscribe();
        let (called, received) = mpsc::channel();
        let binding = Binding::write_with("job", move |engine, value| {
            // Acquiring the snapshot here also verifies callbacks hold no state lock.
            let _ = engine.snapshot();
            called.send(thread::current().id()).unwrap();
            engine.set("result", value);
        });
        let ui_engine = engine.clone();
        let ui = thread::spawn(move || {
            let ui_thread = thread::current().id();
            binding.set(&ui_engine, Value::from("done"));
            assert_eq!(
                received.recv_timeout(Duration::from_secs(2)).unwrap(),
                owner
            );
            assert_ne!(ui_thread, owner);
            let snapshot = updates
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(2))
                .unwrap();
            assert_eq!(snapshot.get("result"), Some(&Value::from("done")));
            ui_engine.shutdown();
        });
        run();
        ui.join().unwrap();
    }

    #[test]
    fn dispatch_preserves_fifo_order_and_stops_with_live_handles() {
        let (engine, run) = Engine::prepare(
            HashMap::new(),
            |values, reset_epoch| Snapshot {
                values: values.clone(),
                reset_epoch,
            },
            None,
        );
        let (sender, receiver) = mpsc::channel();
        for index in 0..3 {
            let sender = sender.clone();
            engine.dispatch(move || sender.send(index).unwrap());
        }
        engine.shutdown();
        run();
        assert_eq!(receiver.try_iter().collect::<Vec<_>>(), vec![0, 1, 2]);
    }

    #[test]
    fn compiled_signal_binding_reads_the_matching_snapshot() {
        let signal = Signal::new(20.0);
        let binding = Binding::signal_at("dose", "dose".into(), signal.clone());
        let snapshot = Snapshot {
            values: HashMap::from([("dose".into(), Value::Number(20.0))]),
            reset_epoch: 0,
        };

        signal.set(30.0);
        assert_eq!(binding.get(&snapshot), Some(Value::Number(20.0)));
        assert_eq!(signal.get(), Value::Number(30.0));
    }
}
