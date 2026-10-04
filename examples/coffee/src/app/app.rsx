use gpui_rsc::runtime::{signal, Signal, Snapshot, StyleContext, Value};
use std::collections::HashMap;

use crate::generated::coffee_profile::CoffeeProfile;
use crate::generated::coffee_variables_form::CoffeeVariablesForm;
use crate::generated::extraction_previews::ExtractionPreviews;

pub fn title() -> &'static str { "Coffee / Lab" }

fn reset_recipe(
    method: &Signal,
    filter: &Signal,
    dose: &Signal,
    water: &Signal,
    grind: &Signal,
    temperature: &Signal,
    time: &Signal,
    pours: &Signal,
    stirs: &Signal,
    swirls: &Signal,
) {
    method.set("V60");
    filter.set("Paper");
    dose.set(20.0);
    water.set(300.0);
    grind.set(5.0);
    temperature.set(94.0);
    time.set(180.0);
    pours.set(3.0);
    stirs.set(0.0);
    swirls.set(1.0);
}

pub fn output_format(name: &str, value: &Value) -> String {
    match name {
        "score" => format!("{:.0}", value.number().unwrap_or(0.0)),
        "ratio" => value
            .number()
            .map(|ratio| format!("1 : {ratio:.1}"))
            .unwrap_or_else(|| value.text()),
        "duration" => {
            let seconds = value.number().unwrap_or(0.0).round() as u32;
            format!("{}:{:02} min", seconds / 60, seconds % 60)
        }
        _ => value.text(),
    }
}

fn number(recipe: &HashMap<String, Value>, key: &str, fallback: f32) -> f32 {
    recipe.get(key).and_then(Value::number).unwrap_or(fallback)
}

fn initial_score(flavor: &str) -> f32 {
    let recipe = HashMap::from([
        ("method".to_owned(), Value::from("V60")),
        ("filter".to_owned(), Value::from("Paper")),
        ("dose".to_owned(), Value::from(20.0)),
        ("water".to_owned(), Value::from(300.0)),
        ("grind".to_owned(), Value::from(5.0)),
        ("temperature".to_owned(), Value::from(94.0)),
        ("time".to_owned(), Value::from(180.0)),
        ("pours".to_owned(), Value::from(3.0)),
        ("stirs".to_owned(), Value::from(0.0)),
        ("swirls".to_owned(), Value::from(1.0)),
    ]);
    calculate(&recipe, 0)
        .get(flavor)
        .and_then(Value::number)
        .unwrap_or(50.0)
}
fn choice<'a>(recipe: &'a HashMap<String, Value>, key: &str, fallback: &'a str) -> &'a str {
    match recipe.get(key) {
        Some(Value::Text(v)) => v,
        _ => fallback,
    }
}

fn tab_channel(context: &StyleContext<'_>, tab: &str, channel: usize) -> f32 {
    let active = matches!(context.props.get("active_tab"), Some(Value::Text(value)) if value == tab);
    if active {
        [0.29, 0.20, 0.12][channel]
    } else {
        0.0
    }
}

