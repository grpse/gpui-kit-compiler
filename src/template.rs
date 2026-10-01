use crate::runtime;
pub type RenderFn = fn(
    &runtime::view::HtmlView,
    &runtime::binding::Element,
    f32,
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
            render: None,
        }
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
