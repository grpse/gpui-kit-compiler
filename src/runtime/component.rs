use super::binding::InlineStyle;
use crate::{
    TemplateElement,
    runtime::{Binding, Snapshot, Value},
};
use std::collections::HashMap;

pub type Style = InlineStyle;
pub type StyleSheet = for<'a> fn(&StyleContext<'a>) -> Vec<StyleRule>;

pub struct StyleContext<'a> {
    pub viewport_width: f32,
    pub snapshot: &'a Snapshot,
}

pub struct StyleRule {
    pub selector: String,
    pub style: Style,
}

impl StyleRule {
    pub fn new(selector: impl Into<String>, style: Style) -> Self {
        Self {
            selector: selector.into(),
            style,
        }
    }
}

#[derive(Clone)]
pub struct Definition {
    pub name: &'static str,
    pub title: &'static str,
    pub imports: Vec<Definition>,
    pub bindings: Vec<Binding>,
    pub template: TemplateElement,
    pub calculate: Option<fn(&HashMap<String, Value>, u64) -> Snapshot>,
    pub on_change: Option<fn(&mut HashMap<String, Value>, &str, &Value)>,
    /// Viewport widths at or below this value use `mobile-style` declarations.
    pub mobile_breakpoint: Option<f32>,
    pub style_sheet: Option<StyleSheet>,
}

impl Definition {
    /// Configure the viewport width (in logical pixels) at which mobile styles apply.
    pub fn mobile_breakpoint(mut self, width: f32) -> Self {
        assert!(
            width.is_finite() && width > 0.0,
            "mobile breakpoint must be a positive finite width"
        );
        self.mobile_breakpoint = Some(width);
        self
    }

    /// Add Rust-defined styles that are recomputed from the current viewport and app state.
    pub fn with_style_sheet(mut self, style_sheet: StyleSheet) -> Self {
        self.style_sheet = Some(style_sheet);
        self
    }
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
            mobile_breakpoint: None,
            style_sheet: None,
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
            mobile_breakpoint: None,
            style_sheet: None,
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
            mobile_breakpoint: None,
            style_sheet: None,
        }
    };
}
