#![cfg(feature = "compiler")]

use quote::ToTokens;
use std::fs;

const COMPONENTS: &str = include_str!("fixtures/gpui-kit/src/components.rsx");
const PAGES: &str = include_str!("fixtures/gpui-kit/pages.txt");

#[test]
fn every_reviewed_component_page_has_a_native_rsx_recipe() {
    let converted = gpui_rsc::convert_source(COMPONENTS).unwrap();
    let parsed = syn::parse_file(&converted).unwrap();
    let recipes = parsed
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            let urls = function
                .attrs
                .iter()
                .filter_map(|attribute| {
                    if !attribute.path().is_ident("doc") {
                        return None;
                    }
                    let syn::Meta::NameValue(value) = &attribute.meta else {
                        return None;
                    };
                    let syn::Expr::Lit(value) = &value.value else {
                        return None;
                    };
                    let syn::Lit::Str(value) = &value.lit else {
                        return None;
                    };
                    Some(value.value().trim().to_owned())
                })
                .collect::<Vec<_>>();
            Some((function.sig.ident.to_string(), urls))
        })
        .collect::<Vec<_>>();

    let pages = PAGES.lines().collect::<Vec<_>>();
    assert_eq!(
        pages.len(),
        77,
        "snapshot of the component index reviewed on 2026-10-04"
    );
    assert_eq!(recipes.len(), pages.len());
    for url in pages {
        let name = url
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap()
            .replace('-', "_");
        assert_eq!(
            recipes
                .iter()
                .filter(|(recipe, urls)| recipe == &name && urls.iter().any(|doc| doc == url))
                .count(),
            1,
            "{url} needs exactly one documented RSX recipe"
        );
    }
    // Check semantic lowering where the docs need multiple method arguments,
    // static factories, nested callback markup, and tuple-valued IDs.
    let rust = parsed.to_token_stream().to_string();
    for expected in [
        ". item (\"Name\" , \"GPUI Kit\" , 1)",
        ". child (left , Some (px (240.)))",
        ". focus_trap (\"trap\" , handle)",
        "c :: tag :: Tag :: primary ()",
        ". id ((\"message\" , index))",
        ". build (window , cx)",
    ] {
        assert!(rust.contains(expected), "missing {expected}");
    }
}

#[test]
fn project_compilation_keeps_gpui_kit_recipes_as_regular_rust() {
    let directory = std::env::temp_dir().join(format!("rsc-kit-project-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("components.rsx");
    let output = directory.join("components.rs");
    fs::write(&input, COMPONENTS).unwrap();
    gpui_rsc::compile_file(&input, &output).unwrap();
    assert_eq!(
        fs::read_to_string(&output).unwrap(),
        gpui_rsc::convert_source(COMPONENTS).unwrap()
    );
    fs::remove_dir_all(directory).unwrap();
}
