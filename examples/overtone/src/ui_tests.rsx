use crate::{
    model::*,
    studio::{Action, Studio},
};
use gpui_kit::{
    point, px, size, test::TestWindowExt, AnyWindowHandle, App, AppContext, Bounds, Entity,
    HeadlessAppContext, TestAppContext, Window, WindowBounds, WindowOptions,
};

fn fixture(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Studio>) {
    cx.update(gpui_kit::init);
    cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(),
                    size: size(px(1480.), px(1500.)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Studio::new(None, window, cx)),
        )
        .unwrap()
    })
}
fn ui(handle: AnyWindowHandle, cx: &mut TestAppContext, f: impl FnOnce(&mut Window, &mut App)) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        f(window, cx);
    })
    .unwrap();
}
fn settings(
    handle: AnyWindowHandle,
    studio: &Entity<Studio>,
    cx: &mut TestAppContext,
) -> AnyWindowHandle {
    ui(handle, cx, |window, cx| window.press("ctrl-,", cx));
    let settings = studio.read_with(cx, |s, _| s.settings_window.expect("settings window"));
    ui(settings, cx, |window, cx| {
        window.resize(size(px(1480.), px(1100.)));
        window.bounds_changed(cx);
        window.render_frame(cx);
    });
    settings
}
#[gpui_kit::test]
fn keyboard_library_and_instance_controls_use_real_callbacks(cx: &mut TestAppContext) {
    let (handle, studio) = fixture(cx);
    ui(handle, cx, |window, cx| {
        window.click("save-sound", cx);
        window.click("add-draft", cx);
        window.click("key-0", cx);
        window.click("key-4", cx);
        window.click("view-menu", cx);
        window.within("popup-menu").click(10usize, cx);
    });
    ui(handle, cx, |window, cx| {
        window.click("instance-noise-color", cx);
    });
    studio.read_with(cx, |s, _| {
        assert_eq!(s.project.library.len(), 1);
        assert_eq!(
            s.project.clips[0]
                .notes
                .iter()
                .map(|n| n.midi)
                .collect::<Vec<_>>(),
            vec![60, 64]
        );
        assert_eq!(s.project.clips[0].sound.noise.color, NoiseColor::Pink);
        assert_eq!(s.project.library[0].noise.color, NoiseColor::White);
        assert!(s.dirty());
        s.project.validate().unwrap();
    });
    ui(handle, cx, |window, cx| {
        window.click("reset-instance", cx);
        window.click("duplicate-clip", cx);
    });
    studio.read_with(cx, |s, _| {
        assert_eq!(s.project.clips.len(), 2);
        assert_eq!(s.project.clips[1].sound.noise.color, NoiseColor::White);
        assert_eq!(s.project.clips[1].start_seconds, 2.);
    });
}
#[gpui_kit::test]
fn profiles_can_be_added_and_reconfigured_without_changing_other_layouts(cx: &mut TestAppContext) {
    let (handle, studio) = fixture(cx);
    let settings = settings(handle, &studio, cx);
    ui(settings, cx, |window, cx| {
        window.click("copy-profile", cx);
        window.click("dock-config-Library", cx);
        window.click("show-Source", cx);
    });
    studio.read_with(cx, |s, _| {
        assert_eq!(s.project.profiles.len(), 4);
        let original = &s.project.profiles[0];
        let active = s.project.profile();
        assert_eq!(
            original
                .panels
                .iter()
                .find(|p| p.module == Module::Library)
                .unwrap()
                .dock,
            Dock::Left
        );
        assert_eq!(
            active
                .panels
                .iter()
                .find(|p| p.module == Module::Library)
                .unwrap()
                .dock,
            Dock::Main
        );
        assert!(
            original
                .panels
                .iter()
                .find(|p| p.module == Module::Source)
                .unwrap()
                .visible
        );
        assert!(
            !active
                .panels
                .iter()
                .find(|p| p.module == Module::Source)
                .unwrap()
                .visible
        );
        s.project.validate().unwrap();
    });
}
#[gpui_kit::test]
fn linked_tempo_edits_start_from_session_bpm(cx: &mut TestAppContext) {
    let (handle, studio) = fixture(cx);
    ui(handle, cx, |window, cx| {
        window.click("session-tempo-plus", cx);
        window.click("tempo-1-plus", cx);
    });
    studio.read_with(cx, |s, _| {
        assert_eq!(s.project.session_bpm, 121.);
        assert_eq!(s.project.tracks[0].bpm, 122.);
        assert!(!s.project.tracks[0].follow_session);
    });
}
#[gpui_kit::test]
fn dividers_resize_docks_panels_and_proportions_per_profile(cx: &mut TestAppContext) {
    let (handle, studio) = fixture(cx);
    ui(handle, cx, |window, cx| {
        let p = window.find("resize-left-dock").bounds().center();
        window.drag(p, point(p.x + px(60.), p.y), cx);
        let p = window.find("resize-right-dock").bounds().center();
        window.drag(p, point(p.x - px(30.), p.y), cx);
        let p = window.find("resize-lower-dock").bounds().center();
        window.drag(p, point(p.x, p.y - px(50.)), cx);
        let p = window
            .find("resize-pair-Harmonics-Controls")
            .bounds()
            .center();
        window.drag(p, point(p.x + px(45.), p.y), cx);
        let p = window.find("resize-panel-Source").bounds().center();
        window.drag(p, point(p.x, p.y + px(50.)), cx);
    });
    studio.read_with(cx, |s, _| {
        let p = s.project.profile();
        assert_eq!(p.left_width, 318.);
        assert_eq!(p.right_width, 318.);
        assert_eq!(p.lower_height, 400.);
        assert!(
            p.panels
                .iter()
                .find(|p| p.module == Module::Harmonics)
                .unwrap()
                .share
                > 0.5
        );
        assert!(p
            .panels
            .iter()
            .find(|p| p.module == Module::Source)
            .unwrap()
            .height
            .is_some());
        assert_eq!(s.project.profiles[1].left_width, 258.);
        assert_eq!(s.project.profiles[1].lower_height, 350.);
        s.project.validate().unwrap();
        let loaded = crate::storage::decode(&crate::storage::encode(&s.project).unwrap()).unwrap();
        assert_eq!(loaded, s.project);
    });
    let settings = settings(handle, &studio, cx);
    ui(settings, cx, |window, cx| {
        window.click("panel-spacing-plus", cx);
        window.click("dock-spacing-plus", cx);
        window.click("panel-padding-plus", cx);
        window.click("auto-height-Source", cx);
    });
    studio.read_with(cx, |s, _| {
        let p = s.project.profile();
        assert_eq!(p.panel_gap, 12.);
        assert_eq!(p.dock_gap, 10.);
        assert_eq!(p.panel_padding, 14.);
        assert!(p
            .panels
            .iter()
            .find(|p| p.module == Module::Source)
            .unwrap()
            .height
            .is_none());
    });
}
#[gpui_kit::test]
fn workspace_settings_dialogs_preserve_the_current_composition(cx: &mut TestAppContext) {
    let (handle, studio) = fixture(cx);
    let path = std::env::temp_dir().join(format!(
        "overtone-settings-{}.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    ui(handle, cx, |window, cx| window.click("add-draft", cx));
    let settings = settings(handle, &studio, cx);
    ui(settings, cx, |window, cx| {
        window.click("panel-spacing-plus", cx);
        window.click("export-settings", cx);
    });
    assert!(cx.did_prompt_for_new_path());
    cx.simulate_new_path_selection(|_| Some(path.clone()));
    cx.run_until_parked();
    let music = studio.read_with(cx, |s, _| {
        assert!(!s.busy);
        s.project.clips.clone()
    });
    ui(settings, cx, |window, cx| {
        window.click("panel-spacing-plus", cx);
        window.click("import-settings", cx);
    });
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(|_| Some(vec![path.clone()]));
    cx.run_until_parked();
    studio.read_with(cx, |s, _| {
        assert_eq!(s.project.clips, music);
        assert_eq!(s.project.profile().panel_gap, 12.);
        assert!(!s.busy);
        assert!(s.dirty());
        s.project.validate().unwrap();
    });
    std::fs::remove_file(path).unwrap();
}
#[gpui_kit::test]
fn alt_drag_spacing_and_compact_lower_resize_follow_the_pointer(cx: &mut TestAppContext) {
    use gpui_kit::{
        InputEvent, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    };
    let (handle, studio) = fixture(cx);
    ui(handle, cx, |window, cx| {
        for (id, dx, dy) in [
            ("resize-pair-Harmonics-Controls", 6., 0.),
            ("resize-left-dock", 4., 0.),
        ] {
            let origin = window.find(id).bounds().center();
            let end = origin + point(px(dx), px(dy));
            let modifiers = Modifiers {
                alt: true,
                ..Default::default()
            };
            window.dispatch_event(
                MouseMoveEvent {
                    position: origin,
                    pressed_button: None,
                    modifiers,
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            window.dispatch_event(
                MouseDownEvent {
                    button: MouseButton::Left,
                    position: origin,
                    modifiers,
                    click_count: 1,
                    first_mouse: false,
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            window.dispatch_event(
                MouseMoveEvent {
                    position: end,
                    pressed_button: Some(MouseButton::Left),
                    modifiers,
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            window.dispatch_event(
                MouseUpEvent {
                    button: MouseButton::Left,
                    position: end,
                    modifiers,
                    click_count: 1,
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
        }
    });
    studio.read_with(cx, |s, _| {
        assert_eq!(s.project.profile().panel_gap, 10.);
        assert_eq!(
            s.project
                .profile()
                .panels
                .iter()
                .find(|p| p.module == Module::Harmonics)
                .unwrap()
                .pair_gap,
            Some(16.)
        );
        assert_eq!(s.project.profile().dock_gap, 12.);
        assert_eq!(s.project.profile().left_width, 258.);
    });
    ui(handle, cx, |window, cx| {
        window.click("view-menu", cx);
        window.within("popup-menu").click(10usize, cx);
    });
    ui(handle, cx, |window, cx| {
        let origin = window.find("resize-gap-Timeline").bounds().center();
        window.drag(origin, point(origin.x, origin.y + px(8.)), cx);
    });
    studio.read_with(cx, |s, _| {
        assert_eq!(
            s.project
                .profile()
                .panels
                .iter()
                .find(|p| p.module == Module::Timeline)
                .unwrap()
                .gap_after,
            Some(18.)
        );
        assert_eq!(s.project.profile().panel_gap, 10.);
        let loaded = crate::storage::decode(&crate::storage::encode(&s.project).unwrap()).unwrap();
        assert_eq!(loaded, s.project);
    });
    let settings = settings(handle, &studio, cx);
    ui(settings, cx, |window, cx| {
        window.click("default-gaps-Timeline", cx)
    });
    ui(handle, cx, |window, cx| {
        window.click("view-menu", cx);
        window.within("popup-menu").click(8usize, cx);
    });
    ui(handle, cx, |window, cx| {
        studio.update(cx, |s, cx| {
            s.project.profile_mut().lower_height = 700.;
            s.changed(cx);
        });
        window.resize(size(px(1000.), px(680.)));
        window.bounds_changed(cx);
        window.render_frame(cx);
        let origin = window.find("resize-lower-dock").bounds().center();
        window.drag(origin, point(origin.x, origin.y + px(30.)), cx);
    });
    studio.read_with(cx, |s, _| {
        assert_eq!(s.project.profile().lower_height, 400.);
        assert!(s.project.profiles[2]
            .panels
            .iter()
            .find(|p| p.module == Module::Timeline)
            .unwrap()
            .gap_after
            .is_none());
        s.project.validate().unwrap();
    });
}
#[gpui_kit::test]
fn dragging_panels_and_timeline_clips_changes_stored_placements(cx: &mut TestAppContext) {
    let (handle, studio) = fixture(cx);
    ui(handle, cx, |window, cx| {
        window.drag_to("handle-Library", "handle-Source", cx);
    });
    studio.read_with(cx, |s, _| {
        assert_eq!(
            s.project
                .profile()
                .panels
                .iter()
                .find(|p| p.module == Module::Library)
                .unwrap()
                .dock,
            Dock::Right
        )
    });
    ui(handle, cx, |window, cx| {
        window.click("add-draft", cx);
        window.click("view-menu", cx);
        window.within("popup-menu").click(10usize, cx);
    });
    ui(handle, cx, |window, cx| {
        window.drag_to("clip-10", "lane-2", cx);
    });
    studio.read_with(cx, |s, _| {
        assert_eq!(s.project.clips[0].track, 2);
        assert!(s.project.clips[0].start_seconds > 0.);
        s.project.validate().unwrap();
    });
}
#[gpui_kit::test]
fn knob_drag_and_unsaved_guard_work_without_audio(cx: &mut TestAppContext) {
    let (handle, studio) = fixture(cx);
    ui(handle, cx, |window, cx| {
        let origin = window.find("knob-false-34").bounds().center();
        window.drag(origin, point(origin.x, origin.y - px(42.)), cx);
        window.press("ctrl-n", cx);
    });
    studio.read_with(cx, |s, _| {
        assert!(s.project.draft.noise.level > 0.08);
        assert!(s.pending.is_some());
        assert!(s.dirty());
    });
    ui(handle, cx, |window, cx| {
        window.click("confirm-cancel", cx);
    });
    studio.read_with(cx, |s, _| assert!(s.pending.is_none()));
    let before = studio.read_with(cx, |s, _| s.project.clone());
    ui(handle, cx, |window, cx| {
        window.click("record-voice", cx);
        window.click("analyze-voice", cx);
    });
    studio.read_with(cx, |s, _| assert_eq!(s.project, before));
}

#[gpui_kit::test]
fn search_and_native_file_dialogs_save_and_restore_the_connected_project(cx: &mut TestAppContext) {
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let folder = Temp(std::env::temp_dir().join(format!(
            "overtone-ui-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    std::fs::create_dir_all(&folder.0).unwrap();
    let audio = folder.0.join("voice.wav");
    std::fs::write(&audio, b"opaque voice reference").unwrap();
    let file = folder.0.join("session.overtone");
    let (handle, studio) = fixture(cx);
    ui(handle, cx, |window, cx| {
        window.click("import-voice", cx);
    });
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(|_| Some(vec![audio.clone()]));
    cx.run_until_parked();
    studio.read_with(cx, |s, _| {
        assert_eq!(s.project.sources.len(), 1);
        assert!(s.project.draft.source.is_some());
        assert!(!s.busy);
    });
    ui(handle, cx, |window, cx| {
        window.click("save-sound", cx);
        window.click("sound-search", cx);
        window.input("no matching sound", cx);
    });
    studio.read_with(cx, |s, _| assert_eq!(s.query, "no matching sound"));
    ui(handle, cx, |window, _| {
        assert!(window.try_find("edit-11").is_none())
    });
    ui(handle, cx, |window, cx| {
        window.press("ctrl-s", cx);
    });
    assert!(cx.did_prompt_for_new_path());
    cx.simulate_new_path_selection(|_| Some(file.clone()));
    cx.run_until_parked();
    studio.read_with(cx, |s, _| {
        assert!(!s.busy);
        assert!(!s.dirty());
        assert_eq!(s.path.as_ref(), Some(&file));
    });
    assert!(file.is_file());
    let snapshot = studio.read_with(cx, |s, _| s.project.clone());
    ui(handle, cx, |window, cx| {
        window.press("ctrl-n", cx);
        window.press("ctrl-o", cx);
        window.click("confirm-discard", cx);
    });
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(|_| Some(vec![file.clone()]));
    cx.run_until_parked();
    studio.read_with(cx, |s, _| {
        assert_eq!(s.project, snapshot);
        assert!(!s.dirty());
        assert!(!s.busy);
    });
}

#[test]
#[ignore = "Requires a GPU renderer and OVERTONE_PREVIEW_DIR"]
fn render_profiles_for_visual_review() {
    use std::{path::PathBuf, sync::Arc};
    let directory =
        PathBuf::from(std::env::var_os("OVERTONE_PREVIEW_DIR").expect("set OVERTONE_PREVIEW_DIR"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);
    let (handle, studio) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(),
                    size: size(px(1480.), px(1040.)),
                })),
                show: false,
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Studio::new(None, window, cx)),
        )
        .unwrap()
    });
    cx.update_window(handle, |_, window, cx| {
        studio.update(cx, |s, cx| {
            s.dispatch(Action::SaveSound, window, cx);
            s.dispatch(Action::AddDraft, window, cx);
            s.dispatch(Action::Note(60), window, cx);
            s.dispatch(Action::Note(64), window, cx);
            s.dispatch(Action::Note(67), window, cx);
        });
    })
    .unwrap();
    for id in [3, 4, 5] {
        cx.update_window(handle, |_, window, cx| {
            studio.update(cx, |s, cx| s.dispatch(Action::Profile(id), window, cx));
            window.render_frame(cx);
        })
        .unwrap();
        cx.capture_screenshot(handle)
            .unwrap()
            .save(directory.join(format!("profile-{id}.png")))
            .unwrap();
    }
    cx.update_window(handle, |_, window, cx| {
        studio.update(cx, |s, cx| s.dispatch(Action::Profile(3), window, cx));
        window.render_frame(cx);
        let p = window.find("resize-left-dock").bounds().center();
        window.drag(p, point(p.x + px(40.), p.y), cx);
        let p = window
            .find("resize-pair-Harmonics-Controls")
            .bounds()
            .center();
        window.drag(p, point(p.x + px(50.), p.y), cx);
        let p = window.find("resize-panel-Source").bounds().center();
        window.drag(p, point(p.x, p.y + px(80.)), cx);
        studio.update(cx, |s, cx| {
            s.dispatch(Action::Spacing(0, 6.), window, cx);
        });
        window.render_frame(cx);
    })
    .unwrap();
    cx.capture_screenshot(handle)
        .unwrap()
        .save(directory.join("resized-studio.png"))
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        let from = window.find("handle-Library").bounds().center();
        let to = window.find("module-Source").bounds().center();
        move_drag(window, from, to, cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.capture_screenshot(handle)
        .unwrap()
        .save(directory.join("drag-preview.png"))
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        release_drag(window, point(px(4.), px(4.)), cx);
        studio.update(cx, |s, cx| {
            s.dispatch(Action::Zoom(None, 0.3), window, cx);
            s.dispatch(Action::Zoom(Some(Module::Controls), 0.2), window, cx);
        });
        window.render_frame(cx);
    })
    .unwrap();
    cx.capture_screenshot(handle)
        .unwrap()
        .save(directory.join("zoomed-studio.png"))
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        studio.update(cx, |s, cx| {
            s.dispatch(Action::Zoom(None, 0.), window, cx);
            s.dispatch(Action::Zoom(Some(Module::Controls), 0.), window, cx);
        });
        window.render_frame(cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        studio.update(cx, |s, cx| {
            crate::persistence::save(&s.project, &directory.join("example.overtone")).unwrap();
            crate::persistence::save_settings(
                &crate::storage::WorkspaceSettings::from_project(&s.project),
                &directory.join("workspace.settings.json"),
            )
            .unwrap();
            s.open_settings(window, cx);
        });
        window.render_frame(cx);
    })
    .unwrap();
    let settings_handle = studio.read_with(&cx, |s, _| s.settings_window.unwrap());
    cx.update_window(settings_handle, |_, window, cx| {
        window.render_frame(cx);
    })
    .unwrap();
    cx.capture_screenshot(settings_handle)
        .unwrap()
        .save(directory.join("workspace-settings.png"))
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        studio.update(cx, |s, cx| {
            s.dispatch(Action::Profile(5), window, cx);
            s.dispatch(Action::ToggleTheme, window, cx);
        });
        window.resize(size(px(1000.), px(680.)));
        window.bounds_changed(cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.capture_screenshot(handle)
        .unwrap()
        .save(directory.join("compact-light.png"))
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        studio.update(cx, |s, cx| {
            s.request_replace(crate::studio::Pending::Close, window, cx)
        });
        window.render_frame(cx);
    })
    .unwrap();
    cx.capture_screenshot(handle)
        .unwrap()
        .save(directory.join("exit-save-popup.png"))
        .unwrap();
}

#[gpui_kit::test]
fn menus_settings_shortcuts_and_undo_redo_share_the_project(cx: &mut TestAppContext) {
    let (handle, studio) = fixture(cx);
    ui(handle, cx, |window, cx| {
        assert!(window.try_find("hide-Library").is_none());
        assert!(window.try_find("configure-workspace").is_none());
        assert!(window.try_find("save-project").is_none());
        window.click("add-draft", cx);
        window.click("key-0", cx);
        window.click("edit-menu", cx);
        window.within("popup-menu").click(0usize, cx);
    });
    studio.read_with(cx, |s, _| {
        assert_eq!(s.project.clips.len(), 1);
        assert!(s.project.clips[0].notes.is_empty());
    });
    ui(handle, cx, |window, cx| {
        window.press("ctrl-z", cx);
        window.press("ctrl-shift-z", cx);
    });
    studio.read_with(cx, |s, _| assert_eq!(s.project.clips.len(), 1));
    ui(handle, cx, |window, cx| {
        window.click("view-menu", cx);
        window.within("popup-menu").click(0usize, cx);
    });
    let settings = studio.read_with(cx, |s, _| {
        s.settings_window.expect("View menu opens settings")
    });
    ui(settings, cx, |window, cx| {
        window.resize(size(px(1480.), px(1100.)));
        window.bounds_changed(cx);
        window.render_frame(cx);
        window.click("show-Library", cx);
    });
    studio.read_with(cx, |s, _| {
        assert!(
            !s.project
                .profile()
                .panels
                .iter()
                .find(|p| p.module == Module::Library)
                .unwrap()
                .visible
        )
    });
    ui(handle, cx, |window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("module-Library").is_none());
        window.press("ctrl-z", cx);
    });
    studio.read_with(cx, |s, _| {
        assert!(
            s.project
                .profile()
                .panels
                .iter()
                .find(|p| p.module == Module::Library)
                .unwrap()
                .visible
        )
    });
    let file = std::env::temp_dir().join(format!(
        "overtone-menu-{}.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    ui(handle, cx, |window, cx| {
        window.click("file-menu", cx);
        window.within("popup-menu").click(3usize, cx);
    });
    assert!(cx.did_prompt_for_new_path());
    cx.simulate_new_path_selection(|_| Some(file.clone()));
    cx.run_until_parked();
    ui(handle, cx, |window, cx| {
        window.click("key-4", cx);
        window.press("cmd-s", cx);
    });
    cx.run_until_parked();
    studio.read_with(cx, |s, _| {
        assert!(!s.dirty());
        assert_eq!(crate::persistence::load(&file).unwrap().project, s.project);
    });
    ui(handle, cx, |window, cx| {
        window.click("key-7", cx);
        window.press("ctrl-s", cx);
    });
    cx.run_until_parked();
    studio.read_with(cx, |s, _| {
        assert!(!s.dirty());
        assert_eq!(crate::persistence::load(&file).unwrap().project, s.project);
    });
    std::fs::remove_file(file).unwrap();
}

#[gpui_kit::test]
fn workspace_and_panel_zoom_scale_controls_and_keep_scroll_access(cx: &mut TestAppContext) {
    use gpui_kit::ScrollDelta;
    let (handle, studio) = fixture(cx);
    let mut original = 0.;
    ui(handle, cx, |window, cx| {
        original = f32::from(window.find("knob-false-34").bounds().size.height);
        window.press("ctrl-=", cx);
    });
    ui(handle, cx, |window, cx| {
        assert!(
            (f32::from(window.find("knob-false-34").bounds().size.height) - original * 1.1).abs()
                < 1.,
            "original={original}, actual={:?}, zoom={}",
            window.find("knob-false-34").bounds().size.height,
            studio.read(cx).project.profile().zoom
        );
        window.click("zoom-Controls-plus", cx);
    });
    ui(handle, cx, |window, cx| {
        assert!(
            (f32::from(window.find("knob-false-34").bounds().size.height) - original * 1.21).abs()
                < 1.
        );
        window.resize(size(px(1000.), px(680.)));
        window.bounds_changed(cx);
        window.render_frame(cx);
        window.scroll(
            "center-scroll",
            ScrollDelta::Pixels(point(px(-40.), px(-80.))),
            cx,
        );
    });
    studio.read_with(cx, |s, _| {
        assert_eq!(s.project.profile().zoom, 1.1);
        assert_eq!(s.project.profiles[1].zoom, 1.);
        assert!(s.scroll("center-scroll").max_offset().x > px(0.));
        assert!(s.scroll("center-scroll").max_offset().y > px(0.));
        assert!(s.scroll("center-scroll").offset().y < px(0.));
        let scroll = s.scroll("center-scroll");
        let maximum = scroll.max_offset();
        assert_eq!(
            crate::primitives::scroll_edges(point(px(0.), px(0.)), maximum),
            [false, false, true, true]
        );
        assert_eq!(
            crate::primitives::scroll_edges(point(-maximum.x, -maximum.y), maximum),
            [true, true, false, false]
        );
        assert_eq!(
            crate::primitives::scroll_edges(point(-maximum.x / 2., -maximum.y / 2.), maximum),
            [true, true, true, true]
        );
        assert_eq!(
            crate::primitives::scroll_edges(point(px(0.), px(0.)), point(px(0.), px(0.))),
            [false; 4]
        );
        assert_eq!(
            crate::storage::decode(&crate::storage::encode(&s.project).unwrap()).unwrap(),
            s.project
        );
    });
    ui(handle, cx, |window, cx| {
        window.press("ctrl-0", cx);
        window.click("view-menu", cx);
        window.within("popup-menu").click(9usize, cx);
    });

    studio.read_with(cx, |s, _| assert_eq!(s.project.profile().zoom, 1.));
}

fn move_drag(
    window: &mut Window,
    from: gpui_kit::Point<gpui_kit::Pixels>,
    to: gpui_kit::Point<gpui_kit::Pixels>,
    cx: &mut App,
) {
    use gpui_kit::{InputEvent, MouseButton, MouseDownEvent, MouseMoveEvent};
    window.dispatch_event(
        MouseMoveEvent {
            position: from,
            pressed_button: None,
            modifiers: Default::default(),
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
    window.dispatch_event(
        MouseDownEvent {
            button: MouseButton::Left,
            position: from,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
    for fraction in [0.2, 0.5, 1.] {
        window.dispatch_event(
            MouseMoveEvent {
                position: from + (to - from) * fraction,
                pressed_button: Some(MouseButton::Left),
                modifiers: Default::default(),
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    }
}
fn release_drag(window: &mut Window, position: gpui_kit::Point<gpui_kit::Pixels>, cx: &mut App) {
    use gpui_kit::{InputEvent, MouseButton, MouseUpEvent};
    window.dispatch_event(
        MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Default::default(),
            click_count: 1,
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}
#[gpui_kit::test]
fn dragging_previews_the_future_layout_and_commits_only_on_drop(cx: &mut TestAppContext) {
    let (handle, studio) = fixture(cx);
    let before = studio.read_with(cx, |s, _| s.project.clone());
    let mut target = point(px(0.), px(0.));
    ui(handle, cx, |window, cx| {
        let from = window.find("handle-Library").bounds().center();
        target = window.find("handle-Source").bounds().center();
        move_drag(window, from, target, cx);
    });
    studio.read_with(cx, |s, _| {
        assert_eq!(s.project, before);
        assert!(!s.dirty());
        let preview = s.drop_preview.expect("live drop preview");
        assert_eq!(preview.module, Module::Library);
        assert_eq!(preview.dock, Dock::Right);
        assert_eq!(
            s.displayed_profile()
                .panels
                .iter()
                .find(|p| p.module == Module::Library)
                .unwrap()
                .dock,
            Dock::Right
        );
    });
    ui(handle, cx, |window, cx| release_drag(window, target, cx));
    studio.read_with(cx, |s, _| {
        assert!(s.drop_preview.is_none());
        assert_eq!(
            s.project
                .profile()
                .panels
                .iter()
                .find(|p| p.module == Module::Library)
                .unwrap()
                .dock,
            Dock::Right
        );
        assert!(s.dirty());
    });
    ui(handle, cx, |window, cx| window.press("ctrl-z", cx));
    studio.read_with(cx, |s, _| assert_eq!(s.project, before));
    ui(handle, cx, |window, cx| {
        let from = window.find("handle-Library").bounds().center();
        let to = window.find("handle-Source").bounds().center();
        move_drag(window, from, to, cx);
        release_drag(window, point(px(4.), px(4.)), cx);
    });
    studio.read_with(cx, |s, _| {
        assert_eq!(s.project, before);
        assert!(s.drop_preview.is_none());
    });
}

#[gpui_kit::test]
fn menu_rename_and_exit_popup_save_before_closing(cx: &mut TestAppContext) {
    let (handle, studio) = fixture(cx);
    ui(handle, cx, |window, cx| {
        assert!(window.try_find("project-title").is_none());
        assert!(window.try_find("profile-3").is_none());
        window.click("file-menu", cx);
        window.within("popup-menu").click(7usize, cx);
    });
    ui(handle, cx, |window, cx| {
        window.click("project-title", cx);
        window.press("ctrl-a", cx);
        window.input("Menu session", cx);
        window.click("rename-done", cx);
    });
    studio.read_with(cx, |s, _| assert_eq!(s.project.title, "Menu session"));
    ui(handle, cx, |window, cx| {
        window.click("file-menu", cx);
        window.within("popup-menu").click(6usize, cx);
    });
    ui(handle, cx, |window, cx| {
        assert!(window.try_find("project-dialog").is_some());
        window.click("confirm-cancel", cx);
    });
    studio.read_with(cx, |s, _| assert!(s.pending.is_none()));
    ui(handle, cx, |window, cx| {
        window.click("file-menu", cx);
        window.within("popup-menu").click(6usize, cx);
    });
    ui(handle, cx, |window, cx| {
        window.click("confirm-save", cx);
    });
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    studio.read_with(cx, |s, _| {
        assert!(s.pending == Some(crate::studio::Pending::Close));
        assert!(s.dirty());
    });
    let file = std::env::temp_dir().join(format!(
        "overtone-exit-{}.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    ui(handle, cx, |window, cx| window.click("confirm-save", cx));
    cx.simulate_new_path_selection(|_| Some(file.clone()));
    cx.run_until_parked();
    assert_eq!(
        crate::persistence::load(&file).unwrap().project.title,
        "Menu session"
    );
    assert!(cx.update_window(handle, |_, _, _| ()).is_err());
    std::fs::remove_file(file).unwrap();
}
