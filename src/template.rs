use crate::runtime;
pub type RenderFn = fn(
    &runtime::view::HtmlView,
    &runtime::binding::Element,
    f32,
    &[runtime::StyleRule],
    &[runtime::StyleRule],
    &mut gpui::Context<runtime::view::HtmlView>,
) -> gpui::AnyElement;

#[derive(Clone)]
pub enum TemplateNode {
    Text(String),
    Element(TemplateElement),
}

#[derive(Clone)]
pub struct TemplateElement {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<TemplateNode>,
    pub style: runtime::InlineStyle,
    pub mobile_style: runtime::InlineStyle,
    pub inline_style: runtime::InlineStyle,
    pub render: Option<RenderFn>,
}

impl TemplateElement {
    pub fn new(
        tag: impl Into<String>,
        attrs: Vec<(String, String)>,
        children: Vec<TemplateNode>,
    ) -> Self {
        Self {
            tag: tag.into(),
            attrs,
            children,
            style: runtime::InlineStyle::new(),
            mobile_style: runtime::InlineStyle::new(),
            inline_style: runtime::InlineStyle::new(),
            render: None,
        }
    }

    pub fn with_style(mut self, style: runtime::InlineStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_mobile_style(mut self, style: runtime::InlineStyle) -> Self {
        self.mobile_style = style;
        self
    }

    pub fn with_inline_style(mut self, style: runtime::InlineStyle) -> Self {
        self.inline_style = style;
        self
    }

    pub fn with_render(mut self, render: RenderFn) -> Self {
        self.render = Some(render);
        self
    }

    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }
}

pub trait CompiledComponent {
    fn template() -> TemplateElement;
}
