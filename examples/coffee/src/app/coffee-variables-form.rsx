pub fn CoffeeVariablesForm(method: &mut String, section: &str, dose: &mut f32, water: &mut f32, grind: &mut f32, temperature: &mut f32, time: &mut f32, pours: &mut f32, stirs: &mut f32, swirls: &mut f32, filter: &mut String) -> gpui::AnyElement {
  let methods = ["V60", "French press", "AeroPress", "Espresso"];
  let options = methods.iter().map(|&method| => <option value={method}>{method}</option>);
  let myStyles = styles({
      controlsStack: {
          display: "flex", flexDirection: "column",
          gap: if context.viewport_width <= 768.0 { 16.0 } else { 20.0 },
          flexShrink: 0, width: gpui::Length::Percent(1.0), minWidth: 0.0,
          alignItems: "stretch"
      },
      recipeCard: {
          display: "flex", flexDirection: "column", flexShrink: 0,
          width: gpui::Length::Percent(1.0), minWidth: 0.0,
          gap: if context.viewport_width <= 768.0 { 16.0 } else { 18.0 },
          backgroundColor: rgba(33.0 / 255.0, 28.0 / 255.0, 23.0 / 255.0),
          border: (1.0, rgba(59.0 / 255.0, 49.0 / 255.0, 40.0 / 255.0)),
          borderRadius: 18.0,
          padding: if context.viewport_width <= 768.0 { 18.0 } else { 22.0 }
      },
      actionsCard: {
          display: "flex", flexDirection: "column", flexShrink: 0,
          width: gpui::Length::Percent(1.0), minWidth: 0.0,
          gap: if context.viewport_width <= 768.0 { 16.0 } else { 18.0 },
          backgroundColor: rgba(33.0 / 255.0, 28.0 / 255.0, 23.0 / 255.0),
          border: (1.0, rgba(59.0 / 255.0, 49.0 / 255.0, 40.0 / 255.0)),
          borderRadius: 18.0,
          padding: if context.viewport_width <= 768.0 { 18.0 } else { 22.0 }
      },
      cardHeading: { display: "flex", flexDirection: "column", gap: 4.0 },
      recipeTitle: { fontSize: 18.0, fontWeight: 700, color: rgba(248.0 / 255.0, 241.0 / 255.0, 232.0 / 255.0) },
      cardDescription: { fontSize: 12.0, color: rgba(159.0 / 255.0, 146.0 / 255.0, 132.0 / 255.0) },
      field: { display: "flex", flexDirection: "column", gap: 7.0 },
      fieldLabel: { fontSize: 14.0, fontWeight: 600 },
      fieldSelect: { width: gpui::Length::Percent(1.0) },
      fieldHelp: { fontSize: 11.0, color: rgba(152.0 / 255.0, 140.0 / 255.0, 126.0 / 255.0) },
      fieldRow: { display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: 8.0 },
      fieldValue: { fontSize: 14.0, fontWeight: 600, color: rgba(240.0 / 255.0, 180.0 / 255.0, 107.0 / 255.0) },
      actionsTitle: { fontSize: 18.0, fontWeight: 700 }
  });

  <div class={myStyles.controlsStack}>
    {if section == "recipe" {
    <div class={myStyles.recipeCard}>
      <div class={myStyles.cardHeading}>
        <div class={myStyles.recipeTitle}>Brew recipe</div>
        <div class={myStyles.cardDescription}>The ingredients and brewing conditions</div>
      </div>
      <div class={myStyles.field}>
        <div class={myStyles.fieldLabel}>Brewing method</div>
        <select id="method" value={method} class={myStyles.fieldSelect}>
          {options}
        </select>
        <div class={myStyles.fieldHelp}>Choose the brewer you used.</div>
      </div>
      <div class={myStyles.field}>
        <div class={myStyles.fieldRow}><div class={myStyles.fieldLabel}>Coffee dose</div><div class={myStyles.fieldValue}>{dose} g</div></div>
        <input id="dose" type="range" min="8" max="40" step="1" value={dose}>
        <div class={myStyles.fieldHelp}>Ground coffee added to the brewer.</div>
      </div>
      <div class={myStyles.field}>
        <div class={myStyles.fieldRow}><div class={myStyles.fieldLabel}>Water amount</div><div class={myStyles.fieldValue}>{water} g</div></div>
        <input id="water" type="range" min="100" max="600" step="10" value={water}>
        <div class={myStyles.fieldHelp}>Total water used in the brew.</div>
      </div>
      <div class={myStyles.field}>
        <div class={myStyles.fieldRow}>
          <div class={myStyles.fieldLabel}>Grind size</div>
          <div class={myStyles.fieldValue}>{grind}/10</div>
        </div>
        <input id="grind" type="range" min="1" max="10" step="1" value={grind}>
        <div class={myStyles.fieldHelp}>Your grinder setting, from coarse to fine.</div>
      </div>
      <div class={myStyles.field}>
        <div class={myStyles.fieldRow}><div class={myStyles.fieldLabel}>Water temperature</div><div class={myStyles.fieldValue}>{temperature}°C</div></div>
        <input id="temperature" type="range" min="80" max="100" step="0.5" value={temperature}>
        <div class={myStyles.fieldHelp}>Temperature when water touched the coffee.</div>
      </div>
      <div class={myStyles.field}>
        <div class={myStyles.fieldRow}><div class={myStyles.fieldLabel}>Brew time</div><output data-in="time" data-format="duration" class={myStyles.fieldValue}></output></div>
        <input id="time" type="range" min="30" max="600" step="15" value={time}>
        <div class={myStyles.fieldHelp}>Total contact time.</div>
      </div>
    </div>
    } else {
    <div class={myStyles.actionsCard}>
      <div class={myStyles.cardHeading}><div class={myStyles.actionsTitle}>What you did</div><div class={myStyles.cardDescription}>Simple actions that affect movement through the bed</div></div>
      <div class={myStyles.field}><div class={myStyles.fieldRow}><div class={myStyles.fieldLabel}>Number of pours</div><div class={myStyles.fieldValue}>{pours} pours</div></div><input id="pours" type="range" min="1" max="8" step="1" value={pours}></div>
      <div class={myStyles.field}><div class={myStyles.fieldRow}><div class={myStyles.fieldLabel}>Stirs</div><div class={myStyles.fieldValue}>{stirs}</div></div><input id="stirs" type="range" min="0" max="8" step="1" value={stirs}></div>
      <div class={myStyles.field}><div class={myStyles.fieldRow}><div class={myStyles.fieldLabel}>Swirls</div><div class={myStyles.fieldValue}>{swirls}</div></div><input id="swirls" type="range" min="0" max="8" step="1" value={swirls}></div>
      <div class={myStyles.field}><div class={myStyles.fieldLabel}>Filter type</div><select id="filter" value={filter} class={myStyles.fieldSelect}><option value="Paper" selected>Paper</option><option value="Metal">Metal</option><option value="Cloth">Cloth</option></select></div>
    </div>
    }}
  </div>
}
