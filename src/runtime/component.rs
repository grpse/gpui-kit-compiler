use crate::{
    TemplateElement,
    runtime::{Binding, Snapshot, Value},
};
use std::collections::HashMap;

#[derive(Clone)]
pub struct Definition {
    pub name: &'static str,
    pub title: &'static str,
    pub imports: Vec<Definition>,
    pub bindings: Vec<Binding>,
    pub template: TemplateElement,
    pub calculate: Option<fn(&HashMap<String, Value>, u64) -> Snapshot>,
    pub on_change: Option<fn(&mut HashMap<String, Value>, &str, &Value)>,
}

#[macro_export]
macro_rules! component {
    (name: $name:literal, imports: [$($import:expr),* $(,)?], bindings: [$($binding:expr),* $(,)?], calculate: $calculate:expr, on_change: $on_change:expr $(,)?) => {
        $crate::runtime::Definition {
            name: $name,
            title: title(),
            imports: vec![$($import),*],
            bindings: vec![$($binding),*],
            template: template(),
            calculate: Some($calculate),
            on_change: Some($on_change),
        }
    };
    (name: $name:literal, imports: [$($import:expr),* $(,)?], bindings: [$($binding:expr),* $(,)?], calculate: $calculate:expr $(,)?) => {
        $crate::runtime::Definition {
            name: $name,
            title: title(),
            imports: vec![$($import),*],
            bindings: vec![$($binding),*],
            template: template(),
            calculate: Some($calculate),
            on_change: None,
        }
    };
    (name: $name:literal, imports: [$($import:expr),* $(,)?], bindings: [$($binding:expr),* $(,)?] $(,)?) => {
        $crate::runtime::Definition {
            name: $name,
            title: title(),
            imports: vec![$($import),*],
            bindings: vec![$($binding),*],
            template: template(),
            calculate: None,
            on_change: None,
        }
    };
}
