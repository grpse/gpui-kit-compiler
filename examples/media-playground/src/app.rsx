use gpui_rsc::runtime::{signal, Signal, StyleContext};
use crate::generated::native::{NativeBadge, VideoPanel};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CAT: AtomicU64 = AtomicU64::new(1);

pub fn title() -> &'static str { "Media Playground" }

fn cat_url(nonce: u64) -> String {
    format!("https://cataas.com/cat?type=medium&width=640&height=420&fresh={nonce}")
}

fn next_cat(cat: &Signal) {
    let nonce = NEXT_CAT.fetch_add(1, Ordering::Relaxed);
    cat.set(cat_url(nonce));
}

fn tab_color(context: &StyleContext<'_>, tab: &str, channel: usize) -> f32 {
    let active = context.props.get("active_tab").is_some_and(|value| value.text() == tab);
    let color = if active { [69.0, 176.0, 158.0] } else { [59.0, 75.0, 89.0] };
    color[channel] / 255.0
}

pub fn App() -> gpui::AnyElement {
    let cat = signal(cat_url(0));
    let active_tab = signal("video");
    let mediaStyles = styles({
        page: {
            width: gpui::Length::Percent(1.0), minWidth: 0.0,
            backgroundColor: rgba(14.0 / 255.0, 20.0 / 255.0, 29.0 / 255.0),
            textColor: rgba(235.0 / 255.0, 242.0 / 255.0, 248.0 / 255.0)
        },
        content: {
            width: gpui::Length::Percent(1.0), maxWidth: 1140.0, margin: "auto",
            display: "flex", flexDirection: "column", gap: 26.0,
            padding: (30.0, 28.0)
        },
        eyebrow: {
            fontSize: 11.0, fontWeight: 700,
            color: rgba(96.0 / 255.0, 211.0 / 255.0, 192.0 / 255.0)
        },
        heading: { fontSize: 32.0, fontWeight: 700 },
        subtitle: {
            fontSize: 14.0,
            color: rgba(156.0 / 255.0, 173.0 / 255.0, 190.0 / 255.0)
        },
        intro: { display: "flex", flexDirection: "column", gap: 8.0 },
        cards: {
            display: "flex",
            flexDirection: "row",
            gap: 20.0, width: gpui::Length::Percent(1.0), minWidth: 0.0
        },
        card: {
            display: "flex", flexDirection: "column", flex: 1, minWidth: 0.0,
            gap: 16.0, padding: 18.0, borderRadius: 18.0,
            backgroundColor: rgba(25.0 / 255.0, 34.0 / 255.0, 46.0 / 255.0),
            border: (1.0, rgba(59.0 / 255.0, 75.0 / 255.0, 89.0 / 255.0))
        },
        cardHeader: { display: "flex", flexDirection: "column", gap: 5.0 },
        label: {
            fontSize: 11.0, fontWeight: 700,
            color: rgba(96.0 / 255.0, 211.0 / 255.0, 192.0 / 255.0)
        },
        cardTitle: { fontSize: 20.0, fontWeight: 700 },
        cardHint: {
            fontSize: 12.0,
            color: rgba(156.0 / 255.0, 173.0 / 255.0, 190.0 / 255.0)
        },
        media: {
            width: gpui::Length::Percent(1.0), height: gpui::px(270.0),
            borderRadius: 12.0,
            backgroundColor: rgba(11.0 / 255.0, 16.0 / 255.0, 24.0 / 255.0)
        },
        action: {
            backgroundColor: rgba(69.0 / 255.0, 176.0 / 255.0, 158.0 / 255.0),
            color: rgba(11.0 / 255.0, 24.0 / 255.0, 29.0 / 255.0),
            fontSize: 13.0, fontWeight: 700,
            padding: (11.0, 16.0), borderRadius: 10.0
        },
        videoTab: {
            padding: (11.0, 16.0), borderRadius: 10.0, fontWeight: 700,
            backgroundColor: rgba(tab_color(context, "video", 0), tab_color(context, "video", 1), tab_color(context, "video", 2))
        },
        catsTab: {
            padding: (11.0, 16.0), borderRadius: 10.0, fontWeight: 700,
            backgroundColor: rgba(tab_color(context, "cats", 0), tab_color(context, "cats", 1), tab_color(context, "cats", 2))
        },
        footer: {
            fontSize: 12.0,
            color: rgba(128.0 / 255.0, 148.0 / 255.0, 164.0 / 255.0)
        }
    });

    <div class={mediaStyles.page}>
        <div class={mediaStyles.content}>
            <div class={mediaStyles.intro}>
                <div class={mediaStyles.eyebrow}>GPUI KIT · MEDIA DEMO</div>
                <div class={mediaStyles.heading}>Media Playground</div>
                <div class={mediaStyles.subtitle}>Play a local video and load a new cat photo from the web.</div>
            </div>
            <div class={mediaStyles.cards}>
                <button id="tab-video" on-click={active_tab.set("video")} class={mediaStyles.videoTab}>Video</button>
                <button id="tab-cats" on-click={active_tab.set("cats")} class={mediaStyles.catsTab}>Cat Photos</button>
            </div>
            {if active_tab == "video" {
                <div class={mediaStyles.card}>
                    <div class={mediaStyles.cardTitle}>Video player</div>
                    <div class={mediaStyles.cardHint}>Choose a local video, then use the playback controls below.</div>
                    <VideoPanel />
                </div>
            } else {
                <div class={mediaStyles.card}>
                    <div class={mediaStyles.cardHeader}>
                        <div class={mediaStyles.label}>LIVE CAT PHOTO</div>
                        <div class={mediaStyles.cardTitle}>Random cat</div>
                        <div class={mediaStyles.cardHint}>Each click requests a new image from CATAAS.</div>
                    </div>
                    <img id="cat-photo" src={cat} alt="Random cat from CATAAS" width="480" height="270" object-fit="cover" class={mediaStyles.media} />
                    <button id="next-cat" on-click={next_cat(&cat)} class={mediaStyles.action}>Show another cat</button>
                </div>
            }}
            <NativeBadge />
            <div class={mediaStyles.footer}>Video playback requires GStreamer. Cat photos require an internet connection.</div>
        </div>
    </div>
}
