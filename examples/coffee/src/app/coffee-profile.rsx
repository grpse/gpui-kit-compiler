pub fn CoffeeProfile(acidity: f32, target_acidity: &mut f32, sweetness: f32, target_sweetness: &mut f32, bitterness: f32, target_bitterness: &mut f32, body: f32, target_body: &mut f32, clarity: f32, target_clarity: &mut f32, astringency: f32, target_astringency: &mut f32, intensity: f32, target_intensity: &mut f32, notes: &str, ratio: &str, extraction_signal: &str) -> gpui::AnyElement {
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
      <div class={myStyles.ratioBadge}>{ratio}</div>
    </div>
    <div class={myStyles.scorePanel}>
      <div class={myStyles.scoreRow}><div class={myStyles.scoreHeading}><div>Acidity</div><div>{acidity}</div></div><input id="flavor-acidity" type="range" min="0" max="100" step="1" value={target_acidity} class={myStyles.scoreSlider}></div>
      <div class={myStyles.scoreRow}><div class={myStyles.scoreHeading}><div>Sweetness</div><div>{sweetness}</div></div><input id="flavor-sweetness" type="range" min="0" max="100" step="1" value={target_sweetness} class={myStyles.scoreSlider}></div>
      <div class={myStyles.scoreRow}><div class={myStyles.scoreHeading}><div>Bitterness</div><div>{bitterness}</div></div><input id="flavor-bitterness" type="range" min="0" max="100" step="1" value={target_bitterness} class={myStyles.scoreSlider}></div>
      <div class={myStyles.scoreRow}><div class={myStyles.scoreHeading}><div>Body</div><div>{body}</div></div><input id="flavor-body" type="range" min="0" max="100" step="1" value={target_body} class={myStyles.scoreSlider}></div>
      <div class={myStyles.scoreRow}><div class={myStyles.scoreHeading}><div>Clarity</div><div>{clarity}</div></div><input id="flavor-clarity" type="range" min="0" max="100" step="1" value={target_clarity} class={myStyles.scoreSlider}></div>
      <div class={myStyles.scoreRow}><div class={myStyles.scoreHeading}><div>Astringency</div><div>{astringency}</div></div><input id="flavor-astringency" type="range" min="0" max="100" step="1" value={target_astringency} class={myStyles.scoreSlider}></div>
      <div class={myStyles.scoreRow}><div class={myStyles.scoreHeading}><div>Intensity</div><div>{intensity}</div></div><input id="flavor-intensity" type="range" min="0" max="100" step="1" value={target_intensity} class={myStyles.scoreSlider}></div>
    </div>
    <div class={myStyles.notes}><div class={myStyles.notesTitle}>Likely impressions</div><div class={myStyles.notesValue}>{notes}</div></div>
    <div class={myStyles.extractionRow}><div class={myStyles.extractionLabel}>Extraction signal</div><div class={myStyles.extractionValue}>{extraction_signal}</div></div>
    <div class={myStyles.footnote}>Recipe-only estimate. Coffee origin, roast, and tasting feedback are not included yet.</div>
  </div>
}