pub fn calculate(recipe: &HashMap<String, Value>, reset_epoch: u64) -> Snapshot {
    let method = choice(recipe, "method", "V60");
    let filter = choice(recipe, "filter", "Paper");
    let dose = number(recipe, "dose", 20.0).max(1.0);
    let water = number(recipe, "water", 300.0).max(1.0);
    let grind = number(recipe, "grind", 5.0);
    let temperature = number(recipe, "temperature", 94.0);
    let time = number(recipe, "time", 180.0);
    let pours = number(recipe, "pours", 3.0);
    let stirs = number(recipe, "stirs", 0.0);
    let swirls = number(recipe, "swirls", 1.0);
    let ratio = water / dose;
    let expected_ratio = match method {
        "French press" => 15.0,
        "AeroPress" => 13.0,
        "Espresso" => 2.2,
        _ => 15.0,
    };
    let agitation = (stirs * 2.4 + swirls * 1.1 + (pours - 1.0).max(0.0) * 0.55).clamp(0.0, 24.0);
    let method_extraction = match method {
        "French press" => 2.0,
        "AeroPress" => 1.0,
        "Espresso" => 4.0,
        _ => 0.0,
    };
    let extraction = (58.0
        + (grind - 5.0) * 3.6
        + (temperature - 94.0) * 1.15
        + (time - 180.0) * 0.075
        + agitation * 0.55
        + method_extraction
        + (expected_ratio - ratio) * 0.35)
        .clamp(12.0, 94.0);
    let concentration = (68.0 + (expected_ratio - ratio) * 4.8).clamp(8.0, 96.0);
    let method_body = match method {
        "French press" => 11.0,
        "AeroPress" => 2.0,
        "Espresso" => 15.0,
        _ => 0.0,
    };
    let filter_body = match filter {
        "Metal" => 12.0,
        "Cloth" => 5.0,
        _ => -2.0,
    };
    let filter_clarity = match filter {
        "Metal" => -19.0,
        "Cloth" => -7.0,
        _ => 8.0,
    };
    let method_clarity = match method {
        "French press" => -13.0,
        "Espresso" => -8.0,
        "AeroPress" => 1.0,
        _ => 0.0,
    };
    let body = (33.0 + concentration * 0.38 + method_body + filter_body).clamp(8.0, 96.0);
    let clarity = (76.0 + filter_clarity + method_clarity - agitation * 0.5).clamp(8.0, 96.0);
    let acidity = (65.0 - (extraction - 58.0) * 0.42).clamp(8.0, 96.0);
    let sweetness = (78.0 - (extraction - 67.0).abs() * 1.25).clamp(8.0, 96.0);
    let bitterness = (20.0 + (extraction - 55.0) * 0.82).clamp(8.0, 96.0);
    let astringency =
        (13.0 + (extraction - 68.0).max(0.0) * 1.15 + agitation * 0.7).clamp(8.0, 96.0);
    let intensity =
        (extraction * 0.43 + concentration * 0.57 + if method == "Espresso" { 8.0 } else { 0.0 })
            .clamp(8.0, 96.0);
    let mut notes = Vec::new();
    if acidity >= 62.0 {
        notes.push("Citrus · bright");
    }
    if sweetness >= 65.0 {
        notes.push("Caramel · sweet");
    }
    if bitterness >= 42.0 {
        notes.push("Cocoa · roast");
    }
    if clarity >= 68.0 {
        notes.push("Tea-like · clean");
    }
    if body >= 66.0 {
        notes.push("Silky · full body");
    }
    if astringency >= 38.0 {
        notes.push("Dry finish");
    }
    if notes.is_empty() {
        notes.push("Soft · delicate");
    }
    let mut values = recipe.clone();
    for (key, value) in [
        ("acidity", acidity),
        ("sweetness", sweetness),
        ("bitterness", bitterness),
        ("body", body),
        ("clarity", clarity),
        ("astringency", astringency),
        ("intensity", intensity),
    ] {
        values.insert(key.into(), Value::Number(value));
    }
    values.insert("ratio".into(), Value::Text(format!("1 : {ratio:.1}")));
    values.insert("notes".into(), Value::Text(notes.join("  •  ")));
    values.insert(
        "extraction_signal".into(),
        Value::Text(
            (if extraction < 43.0 {
                "gentle"
            } else if extraction > 73.0 {
                "high"
            } else {
                "balanced"
            })
            .into(),
        ),
    );
    Snapshot {
        values,
        reset_epoch,
    }
}


const FLAVOR_KEYS: [&str; 7] = [
    "acidity", "sweetness", "bitterness", "body", "clarity", "astringency", "intensity",
];
const RECIPE_KEYS: [&str; 10] = [
    "method", "filter", "dose", "water", "grind", "temperature", "time", "pours", "stirs",
    "swirls",
];

fn target_loss(recipe: &HashMap<String, Value>, original: &HashMap<String, Value>) -> f32 {
    let prediction = calculate(recipe, 0);
    let flavor_error = FLAVOR_KEYS
        .iter()
        .map(|flavor| {
            let predicted = prediction.get(*flavor).and_then(Value::number).unwrap_or(50.0);
            let target = number(recipe, &format!("target_{flavor}"), predicted);
            (predicted - target).powi(2)
        })
        .sum::<f32>();
    let ranges = [
        ("dose", 32.0),
        ("water", 500.0),
        ("grind", 9.0),
        ("temperature", 20.0),
        ("time", 570.0),
        ("pours", 7.0),
        ("stirs", 8.0),
        ("swirls", 8.0),
    ];
    let changes = ranges
        .iter()
        .map(|(key, range)| {
            let delta = (number(recipe, key, 0.0) - number(original, key, 0.0)) / range;
            delta * delta * 80.0
        })
        .sum::<f32>();
    let method_cost = if choice(recipe, "method", "V60") != choice(original, "method", "V60") {
        36.0
    } else {
        0.0
    };
    let filter_cost = if choice(recipe, "filter", "Paper") != choice(original, "filter", "Paper") {
        16.0
    } else {
        0.0
    };
    flavor_error + changes + method_cost + filter_cost
}

