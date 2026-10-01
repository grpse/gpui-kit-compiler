<script>
use gpui_rsc::{component, in_param};
use gpui_rsc::runtime::Definition;
use crate::generated::{aeropress_preview, espresso_preview, french_press_preview, v60_preview};

pub fn definition() -> Definition {
    component! {
        name: "extraction-previews",
        imports: [
            v60_preview::V60PreviewComponent::definition(),
            french_press_preview::FrenchPressPreviewComponent::definition(),
            aeropress_preview::AeropressPreviewComponent::definition(),
            espresso_preview::EspressoPreviewComponent::definition(),
        ],
        bindings: [
            in_param!("water"),
            in_param!("intensity"),
            in_param!("bitterness"),
            in_param!("body"),
            in_param!("method"),
        ],
    }
    .with_template(selected_preview())
}

fn selected_preview() -> gpui_rsc::TemplateElement {
    let preview = {if method == "V60" {
        <div class="preview-gallery"><component class="method-preview" name="v60-preview" water="[water]" intensity="[intensity]" bitterness="[bitterness]" body="[body]" /></div>
    } else {
        {if method == "French press" {
            <div class="preview-gallery"><component class="method-preview" name="french-press-preview" water="[water]" intensity="[intensity]" bitterness="[bitterness]" body="[body]" /></div>
        } else {
            {if method == "AeroPress" {
                <div class="preview-gallery"><component class="method-preview" name="aeropress-preview" water="[water]" intensity="[intensity]" bitterness="[bitterness]" body="[body]" /></div>
            } else {
                <div class="preview-gallery"><component class="method-preview" name="espresso-preview" water="[water]" intensity="[intensity]" bitterness="[bitterness]" body="[body]" /></div>
            }}
        }}
    }};
    preview
}

</script>
<!doctype html>
<html><head>
  <style>
    .preview-gallery { display:flex; flex-direction:column; width:100%; min-width:0; }
    .method-preview { display:flex; flex:1; width:100%; min-width:0; }
  </style>
</head><body>
  <div class="preview-gallery"></div>
</body></html>
