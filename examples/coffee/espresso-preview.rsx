fn style_strength(intensity: f32, bitterness: f32, body: f32) -> f32 {
    ((intensity * 0.5 + bitterness * 0.3 + body * 0.2) / 100.0).clamp(0.0, 1.0)
}

fn coffee_channel(intensity: f32, bitterness: f32, body: f32, from: f32, to: f32) -> f32 {
    (from + (to - from) * style_strength(intensity, bitterness, body)).round() / 255.0
}

fn liquid_level(water: f32) -> f32 {
    (0.24 + ((water - 100.0) / 500.0).clamp(0.0, 1.0) * 0.62).clamp(0.2, 0.88)
}

pub fn EspressoPreview(water: f32, intensity: f32, bitterness: f32, body: f32) -> gpui::AnyElement {
    let myStyles = styles({
        card: {
            display: "flex",
            flexDirection: "column",
            flex: 1,
            width: gpui::Length::Percent(1.0),
            height: gpui::Length::Percent(1.0),
            minWidth: 0.0,
            gap: 12.0,
            padding: (18.0, 18.0, 18.0, 18.0),
            border: (1.0, rgba(59.0 / 255.0, 49.0 / 255.0, 40.0 / 255.0)),
            borderRadius: 16.0,
            backgroundColor: rgba(33.0 / 255.0, 28.0 / 255.0, 23.0 / 255.0),
            textColor: rgba(244.0 / 255.0, 236.0 / 255.0, 225.0 / 255.0)
        },
        coffee: { backgroundColor: rgba(coffee_channel(intensity, bitterness, body, 0xd8 as f32, 0x48 as f32), coffee_channel(intensity, bitterness, body, 0x90 as f32, 0x23 as f32), coffee_channel(intensity, bitterness, body, 0x4e as f32, 0x15 as f32)) },
        previewTitle: { fontSize: 14.0, fontWeight: 700, color: rgba(242.0 / 255.0, 193.0 / 255.0, 138.0 / 255.0) },
        previewCopy: { fontSize: 11.0, color: rgba(169.0 / 255.0, 155.0 / 255.0, 141.0 / 255.0) },
        stage: { display: "flex", flex: 1, flexDirection: "column", alignItems: "center", justifyContent: "flex-start", gap: 7.0, height: gpui::Length::from(gpui::px(320.0)), borderRadius: 12.0, backgroundColor: rgba(244.0 / 255.0, 230.0 / 255.0, 211.0 / 255.0) },
        machine: { width: gpui::Length::from(gpui::px(130.0)), height: gpui::Length::from(gpui::px(46.0)), borderRadius: 9.0, backgroundColor: rgba(89.0 / 255.0, 100.0 / 255.0, 93.0 / 255.0) },
        portafilter: { width: gpui::Length::from(gpui::px(76.0)), height: gpui::Length::from(gpui::px(9.0)), borderRadius: 99.0, backgroundColor: rgba(106.0 / 255.0, 85.0 / 255.0, 64.0 / 255.0) },
        espressoCup: { position: "relative", width: gpui::Length::from(gpui::px(56.0)), height: gpui::Length::from(gpui::px(56.0)), overflow: "hidden", border: (3.0, rgba(203.0 / 255.0, 182.0 / 255.0, 158.0 / 255.0)), borderRadius: 10.0, backgroundColor: rgba(1.0, 250.0 / 255.0, 242.0 / 255.0) },
        liquid: {
            position: "absolute",
            right: 2.0,
            bottom: 2.0,
            left: 2.0,
            borderRadius: 8.0,
            backgroundColor: rgba(coffee_channel(intensity, bitterness, body, 0xd8 as f32, 0x48 as f32), coffee_channel(intensity, bitterness, body, 0x90 as f32, 0x23 as f32), coffee_channel(intensity, bitterness, body, 0x4e as f32, 0x15 as f32)),
            height: gpui::Length::Percent(liquid_level(water))
        }
    });
<div class={myStyles.card}>
    <div><div class={myStyles.previewTitle}>Espresso · Pressure</div><div class={myStyles.previewCopy}>Hot water is forced through a compact puck of finely ground coffee.</div></div>
    <div class={myStyles.stage}>
      <div class={myStyles.machine}></div>
      <div class={myStyles.portafilter}></div>
      <div class={myStyles.espressoCup}><div class={myStyles.liquid}></div></div>
    </div>
  </div>
}
