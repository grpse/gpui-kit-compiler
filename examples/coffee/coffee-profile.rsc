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
<html><head>
  <style>
    @media (max-width: 768px) {
      .mobile-card { width:100%; max-width:560px; min-width:0; padding:18px; gap:16px; }
    }
    .profile-card { display:flex; flex-direction:column; gap:20px; flex:1; min-width:360px; background:#f9f5ef; color:#2d251f; border-radius:22px; padding:24px; }
    .profile-header { display:flex; justify-content:space-between; align-items:center; gap:12px; }
    .profile-heading { display:flex; flex-direction:column; gap:4px; }
    .profile-title { font-size:20px; font-weight:700; }
    .profile-help { font-size:11px; color:#8b7d70; }
    .ratio-badge { background:#eee5d9; border-radius:99px; padding:5px 12px; font-size:11px; color:#765b43; font-weight:600; }
    .score-panel { display:flex; flex-direction:column; gap:17px; background:#ffffff; border-radius:14px; padding:18px; }
    .score-row { display:flex; flex-direction:column; gap:6px; }
    .score-heading { display:flex; justify-content:space-between; }
    .score-slider { width:100%; }
    .notes { display:flex; flex-direction:column; gap:8px; }
    .notes-title { font-size:13px; font-weight:700; }
    .notes-value { font-size:12px; color:#765b43; }
    .extraction-row { display:flex; justify-content:space-between; background:#eee5d9; border-radius:13px; padding:12px 16px; }
    .extraction-label { font-size:11px; color:#74675c; }
    .extraction-value { font-size:12px; font-weight:600; color:#533e2c; }
    .footnote { font-size:10px; color:#94877b; }
  </style>
</head><body>
      <section class="mobile-card profile-card">
        <div class="profile-header">
          <div class="profile-heading"><h2 class="profile-title">Cup prediction</h2><p class="profile-help">Drag a score to adjust the recipe · 0–100</p></div>
          <output data-in="ratio" data-format="ratio" class="ratio-badge"></output>
        </div>
        <div class="score-panel">
          <div class="score-row"><div class="score-heading"><span>Acidity</span><output data-in="acidity" data-format="score"></output></div><input id="flavor-acidity" type="range" min="0" max="100" step="1" value="64" data-in-out="acidity" class="score-slider"></div>
          <div class="score-row"><div class="score-heading"><span>Sweetness</span><output data-in="sweetness" data-format="score"></output></div><input id="flavor-sweetness" type="range" min="0" max="100" step="1" value="68" data-in-out="sweetness" class="score-slider"></div>
          <div class="score-row"><div class="score-heading"><span>Bitterness</span><output data-in="bitterness" data-format="score"></output></div><input id="flavor-bitterness" type="range" min="0" max="100" step="1" value="23" data-in-out="bitterness" class="score-slider"></div>
          <div class="score-row"><div class="score-heading"><span>Body</span><output data-in="body" data-format="score"></output></div><input id="flavor-body" type="range" min="0" max="100" step="1" value="57" data-in-out="body" class="score-slider"></div>
          <div class="score-row"><div class="score-heading"><span>Clarity</span><output data-in="clarity" data-format="score"></output></div><input id="flavor-clarity" type="range" min="0" max="100" step="1" value="83" data-in-out="clarity" class="score-slider"></div>
          <div class="score-row"><div class="score-heading"><span>Astringency</span><output data-in="astringency" data-format="score"></output></div><input id="flavor-astringency" type="range" min="0" max="100" step="1" value="15" data-in-out="astringency" class="score-slider"></div>
          <div class="score-row"><div class="score-heading"><span>Intensity</span><output data-in="intensity" data-format="score"></output></div><input id="flavor-intensity" type="range" min="0" max="100" step="1" value="64" data-in-out="intensity" class="score-slider"></div>
        </div>
        <div class="notes"><h3 class="notes-title">Likely impressions</h3><output data-in="notes" class="notes-value"></output></div>
        <div class="extraction-row"><span class="extraction-label">Extraction signal</span><output data-in="extraction_signal" class="extraction-value"></output></div>
        <p class="footnote">Recipe-only estimate. Coffee origin, roast, and tasting feedback are not included yet.</p>
      </section>
</body></html>
