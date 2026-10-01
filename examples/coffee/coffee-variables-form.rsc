<script>
use gpui_rsc::{in_out_param, out_param};

use gpui_rsc::{component, runtime::Definition};

pub fn definition() -> Definition {
    let methods = ["V60", "French press", "AeroPress", "Espresso"];
    component! {
        name: "coffee-variables-form",
        imports: [],
        bindings: [
            in_out_param!("method"),
            in_out_param!("dose"),
            in_out_param!("water"),
            in_out_param!("grind"),
            in_out_param!("temperature"),
            in_out_param!("time"),
            in_out_param!("pours"),
            in_out_param!("stirs"),
            in_out_param!("swirls"),
            in_out_param!("filter"),
            out_param!("reset"),
        ],
    }
}
</script>
<!doctype html>
<html><head>
  <style>
    @media (max-width: 768px) {
      .controls-stack { width:100%; min-width:0; gap:16px; align-items:center; }
      .mobile-card { width:100%; max-width:560px; min-width:0; padding:18px; }
    }
    .controls-stack { display:flex; flex-direction:column; gap:20px; flex:1; min-width:360px; }
    .form-card { display:flex; flex-direction:column; gap:18px; background:#211c17; border:1px solid #3b3128; border-radius:18px; padding:22px; }
    .card-heading { display:flex; flex-direction:column; gap:4px; }
    .recipe-title { font-size:18px; font-weight:700; color:#f8f1e8; }
    .card-description { font-size:12px; color:#9f9284; }
    .field { display:flex; flex-direction:column; gap:7px; }
    .field-label { font-size:14px; font-weight:600; }
    .field-select { width:100%; }
    .field-help { font-size:11px; color:#988c7e; }
    .field-row { display:flex; justify-content:space-between; }
    .field-value { font-size:14px; font-weight:600; color:#f0b46b; }
    .actions-title { font-size:18px; font-weight:700; }
    .reset-button { background:#3e2d20; color:#f6e4cf; border-radius:9px; padding:9px 14px; }
  </style>
</head><body>
      <div class="controls-stack">
        <section class="mobile-card recipe-card form-card">
          <div class="card-heading">
            <h2 class="recipe-title">Brew recipe</h2>
            <p class="card-description">The ingredients and brewing conditions</p>
          </div>
          <div class="field">
            <label class="field-label">Brewing method</label>
            <select id="method" data-in-out="method" class="field-select">
              {methods.iter().map(|&method| => <option value={method}>{method}</option>)}
            </select>
            <small class="field-help">Choose the brewer you used.</small>
          </div>
          <div class="field">
            <div class="field-row"><label class="field-label">Coffee dose</label><output data-in="dose" data-suffix=" g" class="field-value"></output></div>
            <input id="dose" type="range" min="8" max="40" step="1" value="20" data-in-out="dose">
            <small class="field-help">Ground coffee added to the brewer.</small>
          </div>
          <div class="field">
            <div class="field-row"><label class="field-label">Water amount</label><output data-in="water" data-suffix=" g" class="field-value"></output></div>
            <input id="water" type="range" min="100" max="600" step="10" value="300" data-in-out="water">
            <small class="field-help">Total water used in the brew.</small>
          </div>
          <div class="field">
            <div class="field-row"><label class="field-label">Grind size</label><output data-in="grind" data-suffix=" / 10" class="field-value"></output></div>
            <input id="grind" type="range" min="1" max="10" step="1" value="5" data-in-out="grind">
            <small class="field-help">Your grinder setting, from coarse to fine.</small>
          </div>
          <div class="field">
            <div class="field-row"><label class="field-label">Water temperature</label><output data-in="temperature" data-suffix="°C" class="field-value"></output></div>
            <input id="temperature" type="range" min="80" max="100" step="0.5" value="94" data-in-out="temperature">
            <small class="field-help">Temperature when water touched the coffee.</small>
          </div>
          <div class="field">
            <div class="field-row"><label class="field-label">Brew time</label><output data-in="time" data-format="duration" class="field-value"></output></div>
            <input id="time" type="range" min="30" max="600" step="15" value="180" data-in-out="time">
            <small class="field-help">Total contact time.</small>
          </div>
        </section>

        <section class="mobile-card actions-card form-card">
          <div class="card-heading"><h2 class="actions-title">What you did</h2><p class="card-description">Simple actions that affect movement through the bed</p></div>
          <div class="field"><div class="field-row"><label class="field-label">Number of pours</label><output data-in="pours" data-suffix=" pours" class="field-value"></output></div><input id="pours" type="range" min="1" max="8" step="1" value="3" data-in-out="pours"></div>
          <div class="field"><div class="field-row"><label class="field-label">Stirs</label><output data-in="stirs" class="field-value"></output></div><input id="stirs" type="range" min="0" max="8" step="1" value="0" data-in-out="stirs"></div>
          <div class="field"><div class="field-row"><label class="field-label">Swirls</label><output data-in="swirls" class="field-value"></output></div><input id="swirls" type="range" min="0" max="8" step="1" value="1" data-in-out="swirls"></div>
          <div class="field"><label class="field-label">Filter type</label><select id="filter" data-in-out="filter" class="field-select"><option value="Paper" selected>Paper</option><option value="Metal">Metal</option><option value="Cloth">Cloth</option></select></div>
          <button id="reset" data-out="reset" class="reset-button">Reset recipe</button>
        </section>
      </div>
</body></html>
