<script>
use gpui_rsc::{component, in_param};
use gpui_rsc::runtime::{Definition, InlineStyle as Style, Length, StyleContext, Value};

pub fn definition() -> Definition {
    component! {
        name: "brew-visualization",
        imports: [],
        bindings: [
            in_param!("method"),
            in_param!("filter"),
            in_param!("water"),
            in_param!("pours"),
            in_param!("acidity"),
            in_param!("bitterness"),
            in_param!("body"),
            in_param!("clarity"),
            in_param!("intensity"),
            in_param!("extraction_signal"),
            in_param!("character"),
        ],
    }
}

fn number(context: &StyleContext<'_>, key: &str, fallback: f32) -> f32 {
    context.snapshot.get(key).and_then(Value::number).unwrap_or(fallback)
}

fn text<'a>(context: &'a StyleContext<'_>, key: &str, fallback: &'a str) -> &'a str {
    match context.snapshot.get(key) {
        Some(Value::Text(value)) => value,
        _ => fallback,
    }
}

fn blend_color(from: u32, to: u32, amount: f32) -> u32 {
    let amount = amount.clamp(0.0, 1.0);
    let channel = |shift: u32| {
        let from = ((from >> shift) & 0xff) as f32;
        let to = ((to >> shift) & 0xff) as f32;
        (from + (to - from) * amount).round() as u32
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

pub struct PreviewStyles {
    pub coffee_tone: Style,
    pub cup_fill: Style,
    pub filter_tone: Style,
    pub kettle_tone: Style,
    pub kettle_outline: Style,
}

pub fn styles(context: &StyleContext<'_>) -> PreviewStyles {
    let intensity = number(context, "profile.intensity", 55.0);
    let bitterness = number(context, "profile.bitterness", 35.0);
    let body = number(context, "profile.body", 50.0);
    let water = number(context, "recipe.water", 300.0);
    let strength = ((intensity * 0.5 + bitterness * 0.3 + body * 0.2) / 100.0).clamp(0.0, 1.0);
    let coffee = blend_color(0xd8904e, 0x482315, strength);
    let level = (0.28 + ((water - 100.0) / 500.0).clamp(0.0, 1.0) * 0.48).clamp(0.22, 0.78);
    let filter_color = match text(context, "recipe.filter", "Paper") {
        "Metal" => 0xaeb5b2,
        "Cloth" => 0x8b6a4d,
        _ => 0xd8bd97,
    };
    let kettle_color = if text(context, "recipe.method", "V60") == "Espresso" { 0x6d7f75 } else { 0xc9824c };
    PreviewStyles {
        coffee_tone: Style::new().background_color(coffee),
        cup_fill: Style::new().background_color(coffee).height(Length::Percent(level)),
        filter_tone: Style::new().background_color(filter_color),
        kettle_tone: Style::new().background_color(kettle_color),
        kettle_outline: Style::new().border(2.0, kettle_color),
    }
}
</script>
<!doctype html>
<html><head>
  <style>
    @keyframes live-pulse { from { opacity:0.55; } to { opacity:1; } }
    @keyframes first-drop { from { top:109px; opacity:1; } to { top:152px; opacity:0.2; } }
    @keyframes second-drop { from { top:112px; opacity:1; } to { top:151px; opacity:0.2; } }
    @keyframes cup-drip { from { top:170px; opacity:1; } to { top:192px; opacity:0; } }
    @keyframes left-steam { from { bottom:77px; opacity:0.45; } to { bottom:98px; opacity:0; } }
    @keyframes right-steam { from { bottom:79px; opacity:0.42; } to { bottom:97px; opacity:0; } }
    .preview-panel { display:flex; flex-direction:column; flex:1; min-width:0; gap:14px; background:#211c17; color:#f4ece1; border:1px solid #3b3128; border-radius:18px; padding:18px; }
    .preview-row { display:flex; align-items:center; justify-content:space-between; }
    .preview-status { display:flex; align-items:center; gap:8px; }
    .live-dot { width:8px; height:8px; border-radius:99px; background:#e7a45e; animation:live-pulse 1200ms infinite; }
    .preview-title { font-size:10px; font-weight:700; color:#e7a45e; }
    .extraction-badge { background:#35281e; border-radius:99px; padding:4px 8px; font-size:10px; color:#e8c095; }
    .brew-scene { position:relative; width:100%; height:250px; overflow:hidden; border-radius:16px; background:#f4e6d3; }
    .scene-sun { position:absolute; top:14px; right:14px; width:56px; height:56px; border-radius:99px; background:#f8eddf; }
    .kettle-handle { position:absolute; top:26px; left:56px; width:32px; height:28px; border-radius:99px; }
    .kettle-body { position:absolute; top:30px; left:17px; width:66px; height:43px; border-radius:16px; border:2px solid #a9663a; }
    .kettle-highlight { position:absolute; top:39px; left:23px; width:26px; height:7px; border-radius:99px; }
    .kettle-spout { position:absolute; top:59px; left:69px; width:46px; height:8px; border-radius:99px; }
    .filter-layer-one { position:absolute; top:86px; left:98px; width:106px; height:17px; border-radius:8px; }
    .filter-layer-two { position:absolute; top:102px; left:106px; width:90px; height:19px; border-radius:8px; }
    .filter-layer-three { position:absolute; top:120px; left:116px; width:70px; height:19px; border-radius:8px; }
    .filter-layer-four { position:absolute; top:138px; left:126px; width:50px; height:19px; border-radius:8px; }
    .filter-layer-five { position:absolute; top:156px; left:138px; width:26px; height:18px; border-radius:8px; }
    .dripper-rim { position:absolute; top:87px; left:100px; width:102px; height:12px; border-radius:99px; background:#efd9b9; }
    .coffee-surface { position:absolute; top:99px; left:116px; width:70px; height:13px; border-radius:99px; }
    .drop-one { position:absolute; top:109px; left:140px; width:6px; height:11px; border-radius:99px; animation:first-drop 2100ms infinite; }
    .drop-two { position:absolute; top:112px; left:154px; width:5px; height:8px; border-radius:99px; animation:second-drop 2380ms infinite; }
    .cup-drop { position:absolute; top:170px; left:151px; width:5px; height:8px; border-radius:99px; animation:cup-drip 2630ms infinite; }
    .cup-saucer { position:absolute; bottom:13px; left:83px; width:112px; height:8px; border-radius:99px; background:#d8c5ae; }
    .cup-body { position:absolute; bottom:19px; left:98px; width:88px; height:54px; overflow:hidden; border-radius:16px; border:2px solid #cbb69e; background:#fffaf2; }
    .cup-liquid { position:absolute; bottom:0px; left:2px; right:2px; border-radius:12px; }
    .cup-handle { position:absolute; bottom:31px; left:178px; width:22px; height:25px; border-radius:99px; border:2px solid #cbb69e; }
    .steam-left { position:absolute; bottom:77px; left:125px; width:7px; height:18px; border-radius:99px; background:#b6a28b; animation:left-steam 1800ms infinite; }
    .steam-right { position:absolute; bottom:79px; left:145px; width:6px; height:16px; border-radius:99px; background:#b6a28b; animation:right-steam 2100ms infinite; }
    .metric-stack { display:flex; flex-direction:column; gap:3px; }
    .metric-label { font-size:10px; color:#a99b8d; }
    .metric-value { font-size:14px; font-weight:700; color:#f4ece1; }
    .metric-stack-end { display:flex; flex-direction:column; align-items:flex-end; gap:3px; }
    .intensity-value { font-size:14px; font-weight:700; color:#e7a45e; }
    .preview-footer { display:flex; justify-content:space-between; background:#2a231d; border-radius:10px; padding:8px 10px; font-size:10px; color:#c9b9a6; }
    .footer-method { display:flex; gap:4px; }
  </style>
</head><body>
  <section class="preview-panel">
    <div class="preview-row">
      <div class="preview-status">
        <span class="live-dot"></span>
        <strong class="preview-title">BREW PREVIEW</strong>
      </div>
      <output data-in="extraction_signal" class="extraction-badge"></output>
    </div>

    <div class="brew-scene">
      <div class="scene-sun"></div>
      <div class="kettle-handle" class={styles.kettle_outline}></div>
      <div class="kettle-body" class={styles.kettle_tone}></div>
      <div class="kettle-highlight" class={Style::new().background_color(if text(context, "recipe.method", "V60") == "Espresso" { 0x9eafa5 } else { 0xe7b17b })}></div>
      <div class="kettle-spout" class={styles.kettle_tone}></div>

      <div class="filter-layer-one" class={styles.filter_tone}></div>
      <div class="filter-layer-two" class={styles.filter_tone}></div>
      <div class="filter-layer-three" class={styles.filter_tone}></div>
      <div class="filter-layer-four" class={styles.filter_tone}></div>
      <div class="filter-layer-five" class={styles.filter_tone}></div>
      <div class="dripper-rim"></div>
      <div class="coffee-surface" class={styles.coffee_tone}></div>
      <div class="drop-one" class={styles.coffee_tone}></div>
      <div class="drop-two" class={styles.coffee_tone}></div>
      <div class="cup-drop" class={styles.coffee_tone}></div>

      <div class="cup-saucer"></div>
      <div class="cup-body">
        <div class="cup-liquid" class={styles.cup_fill}></div>
      </div>
      <div class="cup-handle"></div>
      <div class="steam-left"></div>
      <div class="steam-right"></div>
    </div>

    <div class="preview-row">
      <div class="metric-stack">
        <span class="metric-label">CUP CHARACTER</span>
        <output data-in="character" class="metric-value"></output>
      </div>
      <div class="metric-stack-end">
        <span class="metric-label">INTENSITY</span>
        <output data-in="intensity" data-format="score" data-suffix="%" class="intensity-value"></output>
      </div>
    </div>
    <div class="preview-footer">
      <div class="footer-method"><output data-in="method"></output><span>·</span><output data-in="filter"></output><span>filter</span></div>
      <output data-in="water" data-format="score" data-suffix=" g water"></output>
    </div>
  </section>
</body></html>
