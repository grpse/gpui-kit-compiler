mod generated {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/rsc-build/generated.rs"
    ));
}

use gpui_rsc::runtime::{self, StartupConfig};

fn main() {
    runtime::run_with_config(
        generated::app::App(),
        StartupConfig {
            resizable: true,
            ..StartupConfig::default()
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_rsc::runtime::{
        Engine, Value,
        binding::{Control, Element, Node, Page},
    };
    use std::time::Duration;

    fn control<'a>(element: &'a Element, id: &str) -> Option<&'a Control> {
        if let Some(component) = &element.component {
            if let Some(control) = component
                .page
                .controls
                .iter()
                .find(|control| control.id().ends_with(id))
            {
                return Some(control);
            }
        }
        element.children.iter().find_map(|child| match child {
            Node::Element(child) => control(child, id),
            Node::Text(_) => None,
        })
    }

    fn setup() -> (Page, Engine) {
        let definition = generated::app::App();
        let page = runtime::binding::compile(&definition).expect("coffee bindings compile");
        let engine = Engine::start(
            page.defaults.clone(),
            definition.calculate.unwrap(),
            definition.on_change,
        );
        page.bind_signals(&engine);
        (page, engine)
    }

    #[test]
    fn recipe_controls_update_prediction_and_profile_controls_update_recipe() {
        let (page, engine) = setup();
        let updates = engine.subscribe();
        let original = engine.snapshot();
        let dose = control(&page.root, "/dose").expect("dose control");
        dose.binding().set(&engine, Value::Number(30.0));
        let after_dose = updates
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert_eq!(after_dose.get("dose"), Some(&Value::Number(30.0)));
        assert_eq!(
            after_dose.get("ratio"),
            Some(&Value::Text("1 : 10.0".into()))
        );
        assert_ne!(after_dose.get("ratio"), original.get("ratio"));
        assert_eq!(after_dose.get("target_acidity"), after_dose.get("acidity"));
        assert_eq!(
            page.signals
                .iter()
                .find(|(key, _)| key == "target_acidity")
                .unwrap()
                .1
                .get(),
            after_dose.get("acidity").unwrap().clone()
        );

        let target = control(&page.root, "/flavor-acidity").expect("acidity target control");
        target.binding().set(&engine, Value::Number(90.0));
        let after_target = updates
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert_ne!(after_target.get("acidity"), after_dose.get("acidity"));
        assert_eq!(
            after_target.get("target_acidity"),
            after_target.get("acidity")
        );
        assert_eq!(
            page.signals
                .iter()
                .find(|(key, _)| key == "dose")
                .unwrap()
                .1
                .get(),
            after_target.get("dose").unwrap().clone()
        );
    }
}
