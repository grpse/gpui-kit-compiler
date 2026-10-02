use crate::{
    TemplateElement,
    runtime::{Binding, Snapshot, Value},
};
use std::collections::HashMap;

pub type Style = gpui::StyleRefinement;
pub type OutputFormatter = fn(&str, &Value) -> String;

/// Immutable values passed to a component render function for its readable parameters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComponentProps {
    values: HashMap<&'static str, Value>,
}

impl ComponentProps {
    pub(crate) fn from_values(values: HashMap<&'static str, Value>) -> Self {
        Self { values }
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }
}

/// A selectable value and the label shown in the select menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
    pub selected: bool,
}

impl SelectOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            selected: false,
        }
    }

    pub fn selected(mut self) -> Self {
        self.selected = true;
        self
    }
}

pub struct StyleContext<'a> {
    pub viewport_width: f32,
    pub props: &'a ComponentProps,
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
    pub output_formatter: Option<OutputFormatter>,
    pub select_options: HashMap<String, Vec<SelectOption>>,
    pub renderer: crate::ComponentRenderFn,
    pub view_inputs: &'static [&'static str],
}

impl Definition {
    pub fn new(
        name: &'static str,
        title: &'static str,
        template: TemplateElement,
        bindings: Vec<Binding>,
        renderer: crate::ComponentRenderFn,
    ) -> Self {
        Self {
            name,
            title,
            imports: Vec::new(),
            bindings,
            template,
            calculate: None,
            on_change: None,
            mobile_breakpoint: None,
            output_formatter: None,
            select_options: HashMap::new(),
            renderer,
            view_inputs: &[],
        }
    }

    /// Add child components inferred from uppercase JSX tags in the component template.
    pub fn with_imports(mut self, imports: impl IntoIterator<Item = Definition>) -> Self {
        for import in imports {
            if !self
                .imports
                .iter()
                .any(|existing| existing.name == import.name)
            {
                self.imports.push(import);
            }
        }
        self
    }

    /// Subscribe this component to immutable input values used by its own view.
    pub fn with_view_inputs(mut self, inputs: &'static [&'static str]) -> Self {
        self.view_inputs = inputs;
        self
    }

    /// Configure the viewport width (in logical pixels) at which mobile styles apply.
    pub fn mobile_breakpoint(mut self, width: f32) -> Self {
        assert!(
            width.is_finite() && width > 0.0,
            "mobile breakpoint must be a positive finite width"
        );
        self.mobile_breakpoint = Some(width);
        self
    }

    pub fn with_output_formatter(mut self, formatter: OutputFormatter) -> Self {
        self.output_formatter = Some(formatter);
        self
    }

    /// Use a template expression assembled in the component script.
    pub fn with_template(mut self, template: TemplateElement) -> Self {
        self.template = template;
        self
    }

    /// Provide select choices from a Rust iterator, mapping values to visible labels.
    pub fn with_select_options(
        mut self,
        id: impl Into<String>,
        options: impl IntoIterator<Item = SelectOption>,
    ) -> Self {
        self.select_options
            .insert(id.into(), options.into_iter().collect());
        self
    }
}

#[macro_export]
macro_rules! component {
    (name: $name:literal, bindings: [$($binding:expr),* $(,)?], calculate: $calculate:expr, on_change: $on_change:expr $(,)?) => {
        $crate::runtime::Definition {
            name: $name,
            title: title(),
            imports: Vec::new(),
            bindings: vec![$($binding),*],
            template: template(),
            calculate: Some($calculate),
            on_change: Some($on_change),
            mobile_breakpoint: None,
            output_formatter: None,
            select_options: std::collections::HashMap::new(),
            renderer: __rsc_render_component,
            view_inputs: __rsc_generated_view_inputs(),
        }
    };
    (name: $name:literal, bindings: [$($binding:expr),* $(,)?], calculate: $calculate:expr $(,)?) => {
        $crate::runtime::Definition {
            name: $name,
            title: title(),
            imports: Vec::new(),
            bindings: vec![$($binding),*],
            template: template(),
            calculate: Some($calculate),
            on_change: None,
            mobile_breakpoint: None,
            output_formatter: None,
            select_options: std::collections::HashMap::new(),
            renderer: __rsc_render_component,
            view_inputs: __rsc_generated_view_inputs(),
        }
    };
    (name: $name:literal, bindings: [$($binding:expr),* $(,)?] $(,)?) => {
        $crate::runtime::Definition {
            name: $name,
            title: title(),
            imports: Vec::new(),
            bindings: vec![$($binding),*],
            template: template(),
            calculate: None,
            on_change: None,
            mobile_breakpoint: None,
            output_formatter: None,
            select_options: std::collections::HashMap::new(),
            renderer: __rsc_render_component,
            view_inputs: __rsc_generated_view_inputs(),
        }
    };
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
            output_formatter: None,
            select_options: std::collections::HashMap::new(),
            renderer: __rsc_render_component,
            view_inputs: __rsc_generated_view_inputs(),
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
            output_formatter: None,
            select_options: std::collections::HashMap::new(),
            renderer: __rsc_render_component,
            view_inputs: __rsc_generated_view_inputs(),
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
            output_formatter: None,
            select_options: std::collections::HashMap::new(),
            renderer: __rsc_render_component,
            view_inputs: __rsc_generated_view_inputs(),
        }
    };
}
