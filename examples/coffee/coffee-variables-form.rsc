<script>
use gpui_rsc::{in_out_param, out_param};

use gpui_rsc::{component, runtime::Definition};

pub fn definition() -> Definition {
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
  </style>
</head><body>
      <div class="controls-stack" style="display:flex; flex-direction:column; gap:20px; flex:1; min-width:360px">
        <section class="mobile-card recipe-card" style="display:flex; flex-direction:column; gap:18px; background:#211c17; border:1px solid #3b3128; border-radius:18px; padding:22px">
          <div style="display:flex; flex-direction:column; gap:4px">
            <h2 style="font-size:18px; font-weight:700; color:#f8f1e8">Brew recipe</h2>
            <p style="font-size:12px; color:#9f9284">The ingredients and brewing conditions</p>
          </div>
          <div style="display:flex; flex-direction:column; gap:7px">
            <label style="font-size:14px; font-weight:600">Brewing method</label>
            <select id="method" data-in-out="method" style="width:100%">
              <option value="V60" selected>V60</option>
              <option value="French press">French press</option>
              <option value="AeroPress">AeroPress</option>
              <option value="Espresso">Espresso</option>
            </select>
            <small style="font-size:11px; color:#988c7e">Choose the brewer you used.</small>
          </div>
          <div style="display:flex; flex-direction:column; gap:7px">
            <div style="display:flex; justify-content:space-between"><label style="font-size:14px; font-weight:600">Coffee dose</label><output data-in="dose" data-suffix=" g" style="font-size:14px; font-weight:600; color:#f0b46b"></output></div>
            <input id="dose" type="range" min="8" max="40" step="1" value="20" data-in-out="dose">
            <small style="font-size:11px; color:#988c7e">Ground coffee added to the brewer.</small>
          </div>
          <div style="display:flex; flex-direction:column; gap:7px">
            <div style="display:flex; justify-content:space-between"><label style="font-size:14px; font-weight:600">Water amount</label><output data-in="water" data-suffix=" g" style="font-size:14px; font-weight:600; color:#f0b46b"></output></div>
            <input id="water" type="range" min="100" max="600" step="10" value="300" data-in-out="water">
            <small style="font-size:11px; color:#988c7e">Total water used in the brew.</small>
          </div>
          <div style="display:flex; flex-direction:column; gap:7px">
            <div style="display:flex; justify-content:space-between"><label style="font-size:14px; font-weight:600">Grind size</label><output data-in="grind" data-suffix=" / 10" style="font-size:14px; font-weight:600; color:#f0b46b"></output></div>
            <input id="grind" type="range" min="1" max="10" step="1" value="5" data-in-out="grind">
            <small style="font-size:11px; color:#988c7e">Your grinder setting, from coarse to fine.</small>
          </div>
          <div style="display:flex; flex-direction:column; gap:7px">
            <div style="display:flex; justify-content:space-between"><label style="font-size:14px; font-weight:600">Water temperature</label><output data-in="temperature" data-suffix="°C" style="font-size:14px; font-weight:600; color:#f0b46b"></output></div>
            <input id="temperature" type="range" min="80" max="100" step="0.5" value="94" data-in-out="temperature">
            <small style="font-size:11px; color:#988c7e">Temperature when water touched the coffee.</small>
          </div>
          <div style="display:flex; flex-direction:column; gap:7px">
            <div style="display:flex; justify-content:space-between"><label style="font-size:14px; font-weight:600">Brew time</label><output data-in="time" data-format="duration" style="font-size:14px; font-weight:600; color:#f0b46b"></output></div>
            <input id="time" type="range" min="30" max="600" step="15" value="180" data-in-out="time">
            <small style="font-size:11px; color:#988c7e">Total contact time.</small>
          </div>
        </section>

        <section class="mobile-card actions-card" style="display:flex; flex-direction:column; gap:18px; background:#211c17; border:1px solid #3b3128; border-radius:18px; padding:22px">
          <div style="display:flex; flex-direction:column; gap:4px"><h2 style="font-size:18px; font-weight:700">What you did</h2><p style="font-size:12px; color:#9f9284">Simple actions that affect movement through the bed</p></div>
          <div style="display:flex; flex-direction:column; gap:7px"><div style="display:flex; justify-content:space-between"><label style="font-size:14px; font-weight:600">Number of pours</label><output data-in="pours" data-suffix=" pours" style="font-size:14px; font-weight:600; color:#f0b46b"></output></div><input id="pours" type="range" min="1" max="8" step="1" value="3" data-in-out="pours"></div>
          <div style="display:flex; flex-direction:column; gap:7px"><div style="display:flex; justify-content:space-between"><label style="font-size:14px; font-weight:600">Stirs</label><output data-in="stirs" style="font-size:14px; font-weight:600; color:#f0b46b"></output></div><input id="stirs" type="range" min="0" max="8" step="1" value="0" data-in-out="stirs"></div>
          <div style="display:flex; flex-direction:column; gap:7px"><div style="display:flex; justify-content:space-between"><label style="font-size:14px; font-weight:600">Swirls</label><output data-in="swirls" style="font-size:14px; font-weight:600; color:#f0b46b"></output></div><input id="swirls" type="range" min="0" max="8" step="1" value="1" data-in-out="swirls"></div>
          <div style="display:flex; flex-direction:column; gap:7px"><label style="font-size:14px; font-weight:600">Filter type</label><select id="filter" data-in-out="filter" style="width:100%"><option value="Paper" selected>Paper</option><option value="Metal">Metal</option><option value="Cloth">Cloth</option></select></div>
          <button id="reset" data-out="reset" style="background:#3e2d20; color:#f6e4cf; border-radius:9px; padding:9px 14px">Reset recipe</button>
        </section>
      </div>
</body></html>
