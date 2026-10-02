use crate::native_preview::ExtractionIllustration;

pub fn ExtractionPreviews(water: f32, intensity: f32, bitterness: f32, body: f32, method: &str) -> gpui::AnyElement {
    let myStyles = styles({
        gallery: { display: "flex", flex: 1, width: gpui::Length::Percent(1.0), height: gpui::Length::Percent(1.0), minWidth: 0.0 }
    });
    <div class={myStyles.gallery}>
        <ExtractionIllustration method={method} water={water} intensity={intensity} bitterness={bitterness} body={body} />
    </div>
}
