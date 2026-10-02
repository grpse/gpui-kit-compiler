pub fn CoffeeProfile(acidity: &mut f32, sweetness: &mut f32, bitterness: &mut f32, body: &mut f32, clarity: &mut f32, astringency: &mut f32, intensity: &mut f32, notes: &str, ratio: &str, extraction_signal: &str) -> gpui::AnyElement {
let myStyles = styles({
    profileCard: {
        display: "flex", flexDirection: "column", gap: if context.viewport_width <= 768.0 { 16.0 } else { 20.0 }, flex: 1,
        width: gpui::Length::Percent(1.0), minWidth: 0.0,
        backgroundColor: rgba(249.0 / 255.0, 245.0 / 255.0, 239.0 / 255.0),
        textColor: rgba(45.0 / 255.0, 37.0 / 255.0, 31.0 / 255.0),
        borderRadius: 22.0,
        padding: if context.viewport_width <= 768.0 { 18.0 } else { 24.0 }
    },
    profileHeader: { display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12.0 },
    profileHeading: { display: "flex", flexDirection: "column", gap: 4.0 },
    profileTitle: { fontSize: 20.0, fontWeight: 700 },
    profileHelp: { fontSize: 11.0, color: rgba(139.0 / 255.0, 125.0 / 255.0, 112.0 / 255.0) },
    ratioBadge: { backgroundColor: rgba(238.0 / 255.0, 229.0 / 255.0, 217.0 / 255.0), borderRadius: 99.0, padding: (5.0, 12.0), fontSize: 11.0, color: rgba(118.0 / 255.0, 91.0 / 255.0, 67.0 / 255.0), fontWeight: 600 },
    scorePanel: { display: "flex", flexDirection: "column", gap: 17.0, backgroundColor: rgba(1.0, 1.0, 1.0), borderRadius: 14.0, padding: 18.0 },
    scoreRow: { display: "flex", flexDirection: "column", gap: 6.0 },
    scoreHeading: { display: "flex", justifyContent: "space-between" },
    scoreSlider: { width: gpui::Length::Percent(1.0) },
    notes: { display: "flex", flexDirection: "column", gap: 8.0 },
    notesTitle: { fontSize: 13.0, fontWeight: 700 },
    notesValue: { fontSize: 12.0, color: rgba(118.0 / 255.0, 91.0 / 255.0, 67.0 / 255.0) },
    extractionRow: { display: "flex", justifyContent: "space-between", backgroundColor: rgba(238.0 / 255.0, 229.0 / 255.0, 217.0 / 255.0), borderRadius: 13.0, padding: (12.0, 16.0) },
    extractionLabel: { fontSize: 11.0, color: rgba(116.0 / 255.0, 103.0 / 255.0, 92.0 / 255.0) },
    extractionValue: { fontSize: 12.0, fontWeight: 600, color: rgba(83.0 / 255.0, 62.0 / 255.0, 44.0 / 255.0) },
    footnote: { fontSize: 10.0, color: rgba(148.0 / 255.0, 135.0 / 255.0, 123.0 / 255.0) }
});
  <div class={myStyles.profileCard}>
    <div class={myStyles.profileHeader}>
      <div class={myStyles.profileHeading}><div class={myStyles.profileTitle}>Cup prediction</div><div class={myStyles.profileHelp}>Drag a score to adjust the recipe · 0–100</div></div>
      <output data-in="ratio" data-format="ratio" class={myStyles.ratioBadge}></output>
    </div>
    <div class={myStyles.scorePanel}>
      <div class={myStyles.scoreRow}><div class={myStyles.scoreHeading}><div>Acidity</div><output data-in="acidity" data-format="score"></output></div><input id="flavor-acidity" type="range" min="0" max="100" step="1" value="64" data-in-out="acidity" class={myStyles.scoreSlider}></div>
      <div class={myStyles.scoreRow}><div class={myStyles.scoreHeading}><div>Sweetness</div><output data-in="sweetness" data-format="score"></output></div><input id="flavor-sweetness" type="range" min="0" max="100" step="1" value="68" data-in-out="sweetness" class={myStyles.scoreSlider}></div>
      <div class={myStyles.scoreRow}><div class={myStyles.scoreHeading}><div>Bitterness</div><output data-in="bitterness" data-format="score"></output></div><input id="flavor-bitterness" type="range" min="0" max="100" step="1" value="23" data-in-out="bitterness" class={myStyles.scoreSlider}></div>
      <div class={myStyles.scoreRow}><div class={myStyles.scoreHeading}><div>Body</div><output data-in="body" data-format="score"></output></div><input id="flavor-body" type="range" min="0" max="100" step="1" value="57" data-in-out="body" class={myStyles.scoreSlider}></div>
      <div class={myStyles.scoreRow}><div class={myStyles.scoreHeading}><div>Clarity</div><output data-in="clarity" data-format="score"></output></div><input id="flavor-clarity" type="range" min="0" max="100" step="1" value="83" data-in-out="clarity" class={myStyles.scoreSlider}></div>
      <div class={myStyles.scoreRow}><div class={myStyles.scoreHeading}><div>Astringency</div><output data-in="astringency" data-format="score"></output></div><input id="flavor-astringency" type="range" min="0" max="100" step="1" value="15" data-in-out="astringency" class={myStyles.scoreSlider}></div>
      <div class={myStyles.scoreRow}><div class={myStyles.scoreHeading}><div>Intensity</div><output data-in="intensity" data-format="score"></output></div><input id="flavor-intensity" type="range" min="0" max="100" step="1" value="64" data-in-out="intensity" class={myStyles.scoreSlider}></div>
    </div>
    <div class={myStyles.notes}><div class={myStyles.notesTitle}>Likely impressions</div><output data-in="notes" class={myStyles.notesValue}></output></div>
    <div class={myStyles.extractionRow}><div class={myStyles.extractionLabel}>Extraction signal</div><output data-in="extraction_signal" class={myStyles.extractionValue}></output></div>
    <div class={myStyles.footnote}>Recipe-only estimate. Coffee origin, roast, and tasting feedback are not included yet.</div>
  </div>
}
