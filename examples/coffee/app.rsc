<script>
use gpui_rsc::{in_binding, in_out_binding, out_binding};
use gpui_rsc::runtime::{Value, Snapshot};
use std::collections::HashMap;

use gpui_rsc::{component, runtime::{Definition, InlineStyle as Style, Length, StyleContext}};
use crate::generated::{brew_visualization, coffee_profile, coffee_variables_form};

pub fn definition() -> Definition {
    component! {
        name: "app",
        imports: [coffee_variables_form::CoffeeVariablesFormComponent::definition(), coffee_profile::CoffeeProfileComponent::definition(), brew_visualization::BrewVisualizationComponent::definition()],
        bindings: [
            in_out_binding!("method" => "recipe.method"),
            in_out_binding!("dose" => "recipe.dose"),
            in_out_binding!("water" => "recipe.water"),
            in_out_binding!("grind" => "recipe.grind"),
            in_out_binding!("temperature" => "recipe.temperature"),
            in_out_binding!("time" => "recipe.time"),
            in_out_binding!("pours" => "recipe.pours"),
            in_out_binding!("stirs" => "recipe.stirs"),
            in_out_binding!("swirls" => "recipe.swirls"),
            in_out_binding!("filter" => "recipe.filter"),
            out_binding!("reset", |engine, _| engine.reset()),
            in_out_binding!("acidity", data_key = None,
                get = |snapshot: &Snapshot| snapshot.get("profile.acidity").cloned(),
                set = |engine: &gpui_rsc::runtime::Engine, value: Value| engine.set("target.acidity", value)),
            in_out_binding!("sweetness", data_key = None,
                get = |snapshot: &Snapshot| snapshot.get("profile.sweetness").cloned(),
                set = |engine: &gpui_rsc::runtime::Engine, value: Value| engine.set("target.sweetness", value)),
            in_out_binding!("bitterness", data_key = None,
                get = |snapshot: &Snapshot| snapshot.get("profile.bitterness").cloned(),
                set = |engine: &gpui_rsc::runtime::Engine, value: Value| engine.set("target.bitterness", value)),
            in_out_binding!("body", data_key = None,
                get = |snapshot: &Snapshot| snapshot.get("profile.body").cloned(),
                set = |engine: &gpui_rsc::runtime::Engine, value: Value| engine.set("target.body", value)),
            in_out_binding!("clarity", data_key = None,
                get = |snapshot: &Snapshot| snapshot.get("profile.clarity").cloned(),
                set = |engine: &gpui_rsc::runtime::Engine, value: Value| engine.set("target.clarity", value)),
            in_out_binding!("astringency", data_key = None,
                get = |snapshot: &Snapshot| snapshot.get("profile.astringency").cloned(),
                set = |engine: &gpui_rsc::runtime::Engine, value: Value| engine.set("target.astringency", value)),
            in_out_binding!("intensity", data_key = None,
                get = |snapshot: &Snapshot| snapshot.get("profile.intensity").cloned(),
                set = |engine: &gpui_rsc::runtime::Engine, value: Value| engine.set("target.intensity", value)),
            in_binding!("notes" => "profile.notes"),

            in_binding!("ratio", |snapshot: &Snapshot| snapshot.get("derived.ratio").cloned()),
            in_binding!("extraction_signal" => "derived.extraction_signal"),
            in_binding!("character" => "preview.character"),
        ],
        calculate: calculate,
        on_change: adjust_recipe_to_target,
    }
    .with_output_formatter(output_format)
}

pub fn output_format(name: &str, value: &Value) -> String {
    match name {
        "score" => format!("{:.0}", value.number().unwrap_or(0.0)),
        "ratio" => format!("1 : {:.1}", value.number().unwrap_or(0.0)),
        "duration" => {
            let seconds = value.number().unwrap_or(0.0).round() as u32;
            format!("{}:{:02} min", seconds / 60, seconds % 60)
        }
        _ => value.text(),
    }
}

pub struct AppStyles {
    pub mobile_card: Style,
}

fn mobile_card_style(context: &StyleContext<'_>) -> Style {
    if context.viewport_width <= 768.0 {
        let card_width = (context.viewport_width - 28.0).max(0.0).min(560.0);
        Style::new()
            .width(Length::Percent(1.0))
            .min_width(0.0)
            .max_width(card_width)
    } else {
        Style::new()
    }
}

