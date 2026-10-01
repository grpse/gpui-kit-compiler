//! Generic worker state and component binding primitives.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
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
}

impl Binding {
    pub fn parameter(name: &'static str, direction: Direction) -> Self {
        Self {
            name,
            direction,
            data_key: None,
            getter: None,
            setter: None,
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
        })
    }
    pub fn get(&self, snapshot: &Snapshot) -> Option<Value> {
        self.getter.as_ref().and_then(|getter| getter(snapshot))
    }
    pub fn set(&self, engine: &Engine, value: Value) {
        if let Some(setter) = &self.setter {
            setter(engine, value);
        }
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
}

#[derive(Clone)]
pub struct Engine {
    sender: Sender<Message>,
    shared: Arc<Mutex<Snapshot>>,
    updates: Arc<Mutex<Receiver<Snapshot>>>,
}

impl Engine {
    pub fn start(
        defaults: HashMap<String, Value>,
        calculate: fn(&HashMap<String, Value>, u64) -> Snapshot,
        on_change: Option<fn(&mut HashMap<String, Value>, &str, &Value)>,
    ) -> Self {
        let shared = Arc::new(Mutex::new(calculate(&defaults, 0)));
        let (sender, receiver) = mpsc::channel();
        let (updates_tx, updates) = mpsc::channel();
        let shared_worker = Arc::clone(&shared);
        thread::Builder::new()
            .name("rsc-calculation-worker".into())
            .spawn(move || {
                let mut values = defaults.clone();
                let mut reset_epoch = 0;
                while let Ok(message) = receiver.recv() {
                    match message {
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
                    let next = calculate(&values, reset_epoch);
                    *shared_worker.lock().expect("snapshot lock poisoned") = next.clone();
                    let _ = updates_tx.send(next);
                }
            })
            .expect("failed to start calculation worker");
        Self {
            sender,
            shared,
            updates: Arc::new(Mutex::new(updates)),
        }
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
        Arc::clone(&self.updates)
    }
    pub fn snapshot(&self) -> Snapshot {
        self.shared.lock().expect("snapshot lock poisoned").clone()
    }
}
