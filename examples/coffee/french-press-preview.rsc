<script>
use gpui_rsc::{component, in_param};
use gpui_rsc::runtime::{Definition, InlineStyle as Style, Length, StyleContext, Value};

pub fn definition() -> Definition {
    component! {
        name: "french-press-preview",
        imports: [],
        bindings: [in_param!("water"), in_param!("intensity"), in_param!("bitterness"), in_param!("body")],
    }
}

fn number(context: &StyleContext<'_>, key: &str, fallback: f32) -> f32 {
    context.snapshot.get(key).and_then(Value::number).unwrap_or(fallback)
}

pub struct PreviewStyles { pub coffee: Style, pub liquid: Style }

pub fn styles(context: &StyleContext<'_>) -> PreviewStyles {
    let strength = ((number(context, "profile.intensity", 55.0) * 0.5
        + number(context, "profile.bitterness", 35.0) * 0.3
        + number(context, "profile.body", 50.0) * 0.2) / 100.0).clamp(0.0, 1.0);
    let channel = |from: f32, to: f32| (from + (to - from) * strength).round() as u32;
    let coffee = (channel(0xd8, 0x48) << 16) | (channel(0x90, 0x23) << 8) | channel(0x4e, 0x15);
    let water = number(context, "recipe.water", 300.0);
    let level = (0.24 + ((water - 100.0) / 500.0).clamp(0.0, 1.0) * 0.62).clamp(0.2, 0.88);
    PreviewStyles { coffee: Style::new().background_color(coffee), liquid: Style::new().background_color(coffee).height(Length::Percent(level)) }
}
</script>
<!doctype html>
<html><head><style>
  .preview-card { display:flex; flex-direction:column; flex:1; min-width:0; gap:12px; padding:18px; border:1px solid #3b3128; border-radius:16px; background:#211c17; color:#f4ece1; }
  .preview-title { font-size:14px; font-weight:700; color:#f2c18a; }
  .preview-copy { font-size:11px; color:#a99b8d; }
  .stage { display:flex; align-items:center; justify-content:center; height:166px; border-radius:12px; background:#f4e6d3; }
  .press { position:relative; width:70px; height:116px; overflow:hidden; border:3px solid #cbb69e; border-radius:12px; background:#fffaf2; }
  .coffee-liquid { position:absolute; right:2px; bottom:2px; left:2px; border-radius:8px; }
  .plunger { position:absolute; top:0px; left:32px; width:6px; height:85px; border-radius:99px; background:#8b7b69; }
  .press-lid { position:absolute; top:-3px; left:-8px; width:86px; height:9px; border-radius:6px; background:#6a5540; }
</style></head><body>
  <section class="preview-card">
    <div><h3 class="preview-title">French press · Immersion</h3><p class="preview-copy">Coffee steeps in the beaker before the plunger separates the grounds.</p></div>
    <div class="stage">
      <div class="press">
        <div class="press-lid"></div>
        <div class="plunger"></div>
        <div class="coffee-liquid" class={styles.liquid}></div>
      </div>
    </div>
  </section>
</body></html>