pub fn on_change(recipe: &mut HashMap<String, Value>, changed_key: &str, _: &Value) {
    let is_target_change = changed_key
        .strip_prefix("target_")
        .is_some_and(|flavor| FLAVOR_KEYS.contains(&flavor));
    if !is_target_change && !RECIPE_KEYS.contains(&changed_key) {
        return;
    }
    if is_target_change {
        let original = recipe.clone();
        let mut best = target_loss(recipe, &original);
        let numeric = [
            ("dose", 8.0, 40.0, [4.0, 2.0, 1.0]),
            ("water", 100.0, 600.0, [80.0, 30.0, 10.0]),
            ("grind", 1.0, 10.0, [3.0, 1.0, 1.0]),
            ("temperature", 80.0, 100.0, [4.0, 1.5, 0.5]),
            ("time", 30.0, 600.0, [90.0, 30.0, 15.0]),
            ("pours", 1.0, 8.0, [2.0, 1.0, 1.0]),
            ("stirs", 0.0, 8.0, [2.0, 1.0, 1.0]),
            ("swirls", 0.0, 8.0, [2.0, 1.0, 1.0]),
        ];
        for round in 0..3 {
            for (key, choices) in [
                ("method", &["V60", "French press", "AeroPress", "Espresso"][..]),
                ("filter", &["Paper", "Metal", "Cloth"][..]),
            ] {
                let mut best_choice = choice(recipe, key, "").to_owned();
                for value in choices {
                    recipe.insert(key.into(), Value::Text((*value).into()));
                    let loss = target_loss(recipe, &original);
                    if loss < best {
                        best = loss;
                        best_choice = (*value).into();
                    }
                }
                recipe.insert(key.into(), Value::Text(best_choice));
            }
            for (key, min, max, steps) in numeric {
                let step = steps[round];
                for direction in [-1.0, 1.0] {
                    let previous = number(recipe, key, min);
                    let next = (previous + direction * step).clamp(min, max);
                    if (next - previous).abs() < 0.001 {
                        continue;
                    }
                    recipe.insert(key.into(), Value::Number(next));
                    let loss = target_loss(recipe, &original);
                    if loss < best {
                        best = loss;
                    } else {
                        recipe.insert(key.into(), Value::Number(previous));
                    }
                }
            }
        }
    }
    let prediction = calculate(recipe, 0);
    for flavor in FLAVOR_KEYS {
        if let Some(value) = prediction.get(flavor).and_then(Value::number) {
            recipe.insert(format!("target_{flavor}"), Value::Number(value));
        }
    }
}

