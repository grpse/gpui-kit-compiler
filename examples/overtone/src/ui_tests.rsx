use crate::{
    model::*,
    studio::{Action, Studio},
};
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, Entity, HeadlessAppContext, TestAppContext, Window,
    WindowBounds, WindowOptions, point, px, size, test::TestWindowExt,
};

fn recording_fixture(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Studio>) {
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
// Keep exercising saved, customizable synthesis layouts alongside the new default desk.
fn fixture(cx:&mut TestAppContext)->(AnyWindowHandle,Entity<Studio>) {
    let (handle,studio)=recording_fixture(cx);
    cx.update(|cx|studio.update(cx,|s,cx|{
        let base=s.project.profiles[0].clone();
        s.project.profiles=(0..3).map(|index|{
            let mut profile=base.clone();profile.id=3+index as u64;
            profile.name=["Studio Desk","Voice Lab","Clip Composer"][index].into();
            profile.left_width=258.;profile.right_width=288.;
            for panel in &mut profile.panels {
                let (dock,visible)=match (index,panel.module) {
                    (0,Module::Library)=>(Dock::Left,true), (0,Module::Source)=>(Dock::Right,true),
                    (0,Module::Timeline|Module::Keyboard)=>(Dock::Lower,true), (0,Module::Inspector)=>(Dock::Right,false),
                    (1,Module::Source)=>(Dock::Left,true), (1,Module::Library)=>(Dock::Right,true),
                    (1,Module::Timeline|Module::Keyboard)=>(Dock::Lower,true), (1,Module::Inspector)=>(Dock::Right,false),
                    (2,Module::Library)=>(Dock::Left,true), (2,Module::Inspector)=>(Dock::Right,true),
                    (2,Module::Timeline|Module::Keyboard)=>(Dock::Main,true), (2,_)=>(Dock::Main,false),
                    _=>(Dock::Main,true),
                };
                panel.dock=dock;panel.visible=visible;
                panel.half=dock==Dock::Lower || index<2 && matches!(panel.module,Module::Harmonics|Module::Controls);
                if panel.module==Module::Reconstruction{panel.height=Some(560.);}
            }
            if index<2{profile.panels.sort_by_key(|p|if p.module==Module::Reconstruction{0}else{1});}
            else{profile.panels.sort_by_key(|p|match p.module{Module::Timeline=>0,Module::Keyboard=>1,_=>2});}
            profile
        }).collect();
        s.checkpoint=s.project.clone();cx.notify();
    }));
    (handle,studio)
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
        assert!(
            p.panels
                .iter()
                .find(|p| p.module == Module::Source)
                .unwrap()
                .height
                .is_some()
        );
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
        assert!(
            p.panels
                .iter()
                .find(|p| p.module == Module::Source)
                .unwrap()
                .height
                .is_none()
        );
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
        assert!(
            s.project.profiles[2]
                .panels
                .iter()
                .find(|p| p.module == Module::Timeline)
                .unwrap()
                .gap_after
                .is_none()
        );
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
        window.click("audio-menu", cx);
        window.within("popup-menu").click(2usize, cx); // Analyze is disabled without a source.
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
        assert!(window.try_find("sound-menu-11").is_none())
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
    use std::{path::PathBuf,sync::Arc};
    let directory=PathBuf::from(std::env::var_os("OVERTONE_PREVIEW_DIR").expect("set OVERTONE_PREVIEW_DIR"));
    std::fs::create_dir_all(&directory).unwrap();let path=directory.join("percussion.wav");
    let mut writer=hound::WavWriter::create(&path,hound::WavSpec{channels:1,sample_rate:8000,bits_per_sample:16,sample_format:hound::SampleFormat::Int}).unwrap();
    for i in 0..13200 {
        let t=i as f64/8000.;let local=t%0.55;
        let wave=if local<0.35{(std::f64::consts::TAU*(150.+(t/0.55).floor()*120.)*local).sin()*0.7*(1.-local/0.35).sqrt()}else{0.};
        writer.write_sample((wave*32767.) as i16).unwrap();
    }
    writer.finalize().unwrap();
    let mut cx=HeadlessAppContext::with_platform(gpui_kit::platform::current_platform(true).text_system(),Arc::new(gpui_kit::assets::Assets),gpui_kit::platform::current_headless_renderer);
    cx.update(gpui_kit::init);
    let (handle,studio)=cx.update(|cx|gpui_kit::open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(Bounds{origin:Default::default(),size:size(px(1480.),px(960.))})),show:false,..Default::default()},cx,|window,cx|cx.new(|cx|Studio::new(None,window,cx))).unwrap());
    cx.update_window(handle,|_,window,cx|studio.update(cx,|s,cx|{
        let mut source=crate::persistence::import_source(&path).unwrap();source.id=s.project.allocate();let id=source.id;
        let recording=rsx_overtone::analysis::decode(&path).unwrap();
        s.sound_waveforms.insert(source.sha256.clone(),Some(Arc::new(rsx_overtone::waveform::SourcePreview::new(rsx_overtone::analysis::Recording{samples:recording.samples.clone(),sample_rate:recording.sample_rate}).unwrap())));
        s.project.sources.push(source);
        let result=rsx_overtone::analysis::split_recording(&recording,&s.project.recording).unwrap();
        s.project.add_recorded_sounds(id,"Percussion",result.sounds);
        let sounds=s.project.library.iter().map(|s|s.id).collect::<Vec<_>>();
        for i in 0..6{s.project.place_library_sound(sounds[i%3],if i%2==0{1}else{2},2.+i as f64*0.5);}
        s.project.title="Microphone composition".into();s.project.timeline.zoom=2.;s.timeline_span=6.;
        s.project.insert_seconds=s.project.selected().unwrap().start_seconds+0.15;
        s.sync_sliders(true,window,cx);s.changed(cx);
    })).unwrap();
    for (width,height,name) in [(1480.,960.,"recording-desk"),(1000.,680.,"recording-desk-compact")] {
        cx.update_window(handle,|_,window,cx|{window.resize(size(px(width),px(height)));window.bounds_changed(cx);window.render_frame(cx);}).unwrap();
        cx.capture_screenshot(handle).unwrap().save(directory.join(format!("{name}.png"))).unwrap();
    }
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

