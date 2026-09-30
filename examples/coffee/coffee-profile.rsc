<script>
use gpui_rsc::{in_param, in_out_param};

use gpui_rsc::{component, runtime::Definition};

pub fn definition() -> Definition {
    component! {
        name: "coffee-profile",
        imports: [],
        bindings: [
            in_out_param!("acidity"),
            in_out_param!("sweetness"),
            in_out_param!("bitterness"),
            in_out_param!("body"),
            in_out_param!("clarity"),
            in_out_param!("astringency"),
            in_out_param!("intensity"),
            in_param!("notes"),
            in_param!("ratio"),
            in_param!("extraction_signal"),

        ],
    }
}
</script>
<!doctype html>
<html><body>
      <section style="display:flex; flex-direction:column; gap:20px; flex:1; min-width:360px; background:#f9f5ef; color:#2d251f; border-radius:22px; padding:24px">
        <div style="display:flex; justify-content:space-between; align-items:center; gap:12px">
          <div style="display:flex; flex-direction:column; gap:4px"><h2 style="font-size:20px; font-weight:700">Cup prediction</h2><p style="font-size:11px; color:#8b7d70">Drag a score to adjust the recipe · 0–100</p></div>
          <output data-in="ratio" data-format="ratio" style="background:#eee5d9; border-radius:99px; padding:5px 12px; font-size:11px; color:#765b43; font-weight:600"></output>
        </div>
        <div style="display:flex; flex-direction:column; gap:17px; background:#ffffff; border-radius:14px; padding:18px">
          <div style="display:flex; flex-direction:column; gap:6px"><div style="display:flex; justify-content:space-between"><span>Acidity</span><output data-in="acidity" data-format="score"></output></div><input id="flavor-acidity" type="range" min="0" max="100" step="1" value="64" data-in-out="acidity" style="width:100%"></div>
          <div style="display:flex; flex-direction:column; gap:6px"><div style="display:flex; justify-content:space-between"><span>Sweetness</span><output data-in="sweetness" data-format="score"></output></div><input id="flavor-sweetness" type="range" min="0" max="100" step="1" value="68" data-in-out="sweetness" style="width:100%"></div>
          <div style="display:flex; flex-direction:column; gap:6px"><div style="display:flex; justify-content:space-between"><span>Bitterness</span><output data-in="bitterness" data-format="score"></output></div><input id="flavor-bitterness" type="range" min="0" max="100" step="1" value="23" data-in-out="bitterness" style="width:100%"></div>
          <div style="display:flex; flex-direction:column; gap:6px"><div style="display:flex; justify-content:space-between"><span>Body</span><output data-in="body" data-format="score"></output></div><input id="flavor-body" type="range" min="0" max="100" step="1" value="57" data-in-out="body" style="width:100%"></div>
          <div style="display:flex; flex-direction:column; gap:6px"><div style="display:flex; justify-content:space-between"><span>Clarity</span><output data-in="clarity" data-format="score"></output></div><input id="flavor-clarity" type="range" min="0" max="100" step="1" value="83" data-in-out="clarity" style="width:100%"></div>
          <div style="display:flex; flex-direction:column; gap:6px"><div style="display:flex; justify-content:space-between"><span>Astringency</span><output data-in="astringency" data-format="score"></output></div><input id="flavor-astringency" type="range" min="0" max="100" step="1" value="15" data-in-out="astringency" style="width:100%"></div>
          <div style="display:flex; flex-direction:column; gap:6px"><div style="display:flex; justify-content:space-between"><span>Intensity</span><output data-in="intensity" data-format="score"></output></div><input id="flavor-intensity" type="range" min="0" max="100" step="1" value="64" data-in-out="intensity" style="width:100%"></div>
        </div>
        <div style="display:flex; flex-direction:column; gap:8px"><h3 style="font-size:13px; font-weight:700">Likely impressions</h3><output data-in="notes" style="font-size:12px; color:#765b43"></output></div>
        <div style="display:flex; justify-content:space-between; background:#eee5d9; border-radius:13px; padding:12px 16px"><span style="font-size:11px; color:#74675c">Extraction signal</span><output data-in="extraction_signal" style="font-size:12px; font-weight:600; color:#533e2c"></output></div>
        <p style="font-size:10px; color:#94877b">Recipe-only estimate. Coffee origin, roast, and tasting feedback are not included yet.</p>
      </section>
</body></html>
