use super::binding::InlineStyle;
use crate::{
    TemplateElement,
    runtime::{Binding, Snapshot, Value},
};
use std::collections::HashMap;

pub type Style = InlineStyle;
pub type StyleSheet = for<'a> fn(&StyleContext<'a>) -> Vec<StyleRule>;
pub type OutputFormatter = fn(&str, &Value) -> String;

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
    pub style_sheets: Vec<StyleSheet>,
    pub responsive_style_sheets: Vec<StyleSheet>,
    pub output_formatter: Option<OutputFormatter>,
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
        self.style_sheets.push(style_sheet);
        self
    }

    /// Add compiler-generated style rules evaluated against the current viewport.
    pub fn with_responsive_style_sheet(mut self, style_sheet: StyleSheet) -> Self {
        self.responsive_style_sheets.push(style_sheet);
        self
    }

    pub fn with_output_formatter(mut self, formatter: OutputFormatter) -> Self {
        self.output_formatter = Some(formatter);
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
            style_sheets: Vec::new(),
            responsive_style_sheets: Vec::new(),
            output_formatter: None,
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
            style_sheets: Vec::new(),
            responsive_style_sheets: Vec::new(),
            output_formatter: None,
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
            style_sheets: Vec::new(),
            responsive_style_sheets: Vec::new(),
            output_formatter: None,
        }
    };
}