#[gpui_kit::test]
fn audio_export_menu_renders_connected_notes_without_changing_project(cx: &mut TestAppContext) {
    let (handle, studio) = fixture(cx);
    ui(handle, cx, |window, cx| {
        window.click("key-0", cx);
        window.click("key-4", cx);
    });
    let before = studio.read_with(cx, |s, _| s.project.clone());
    let path = std::env::temp_dir().join(format!(
        "overtone-menu-export-{}.wav",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    ui(handle, cx, |window, cx| {
        window.click("audio-menu", cx);
        window.within("popup-menu").click(8usize, cx);
    });
    assert!(cx.did_prompt_for_new_path());
    cx.simulate_new_path_selection(|_| Some(path.clone()));
    cx.run_until_parked();
    studio.read_with(cx, |s, _| {
        assert_eq!(s.project, before);
        assert!(!s.busy);
        assert_eq!(s.notice, "Timeline exported as 48 kHz stereo WAV");
    });
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert!(bytes[44..].iter().any(|b| *b != 0));
    let api = rsx_overtone::engine::SynthApi::new(
        rsx_overtone::engine::EngineConfig {
            voices: 64,
            ..Default::default()
        },
        before.tuning.clone(),
    )
    .unwrap();
    assert_eq!(
        bytes.len() as u64,
        44 + api.timeline(&before).unwrap().total_frames * 4
    );
    std::fs::remove_file(path).unwrap();
}

#[gpui_kit::test]
fn recording_split_buttons_create_individual_clips_and_undo(cx:&mut TestAppContext) {
    let (handle,studio)=recording_fixture(cx);
    let dir=std::env::temp_dir().join(format!("overtone-ui-split-{}",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&dir).unwrap();let path=dir.join("voice.wav");
    let mut writer=hound::WavWriter::create(&path,hound::WavSpec{channels:1,sample_rate:48000,bits_per_sample:16,sample_format:hound::SampleFormat::Int}).unwrap();
    for i in 0..62400 {let time=i as f64/48000.;let hz=if time<0.5{220.}else{330.};let x=if time>=0.5 && time<0.8{0.}else{(std::f64::consts::TAU*hz*time).sin()*0.5};writer.write_sample((x*32767.) as i16).unwrap();}writer.finalize().unwrap();
    let id=cx.update_window(handle,|_,_,cx|studio.update(cx,|s,cx|{
        let mut source=crate::persistence::import_source(&path).unwrap();source.id=s.project.allocate();let id=source.id;s.project.sources.push(source);s.project.draft.source=Some(id);s.changed(cx);id
    })).unwrap();
    let before=studio.read_with(cx,|s,_|s.project.clone());
    ui(handle,cx,|window,cx|window.click(format!("analyze-{id}"),cx));cx.run_until_parked();
    studio.read_with(cx,|s,_|{
        assert!(!s.busy);assert_eq!(s.project.library.len(),2);assert_eq!(s.project.clips.len(),2);
        assert!(s.project.library.iter().all(|v|v.recorded_sample && v.source==Some(id)));
        assert_eq!(s.project.clips[1].start_seconds,0.8);assert_eq!(s.project.clip_seconds(&s.project.clips[0]),0.5);
        s.project.validate().unwrap();
    });
    let clip=studio.read_with(cx,|s,_|s.project.clips[1].id);
    ui(handle,cx,|window,cx|{
        let bounds=window.find(format!("clip-{clip}")).bounds();
        window.click_at(format!("clip-{clip}"),point(bounds.size.width*0.4,bounds.size.height/2.),cx);
    });
    studio.read_with(cx,|s,_|assert_eq!(s.project.insert_seconds,1.));
    ui(handle,cx,|window,cx|window.click("split-clip",cx));
    studio.read_with(cx,|s,_|{assert_eq!(s.project.clips.len(),3);assert!((s.project.clip_seconds(&s.project.clips[2])-0.3).abs()<1e-10);assert_eq!(s.project.library.len(),2);s.project.validate().unwrap();});
    ui(handle,cx,|window,cx|window.press("ctrl-z",cx));
    studio.read_with(cx,|s,_|assert_eq!(s.project.clips.len(),2));
    ui(handle,cx,|window,cx|window.press("ctrl-z",cx)); // Insert edit.
    ui(handle,cx,|window,cx|window.press("ctrl-z",cx)); // Split take.
    studio.read_with(cx,|s,_|assert_eq!(s.project,before));
    std::fs::remove_dir_all(dir).unwrap();
}
#[gpui_kit::test]
fn recording_preferences_are_persistent_and_library_stop_uses_the_real_callback(cx:&mut TestAppContext) {
    let (handle,studio)=fixture(cx);
    ui(handle,cx,|window,cx|window.click("analysis-gap-minus",cx));
    studio.read_with(cx,|s,_|{assert_eq!(s.project.recording.split_gap_ms,100);assert!(s.dirty());let restored=crate::storage::decode(&crate::storage::encode(&s.project).unwrap()).unwrap();assert_eq!(restored.recording,s.project.recording);});
    ui(handle,cx,|window,cx|window.click("save-sound",cx));
    let id=studio.read_with(cx,|s,_|s.project.library[0].id);
    cx.update(|cx|studio.update(cx,|s,cx|{s.auditioning=Some(id);cx.notify();}));
    #[cfg(feature="audio-output")] {
        ui(handle,cx,|window,cx|window.click(format!("play-sound-{id}"),cx));
        studio.read_with(cx,|s,_|{assert_eq!(s.auditioning,None);assert!(!s.audio_starting);assert!(s.audio_generation>0);});
    }
}

#[gpui_kit::test]
fn library_menu_edits_same_record_and_removes_without_changing_instances(cx:&mut TestAppContext) {
    let (handle,studio)=fixture(cx);
    ui(handle,cx,|window,cx|window.click("save-sound",cx));
    let id=studio.read_with(cx,|s,_|s.project.library[0].id);
    ui(handle,cx,|window,cx|window.click(format!("place-{id}"),cx));
    let clip=studio.read_with(cx,|s,_|s.project.clips[0].clone());
    ui(handle,cx,|window,cx|{window.click(format!("sound-menu-{id}"),cx);window.within("popup-menu").click(0usize,cx);});
    ui(handle,cx,|window,cx|window.click("harmonic-count-plus",cx));
    ui(handle,cx,|window,cx|{window.click(format!("sound-menu-{id}"),cx);window.within("popup-menu").click(1usize,cx);});
    studio.read_with(cx,|s,_|{assert_eq!(s.project.library.len(),1);assert_eq!(s.project.library[0].id,id);assert_eq!(s.project.library[0].harmonics.len(),9);assert_eq!(s.project.clips[0],clip);});
    ui(handle,cx,|window,cx|{window.click(format!("sound-menu-{id}"),cx);window.within("popup-menu").click(4usize,cx);});
    studio.read_with(cx,|s,_|{assert!(s.project.library.is_empty());assert_eq!(s.project.clips[0].sound,clip.sound);assert_eq!(s.project.clips[0].library_sound,None);s.project.validate().unwrap();});
    ui(handle,cx,|window,cx|window.press("ctrl-z",cx));
    studio.read_with(cx,|s,_|{assert_eq!(s.project.library.len(),1);assert_eq!(s.project.clips[0].library_sound,Some(id));});
}
#[gpui_kit::test]
fn timeline_time_zoom_scales_clip_width_and_grid_is_saved(cx:&mut TestAppContext) {
    let (handle,studio)=fixture(cx);
    ui(handle,cx,|window,cx|window.click("add-draft",cx));
    cx.update(|cx|studio.update(cx,|s,cx|{s.project.active_profile=5;s.changed(cx);}));
    let id=studio.read_with(cx,|s,_|s.project.clips[0].id);
    let mut before=px(0.);
    ui(handle,cx,|window,_|{before=window.find(format!("clip-{id}")).bounds().size.width;assert!(window.try_find("timeline-play").is_some());assert!(window.try_find("timeline-pause").is_some());});
    ui(handle,cx,|window,cx|window.click("timeline-zoom-plus",cx));
    ui(handle,cx,|window,_|{let after=window.find(format!("clip-{id}")).bounds().size.width;assert!((after-before*2.).abs()<px(1.));});
    ui(handle,cx,|window,cx|window.click("timeline-grid",cx));
    studio.read_with(cx,|s,_|{assert_eq!(s.project.timeline.zoom,2.);assert_eq!(s.project.timeline.snap_beats,0.5);assert_eq!(s.project.clips[0].duration_beats,4.);assert_eq!(crate::storage::decode(&crate::storage::encode(&s.project).unwrap()).unwrap(),s.project);});
}

#[gpui_kit::test]
fn library_cards_drag_into_tracks_and_snap_after_time_zoom(cx:&mut TestAppContext) {
    let (handle,studio)=fixture(cx);
    ui(handle,cx,|window,cx|window.click("save-sound",cx));
    let sound_id=studio.read_with(cx,|s,_|s.project.library[0].id);
    cx.update(|cx|studio.update(cx,|s,cx|{s.project.active_profile=5;s.project.timeline.zoom=2.;s.changed(cx);}));
    ui(handle,cx,|window,cx|{
        let origin=window.find(format!("library-sound-{sound_id}")).bounds().center();
        let lane=window.find("lane-2").bounds();
        window.drag(origin,point(lane.left()+px(83.),lane.center().y),cx);
    });
    let first=studio.read_with(cx,|s,_|{
        assert_eq!(s.project.library.len(),1);assert_eq!(s.project.clips.len(),1);
        let c=&s.project.clips[0];assert_eq!(c.track,2);assert_eq!(c.library_sound,Some(sound_id));assert!(!c.notes.is_empty());
        let beats=c.start_seconds*s.project.tempo(2)/60.;assert!((beats*4.-(beats*4.).round()).abs()<1e-8);assert!(c.start_seconds>0.);
        assert_eq!(c.sound,s.project.library[0]);s.project.validate().unwrap();c.clone()
    });
    ui(handle,cx,|window,cx|{
        let origin=window.find(format!("library-sound-{sound_id}")).bounds().center();let lane=window.find("lane-1").bounds();
        window.drag(origin,point(lane.left()+px(111.),lane.center().y),cx);
    });
    studio.read_with(cx,|s,_|{assert_eq!(s.project.clips.len(),2);assert_eq!(s.project.clips[0],first);assert_eq!(s.project.clips[1].track,1);assert_ne!(s.project.clips[1].id,first.id);assert_eq!(s.project.library.len(),1);});
    ui(handle,cx,|window,cx|window.press("ctrl-z",cx));
    studio.read_with(cx,|s,_|assert_eq!(s.project.clips,vec![first]));
}
#[gpui_kit::test]
fn harmonic_remove_buttons_preserve_numbers_save_record_and_isolate_clips(cx:&mut TestAppContext) {
    let (handle,studio)=fixture(cx);
    ui(handle,cx,|window,cx|window.click("save-sound",cx));let id=studio.read_with(cx,|s,_|s.project.library[0].id);
    ui(handle,cx,|window,cx|window.click(format!("place-{id}"),cx));
    let clip=studio.read_with(cx,|s,_|s.project.clips[0].clone());
    ui(handle,cx,|window,cx|{window.click(format!("sound-menu-{id}"),cx);window.within("popup-menu").click(0usize,cx);});
    ui(handle,cx,|window,cx|{window.click("partial-select-2",cx);window.click("remove-harmonic-1",cx);});
    ui(handle,cx,|window,cx|window.click("save-sound",cx));
    studio.read_with(cx,|s,_|{assert_eq!(s.project.library[0].harmonics.len(),7);assert_eq!(s.project.library[0].harmonics[1].multiple,3);assert_eq!(s.project.draft.harmonics[s.selected_harmonic].multiple,3);assert_eq!(s.project.clips[0],clip);});
    cx.update(|cx|studio.update(cx,|s,cx|{s.project.active_profile=5;s.changed(cx);}));
    ui(handle,cx,|window,cx|window.click("instance-remove-harmonic-1",cx));
    studio.read_with(cx,|s,_|{assert_eq!(s.project.clips[0].sound.harmonics[1].multiple,3);assert_eq!(s.project.clips[0].original_sound,clip.original_sound);assert_eq!(s.project.library[0].harmonics.len(),7);assert_eq!(crate::storage::decode(&crate::storage::encode(&s.project).unwrap()).unwrap(),s.project);});
    ui(handle,cx,|window,cx|window.click("reset-instance",cx));
    studio.read_with(cx,|s,_|assert_eq!(s.project.clips[0].sound,clip.sound));
}

#[gpui_kit::test]
fn reconstruction_cleanup_saves_the_record_and_undo_preserves_timeline_instances(cx:&mut TestAppContext) {
    let (handle,studio)=fixture(cx);
    cx.update_window(handle,|_,window,cx|studio.update(cx,|s,cx|{
        s.project.draft.harmonics[1].amplitude=0.001;s.dispatch(Action::SaveSound,window,cx);
        let id=s.project.library[0].id;s.dispatch(Action::AddLibrary(id),window,cx);s.dispatch(Action::LoadSound(id),window,cx);
    })).unwrap();
    let (original,clip)=studio.read_with(cx,|s,_|(s.project.draft.clone(),s.project.clips[0].clone()));
    ui(handle,cx,|window,_|{assert!(window.try_find("reconstruction-editor").is_some());assert!(window.try_find("recorded-placeholder").is_some());assert!(window.try_find("reconstructed-detail").is_some());});
    ui(handle,cx,|window,cx|window.click("cleanup-noise",cx));
    ui(handle,cx,|window,cx|window.click("clean-reconstruction",cx));
    studio.read_with(cx,|s,_|{assert!(!s.project.draft.harmonics.iter().any(|h|h.multiple==2));assert_eq!(s.project.draft.noise.level,0.);assert_eq!(s.project.library[0],original);assert_eq!(s.project.clips[0],clip);});
    ui(handle,cx,|window,cx|window.press("ctrl-z",cx));
    studio.read_with(cx,|s,_|assert_eq!(s.project.draft,original));
    ui(handle,cx,|window,cx|window.click("clean-reconstruction",cx));
    ui(handle,cx,|window,cx|window.click("reconstruction-save",cx));
    studio.read_with(cx,|s,_|{assert_eq!(s.project.library[0],s.project.draft);assert_eq!(s.project.library.len(),1);assert_eq!(s.project.clips[0],clip);});
    ui(handle,cx,|window,cx|window.click("reconstruction-instance",cx));
    ui(handle,cx,|window,cx|window.click("clean-reconstruction",cx));
    studio.read_with(cx,|s,_|{assert_eq!(s.project.clips[0].original_sound,clip.original_sound);assert!(!s.project.clips[0].sound.harmonics.iter().any(|h|h.multiple==2));assert_eq!(s.project.library[0],s.project.draft);});
    ui(handle,cx,|window,cx|{window.click("reconstruction-expand",cx);window.click("sine-select-1",cx);window.click("sine-remove-0",cx);});
    studio.read_with(cx,|s,_|{assert_eq!(s.project.clips[0].sound.harmonics[s.instance_harmonic].multiple,3);assert_eq!(s.project.clips[0].original_sound,clip.original_sound);assert_eq!(s.project.library[0],s.project.draft);});
}
#[gpui_kit::test]
fn reconstruction_loads_actual_waveform_and_seeks_without_modifying_source(cx:&mut TestAppContext) {
    let (handle,studio)=fixture(cx);
    let dir=std::env::temp_dir().join(format!("overtone-wave-ui-{}",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));std::fs::create_dir_all(&dir).unwrap();let path=dir.join("recording.wav");
    let mut writer=hound::WavWriter::create(&path,hound::WavSpec{channels:1,sample_rate:48000,bits_per_sample:16,sample_format:hound::SampleFormat::Int}).unwrap();
    for i in 0..48000 {let x=(std::f64::consts::TAU*220.*i as f64/48000.).sin()*0.5;writer.write_sample((x*32767.) as i16).unwrap();}writer.finalize().unwrap();let bytes=std::fs::read(&path).unwrap();
    cx.update_window(handle,|_,window,cx|studio.update(cx,|s,cx|{
        let mut source=crate::persistence::import_source(&path).unwrap();source.id=s.project.allocate();s.project.draft.source=Some(source.id);
        s.project.draft.capture=Some(SoundCapture{start_seconds:0.,duration_seconds:1.,fundamental_hz:Some(220.),confidence:0.99});s.project.sources.push(source);s.changed(cx);let _=window;
    })).unwrap();
    ui(handle,cx,|_,_|{});cx.run_until_parked();
    ui(handle,cx,|window,cx|{assert!(window.try_find("recorded-overview").is_some());assert!(window.try_find("recorded-detail").is_some());let bounds=window.find("recorded-overview").bounds();window.click_at("recorded-overview",point(bounds.size.width*0.6,bounds.size.height/2.),cx);});
    studio.read_with(cx,|s,_|{assert!(s.waveform_source.preview.is_some());assert!(!s.waveform_source.loading);assert!(s.waveform_position.unwrap()>0.5);assert!(s.reconstruction_plot.as_ref().unwrap().original.is_some());});
    ui(handle,cx,|window,cx|window.click("wave-cycles-plus",cx));
    studio.read_with(cx,|s,_|{assert_eq!(s.project.reconstruction.cycles,4.);let restored=crate::storage::decode(&crate::storage::encode(&s.project).unwrap()).unwrap();assert_eq!(restored.reconstruction,s.project.reconstruction);});
    assert_eq!(std::fs::read(&path).unwrap(),bytes);std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "Requires a GPU renderer and OVERTONE_PREVIEW_DIR"]
fn reconstruction_native_visual_review() {
    use std::{path::PathBuf,sync::Arc};
    let directory=PathBuf::from(std::env::var_os("OVERTONE_PREVIEW_DIR").expect("set OVERTONE_PREVIEW_DIR"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut cx=HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),Arc::new(gpui_kit::assets::Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);
    let (handle,studio)=cx.update(|cx|gpui_kit::open_window(WindowOptions {
        window_bounds:Some(WindowBounds::Windowed(Bounds {origin:Default::default(),size:size(px(1640.),px(1180.))})),show:false,..Default::default()
    },cx,|window,cx|cx.new(|cx|Studio::new(None,window,cx))).unwrap());
    cx.update_window(handle,|_,window,cx|studio.update(cx,|s,cx|{
        s.project.draft.name="Warm vocal · reconstruction".into();
        let amplitudes=[0.7,0.27,0.14,0.08,0.025,0.008,0.003,0.];
        for (h,amplitude) in s.project.draft.harmonics.iter_mut().zip(amplitudes){h.amplitude=amplitude;}
        s.project.draft.capture=Some(SoundCapture {start_seconds:0.,duration_seconds:1.4,fundamental_hz:Some(220.),confidence:0.98});
        s.dispatch(Action::SaveSound,window,cx);
        s.dispatch(Action::AddLibrary(s.project.library[0].id),window,cx);
        let samples=(0..67200).map(|i|{
            let t=i as f64/48000.;let envelope=(t*8.).min(1.)*((1.4-t)*6.).clamp(0.,1.);
            (envelope*(0.48*(std::f64::consts::TAU*220.*t).sin()+0.18*(std::f64::consts::TAU*440.*t).sin()+0.09*(std::f64::consts::TAU*660.*t).sin())) as f32
        }).collect();
        s.waveform_source.preview=Some(Arc::new(rsx_overtone::waveform::SourcePreview::new(rsx_overtone::analysis::Recording {samples,sample_rate:48000}).unwrap()));
        s.waveform_position=Some(0.4);
        for panel in &mut s.project.profile_mut().panels {
            panel.visible=matches!(panel.module,Module::Source|Module::Library|Module::Reconstruction|Module::Timeline);
            if panel.module==Module::Reconstruction{panel.height=None;}
        }
        s.sync_sliders(false,window,cx);s.changed(cx);
    })).unwrap();
    cx.update_window(handle,|_,window,cx|window.render_frame(cx)).unwrap();
    cx.capture_screenshot(handle).unwrap().save(directory.join("reconstruction.png")).unwrap();
}
