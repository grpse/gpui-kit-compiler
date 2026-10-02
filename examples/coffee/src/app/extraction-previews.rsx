use crate::generated::aeropress_preview::AeropressPreview;
use crate::generated::espresso_preview::EspressoPreview;
use crate::generated::french_press_preview::FrenchPressPreview;

pub fn ExtractionPreviews(water: f32, intensity: f32, bitterness: f32, body: f32, method: &str) -> gpui::AnyElement {
let myStyles = styles({
    previewGallery: { display: "flex", flexDirection: "column", flex: 1, width: gpui::Length::Percent(1.0), height: gpui::Length::Percent(1.0), minWidth: 0.0 },
    methodPreview: { display: "flex", flex: 1, width: gpui::Length::Percent(1.0), height: gpui::Length::Percent(1.0), minWidth: 0.0 }
});
{if method == "V60" {
  <div class={myStyles.previewGallery}><V60Preview class={myStyles.methodPreview} water={water} intensity={intensity} bitterness={bitterness} body={body} /></div>
} else {
  {if method == "French press" {
    <div class={myStyles.previewGallery}><FrenchPressPreview class={myStyles.methodPreview} water={water} intensity={intensity} bitterness={bitterness} body={body} /></div>
  } else {
    {if method == "AeroPress" {
      <div class={myStyles.previewGallery}><AeropressPreview class={myStyles.methodPreview} water={water} intensity={intensity} bitterness={bitterness} body={body} /></div>
    } else {
      <div class={myStyles.previewGallery}><EspressoPreview class={myStyles.methodPreview} water={water} intensity={intensity} bitterness={bitterness} body={body} /></div>
    }}
  }}
}}
}

fn style_strength(intensity: f32, bitterness: f32, body: f32) -> f32 {
    ((intensity * 0.5 + bitterness * 0.3 + body * 0.2) / 100.0).clamp(0.0, 1.0)
}

fn coffee_channel(intensity: f32, bitterness: f32, body: f32, from: f32, to: f32) -> f32 {
    (from + (to - from) * style_strength(intensity, bitterness, body)).round() / 255.0
}

fn liquid_level(water: f32) -> f32 {
    (0.24 + ((water - 100.0) / 500.0).clamp(0.0, 1.0) * 0.62).clamp(0.2, 0.88)
}

pub fn V60Preview(water: f32, intensity: f32, bitterness: f32, body: f32) -> gpui::AnyElement {
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
        stage: { display: "flex", flex: 1, flexDirection: "column", alignItems: "center", justifyContent: "flex-end", gap: 8.0, height: gpui::Length::from(gpui::px(320.0)), borderRadius: 12.0, backgroundColor: rgba(244.0 / 255.0, 230.0 / 255.0, 211.0 / 255.0) },
        dripper: { width: gpui::Length::from(gpui::px(94.0)), height: gpui::Length::from(gpui::px(44.0)), borderRadius: 12.0, backgroundColor: rgba(coffee_channel(intensity, bitterness, body, 0xd8 as f32, 0x48 as f32), coffee_channel(intensity, bitterness, body, 0x90 as f32, 0x23 as f32), coffee_channel(intensity, bitterness, body, 0x4e as f32, 0x15 as f32)) },
        server: { position: "relative", width: gpui::Length::from(gpui::px(76.0)), height: gpui::Length::from(gpui::px(76.0)), overflow: "hidden", border: (3.0, rgba(203.0 / 255.0, 182.0 / 255.0, 158.0 / 255.0)), borderRadius: 12.0, backgroundColor: rgba(1.0, 250.0 / 255.0, 242.0 / 255.0) },
        waterline: { width: gpui::Length::from(gpui::px(82.0)), height: gpui::Length::from(gpui::px(4.0)), borderRadius: 99.0, backgroundColor: rgba(216.0 / 255.0, 197.0 / 255.0, 174.0 / 255.0) },
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
    <div><div class={myStyles.previewTitle}>V60 · Pour-over</div><div class={myStyles.previewCopy}>Water passes through a paper filter into a server.</div></div>
      <div class={myStyles.stage}>
      <div class={myStyles.dripper}></div>
      <div class={myStyles.waterline}></div>
      <div class={myStyles.server}><div class={myStyles.liquid}></div></div>
    </div>
  </div>
}
