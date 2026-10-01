<script>
use gpui_rsc::{component, in_param};
use gpui_rsc::runtime::{Definition, InlineStyle as Style, Length, StyleContext, Value};

pub fn definition() -> Definition {
    component! {
        name: "v60-preview",
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
  .stage { display:flex; flex-direction:column; align-items:center; justify-content:flex-end; gap:8px; height:166px; border-radius:12px; background:#f4e6d3; }
  .dripper { width:94px; height:44px; border-radius:12px; background:#d8bd97; }
  .server { position:relative; width:76px; height:76px; overflow:hidden; border:3px solid #cbb69e; border-radius:12px; background:#fffaf2; }
  .coffee-liquid { position:absolute; right:2px; bottom:2px; left:2px; border-radius:8px; }
  .waterline { width:82px; height:4px; border-radius:99px; background:#d8c5ae; }
</style></head><body>
  <section class="preview-card">
    <div><h3 class="preview-title">V60 · Pour-over</h3><p class="preview-copy">Water passes through a paper filter into a server.</p></div>
    <div class="stage">
      <div class="dripper" class={styles.coffee}></div>
      <div class="waterline"></div>
      <div class="server"><div class="coffee-liquid" class={styles.liquid}></div></div>
    </div>
  </section>
</body></html>