pub fn styles(context: &StyleContext<'_>) -> AppStyles {
    AppStyles { mobile_card: mobile_card_style(context) }
}

fn number(recipe: &HashMap<String, Value>, key: &str, fallback: f32) -> f32 {
    recipe.get(key).and_then(Value::number).unwrap_or(fallback)
}
fn choice<'a>(recipe: &'a HashMap<String, Value>, key: &str, fallback: &'a str) -> &'a str {
    match recipe.get(key) {
        Some(Value::Text(v)) => v,
        _ => fallback,
    }
}

pub fn calculate(recipe: &HashMap<String, Value>, reset_epoch: u64) -> Snapshot {
    let method = choice(recipe, "recipe.method", "V60");
    let filter = choice(recipe, "recipe.filter", "Paper");
    let dose = number(recipe, "recipe.dose", 20.0).max(1.0);
    let water = number(recipe, "recipe.water", 300.0).max(1.0);
    let grind = number(recipe, "recipe.grind", 5.0);
    let temperature = number(recipe, "recipe.temperature", 94.0);
    let time = number(recipe, "recipe.time", 180.0);
    let pours = number(recipe, "recipe.pours", 3.0);
    let stirs = number(recipe, "recipe.stirs", 0.0);
    let swirls = number(recipe, "recipe.swirls", 1.0);
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
        ("profile.acidity", acidity),
        ("profile.sweetness", sweetness),
        ("profile.bitterness", bitterness),
        ("profile.body", body),
        ("profile.clarity", clarity),
        ("profile.astringency", astringency),
        ("profile.intensity", intensity),
        ("derived.ratio", ratio),
    ] {
        values.insert(key.into(), Value::Number(value));
    }
    values.insert("profile.notes".into(), Value::Text(notes.join("  •  ")));
    let character = if intensity > 69.0 || bitterness > 62.0 {
        "Bold & roasty"
    } else if clarity > 70.0 && acidity > 58.0 {
        "Bright & tea-like"
    } else if body > 66.0 {
        "Silky & full"
    } else {
        "Soft & balanced"
    };
    values.insert("preview.character".into(), Value::Text(character.into()));
    values.insert(
        "derived.extraction_signal".into(),
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

fn target_loss(recipe: &HashMap<String, Value>, original: &HashMap<String, Value>) -> f32 {
    let prediction = calculate(recipe, 0);
    let flavor_error = FLAVOR_KEYS.iter().map(|flavor| {
        let predicted = prediction.get(&format!("profile.{flavor}")).and_then(Value::number).unwrap_or(50.0);
        let target = number(recipe, &format!("target.{flavor}"), predicted);
        (predicted - target).powi(2)
    }).sum::<f32>();
    let ranges = [
        ("recipe.dose", 32.0), ("recipe.water", 500.0), ("recipe.grind", 9.0),
        ("recipe.temperature", 20.0), ("recipe.time", 570.0), ("recipe.pours", 7.0),
        ("recipe.stirs", 8.0), ("recipe.swirls", 8.0),
    ];
    let changes = ranges.iter().map(|(key, range)| {
        let delta = (number(recipe, key, 0.0) - number(original, key, 0.0)) / range;
        delta * delta * 80.0
    }).sum::<f32>();
    let method_cost = if choice(recipe, "recipe.method", "V60") != choice(original, "recipe.method", "V60") { 36.0 } else { 0.0 };
    let filter_cost = if choice(recipe, "recipe.filter", "Paper") != choice(original, "recipe.filter", "Paper") { 16.0 } else { 0.0 };
    flavor_error + changes + method_cost + filter_cost
}

pub fn adjust_recipe_to_target(recipe: &mut HashMap<String, Value>, changed_key: &str, _: &Value) {
    if !changed_key.starts_with("target.") { return; }
    for flavor in FLAVOR_KEYS {
        let key = format!("target.{flavor}");
        if key != changed_key { recipe.remove(&key); }
    }
    let original = recipe.clone();
    let mut best = target_loss(recipe, &original);
    let numeric = [
        ("recipe.dose", 8.0, 40.0, [4.0, 2.0, 1.0]),
        ("recipe.water", 100.0, 600.0, [80.0, 30.0, 10.0]),
        ("recipe.grind", 1.0, 10.0, [3.0, 1.0, 1.0]),
        ("recipe.temperature", 80.0, 100.0, [4.0, 1.5, 0.5]),
        ("recipe.time", 30.0, 600.0, [90.0, 30.0, 15.0]),
        ("recipe.pours", 1.0, 8.0, [2.0, 1.0, 1.0]),
        ("recipe.stirs", 0.0, 8.0, [2.0, 1.0, 1.0]),
        ("recipe.swirls", 0.0, 8.0, [2.0, 1.0, 1.0]),
    ];
    for round in 0..3 {
        for (key, choices) in [
            ("recipe.method", &["V60", "French press", "AeroPress", "Espresso"][..]),
            ("recipe.filter", &["Paper", "Metal", "Cloth"][..]),
        ] {
            let mut best_choice = choice(recipe, key, "").to_owned();
            for value in choices {
                recipe.insert(key.into(), Value::Text((*value).into()));
                let loss = target_loss(recipe, &original);
                if loss < best { best = loss; best_choice = (*value).into(); }
            }
            recipe.insert(key.into(), Value::Text(best_choice));
        }
        for (key, min, max, steps) in numeric {
            let step = steps[round];
            for direction in [-1.0, 1.0] {
                let previous = number(recipe, key, min);
                let next = (previous + direction * step).clamp(min, max);
                if (next - previous).abs() < 0.001 { continue; }
                recipe.insert(key.into(), Value::Number(next));
                let loss = target_loss(recipe, &original);
                if loss < best { best = loss; }
                else { recipe.insert(key.into(), Value::Number(previous)); }
            }
        }
    }
}
</script>
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Coffee / Lab</title>
  <style>
    @media (max-width: 768px) {
      main { padding:18px 14px; gap:18px; }
      header { flex-direction:column; align-items:center; gap:12px; }
      .cards { flex-direction:column; flex-wrap:nowrap; align-items:center; gap:16px; width:100%; min-width:0; }
      .mobile-card { width:100%; max-width:560px; min-width:0; }
    }
    .page { background:#17130f; color:#f4ece1; overflow-y:auto; }
    .page-content { width:100%; max-width:1200px; margin:auto; padding:28px; display:flex; flex-direction:column; gap:24px; }
    .page-header { display:flex; justify-content:space-between; align-items:center; gap:20px; }
    .brand-heading { display:flex; flex-direction:column; gap:5px; }
    .eyebrow { font-size:10px; font-weight:600; color:#d99b59; }
    .page-title { font-size:28px; font-weight:700; color:#f8f1e8; }
    .page-subtitle { font-size:13px; color:#a99b8d; }
    .build-badge { background:#2b2118; border:1px solid #4b3928; border-radius:99px; padding:8px 12px; color:#e0ad71; font-size:10px; font-weight:600; }
    .cards { display:flex; gap:24px; align-items:flex-start; flex-wrap:wrap; }
  </style>
</head>
<body class="page">
  <main class="page-content">
    <header class="page-header">
      <div class="brand-heading">
        <p class="eyebrow">BREW RECIPE STUDIO</p>
        <h1 class="page-title">Coffee / Lab</h1>
        <p class="page-subtitle">Describe the brew you made. See how each choice shifts the cup.</p>
      </div>
      <div class="build-badge">●  RECIPE ESTIMATE</div>
    </header>

    <div class="cards">
      <component class="mobile-card controls-form" class={styles.mobile_card} name="coffee-variables-form" method="[method]" dose="[dose]" water="[water]" grind="[grind]" temperature="[temperature]" time="[time]" pours="[pours]" stirs="[stirs]" swirls="[swirls]" filter="[filter]" reset="[reset]" />
      <component class="mobile-card profile-form" class={styles.mobile_card} name="coffee-profile" acidity="[acidity]" sweetness="[sweetness]" bitterness="[bitterness]" body="[body]" clarity="[clarity]" astringency="[astringency]" intensity="[intensity]" notes="[notes]" ratio="[ratio]" extraction_signal="[extraction_signal]" />
      <component class="mobile-card brew-preview" class={mobile_card_style(context)} name="brew-visualization" method="[method]" filter="[filter]" water="[water]" pours="[pours]" acidity="[acidity]" bitterness="[bitterness]" body="[body]" clarity="[clarity]" intensity="[intensity]" extraction_signal="[extraction_signal]" character="[character]" />
    </div>
  </main>
</body>
</html>