pub fn App() -> gpui::AnyElement {
    let active_tab = signal("recipe");
    let method = signal("V60");
    let filter = signal("Paper");
    let dose = signal(20.0);
    let water = signal(300.0);
    let grind = signal(5.0);
    let temperature = signal(94.0);
    let time = signal(180.0);
    let pours = signal(3.0);
    let stirs = signal(0.0);
    let swirls = signal(1.0);
    let target_acidity = signal(initial_score("acidity"));
    let target_sweetness = signal(initial_score("sweetness"));
    let target_bitterness = signal(initial_score("bitterness"));
    let target_body = signal(initial_score("body"));
    let target_clarity = signal(initial_score("clarity"));
    let target_astringency = signal(initial_score("astringency"));
    let target_intensity = signal(initial_score("intensity"));
    let myStyles = styles({
        recipe: { flex: context.viewport_width > 480.0, minWidth: 0.0, padding: (10.0, 12.0), borderRadius: 9.0, color: rgba(184.0 / 255.0, 169.0 / 255.0, 153.0 / 255.0), fontSize: 13.0, fontWeight: 600, backgroundColor: rgba(tab_channel(context, "recipe", 0), tab_channel(context, "recipe", 1), tab_channel(context, "recipe", 2)) },
        prediction: { flex: context.viewport_width > 480.0, minWidth: 0.0, padding: (10.0, 12.0), borderRadius: 9.0, color: rgba(184.0 / 255.0, 169.0 / 255.0, 153.0 / 255.0), fontSize: 13.0, fontWeight: 600, backgroundColor: rgba(tab_channel(context, "prediction", 0), tab_channel(context, "prediction", 1), tab_channel(context, "prediction", 2)) },
        actions: { flex: context.viewport_width > 480.0, minWidth: 0.0, padding: (10.0, 12.0), borderRadius: 9.0, color: rgba(184.0 / 255.0, 169.0 / 255.0, 153.0 / 255.0), fontSize: 13.0, fontWeight: 600, backgroundColor: rgba(tab_channel(context, "actions", 0), tab_channel(context, "actions", 1), tab_channel(context, "actions", 2)) },
        page: { minHeight: context.viewport_height, fontFamily: "sans-serif", lineHeight: gpui::relative(1.4), width: gpui::Length::Percent(1.0), minWidth: 0.0, backgroundColor: rgba(23.0 / 255.0, 19.0 / 255.0, 15.0 / 255.0), textColor: rgba(244.0 / 255.0, 236.0 / 255.0, 225.0 / 255.0) },
        pageContent: {
            width: gpui::Length::Percent(1.0), maxWidth: 1440.0, margin: "auto",
            paddingTop: if context.viewport_width <= 1000.0 { 18.0 } else { 28.0 },
            paddingRight: if context.viewport_width <= 1000.0 { 14.0 } else { 28.0 },
            paddingBottom: if context.viewport_width <= 1000.0 { 18.0 } else { 28.0 },
            paddingLeft: if context.viewport_width <= 1000.0 { 14.0 } else { 28.0 },
            display: "flex", flexDirection: "column",
            gap: if context.viewport_width <= 1000.0 { 18.0 } else { 24.0 }
        },
        pageHeader: {
            display: "flex", width: gpui::Length::Percent(1.0), minWidth: 0.0,
            flexDirection: if context.viewport_width <= 1000.0 { "column" } else { "row" },
            justifyContent: if context.viewport_width <= 1000.0 { "center" } else { "space-between" },
            alignItems: "center",
            gap: if context.viewport_width <= 1000.0 { 12.0 } else { 20.0 }
        },
        brandHeading: { display: "flex", flexDirection: "column", width: gpui::Length::Percent(1.0), minWidth: 0.0, flex: context.viewport_width > 1000.0, gap: 5.0 },
        eyebrow: { fontSize: 10.0, fontWeight: 600, color: rgba(217.0 / 255.0, 155.0 / 255.0, 89.0 / 255.0) },
        pageTitle: { fontSize: 28.0, fontWeight: 700, color: rgba(248.0 / 255.0, 241.0 / 255.0, 232.0 / 255.0) },
        pageSubtitle: { fontSize: 13.0, color: rgba(169.0 / 255.0, 155.0 / 255.0, 141.0 / 255.0) },
        buildBadge: { backgroundColor: rgba(43.0 / 255.0, 33.0 / 255.0, 24.0 / 255.0), border: (1.0, rgba(75.0 / 255.0, 57.0 / 255.0, 40.0 / 255.0)), borderRadius: 99.0, padding: (8.0, 12.0), color: rgba(224.0 / 255.0, 173.0 / 255.0, 113.0 / 255.0), fontSize: 10.0, fontWeight: 600 },
        workbench: {
            display: "flex",
            flexDirection: if context.viewport_width <= 1000.0 { "column" } else { "row" },
            alignItems: "stretch", gap: if context.viewport_width <= 1000.0 { 16.0 } else { 24.0 },
            flexWrap: "nowrap", width: gpui::Length::Percent(1.0), minWidth: 0.0
        },
        tabsCard: { display: "flex", flexDirection: "column", flex: context.viewport_width > 1000.0, width: gpui::Length::Percent(1.0), minWidth: 0.0, gap: 18.0 },
        coffeeDrawing: { display: "flex", width: gpui::Length::from(gpui::px((context.viewport_width - if context.viewport_width <= 1000.0 { 28.0 } else { 56.0 }).clamp(0.0, 450.0))), minWidth: 0.0, flexShrink: 1 },
        tabBar: { display: "flex", flexDirection: if context.viewport_width <= 480.0 { "column" } else { "row" }, width: gpui::Length::Percent(1.0), gap: 6.0, padding: 6.0, border: (1.0, rgba(59.0 / 255.0, 49.0 / 255.0, 40.0 / 255.0)), borderRadius: 14.0, backgroundColor: rgba(33.0 / 255.0, 28.0 / 255.0, 23.0 / 255.0) },
        tabContent: { display: "flex", flexDirection: "column", width: gpui::Length::Percent(1.0), minWidth: 0.0 },
        tabPanel: { display: "flex", flexDirection: "column", width: gpui::Length::Percent(1.0), minWidth: 0.0 },
        actionsPanel: { display: "flex", flexDirection: "column", width: gpui::Length::Percent(1.0), minWidth: 0.0, gap: 12.0 },
        resetButton: { width: gpui::Length::from(gpui::px(142.0)), backgroundColor: rgba(62.0 / 255.0, 45.0 / 255.0, 32.0 / 255.0), color: rgba(246.0 / 255.0, 228.0 / 255.0, 207.0 / 255.0), borderRadius: 9.0, padding: (9.0, 14.0) }
    });

    <div class={myStyles.page}>
        <div class={myStyles.pageContent}>
            <div class={myStyles.pageHeader}>
                <div class={myStyles.brandHeading}>
                    <div class={myStyles.eyebrow}>BREW RECIPE STUDIO</div>
                    <div class={myStyles.pageTitle}>Coffee / Lab</div>
                    <div class={myStyles.pageSubtitle}>Describe the brew you made. See how each choice shifts the cup.</div>
                </div>
                <div class={myStyles.buildBadge}>●  RECIPE ESTIMATE</div>
            </div>

            <div class={myStyles.workbench}>
                <ExtractionPreviews class={myStyles.coffeeDrawing} water={water} intensity={intensity} bitterness={bitterness} body={body} method={method} />
                <div class={myStyles.tabsCard}>
                    <div class={myStyles.tabBar}>
                        <button id="tab-recipe" on-click={active_tab.set("recipe")} class={myStyles.recipe}>Recipe</button>
                        <button id="tab-prediction" on-click={active_tab.set("prediction")} class={myStyles.prediction}>Cup Prediction</button>
                        <button id="tab-actions" on-click={active_tab.set("actions")} class={myStyles.actions}>What you did</button>
                    </div>
                    <div class={myStyles.tabContent}>
                        {if active_tab == "recipe" {
                            <CoffeeVariablesForm
                                class={myStyles.tabPanel}
                                section={"recipe"}
                                method={method}
                                dose={dose}
                                water={water}
                                grind={grind}
                                temperature={temperature}
                                time={time}
                                pours={pours}
                                stirs={stirs}
                                swirls={swirls}
                                filter={filter}
                            />
                        } else {
                            {if active_tab == "prediction" {
                                <CoffeeProfile
                                    class={myStyles.tabPanel}
                                    acidity={acidity}
                                    target_acidity={target_acidity}
                                    sweetness={sweetness}
                                    target_sweetness={target_sweetness}
                                    bitterness={bitterness}
                                    target_bitterness={target_bitterness}
                                    body={body}
                                    target_body={target_body}
                                    clarity={clarity}
                                    target_clarity={target_clarity}
                                    astringency={astringency}
                                    target_astringency={target_astringency}
                                    intensity={intensity}
                                    target_intensity={target_intensity}
                                    notes={notes}
                                    ratio={ratio}
                                    extraction_signal={extraction_signal}
                                />
                            } else {
                                <div class={myStyles.actionsPanel}>
                                    <CoffeeVariablesForm
                                        class={myStyles.tabPanel}
                                        section={"actions"}
                                        method={method}
                                        dose={dose}
                                        water={water}
                                        grind={grind}
                                        temperature={temperature}
                                        time={time}
                                        pours={pours}
                                        stirs={stirs}
                                        swirls={swirls}
                                        filter={filter}
                                    />
                                    <button
                                        id="reset"
                                        on-click={reset_recipe(&method, &filter, &dose, &water, &grind, &temperature, &time, &pours, &stirs, &swirls)}
                                        class={myStyles.resetButton}
                                        >Reset recipe</button>
                                </div>
                            }}
                        }}
                    </div>
                </div>
            </div>
        </div>
    </div>
}
