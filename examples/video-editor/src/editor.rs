use crate::state::{Action, ClipDragKind, ClipTiming, EditorState, Screen, TimelineSnapshot};
use gpui_kit::component::{
    input::{InputEvent, InputState},
    slider::{SliderEvent, SliderState},
};
use gpui_kit::{prelude::*, *};
use rsx_video_editor::{
    Workspace, catalog,
    media::{PreviewEvent, PreviewRequest, PreviewWorker},
    playback::{OutputEvent, OutputWorker, PlaybackPlan},
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InlineEdit {
    Track(usize),
    Clip(usize),
    Timing(usize),
}
pub struct ClipGesture {
    pub index: usize,
    kind: ClipDragKind,
    pointer_x: f32,
    scroll_x: f32,
    scale: f32,
    pub duration: f32,
    snapshot: TimelineSnapshot,
    changed: bool,
    timing: Option<ClipTiming>,
}

#[derive(Clone, Copy)]
pub struct NodeGesture {
    pub id: u64,
    pub pointer: [f32; 2],
    pub position: [f32; 2],
    pub scroll: [f32; 2],
}

fn timeline_wheel_movement(delta: ScrollDelta, shift: bool, line_height: Pixels) -> Point<Pixels> {
    let delta = delta.pixel_delta(line_height);
    if shift || delta.x.abs() > delta.y.abs() {
        // macOS can already remap Shift+wheel onto the X axis.
        point(
            if delta.x.abs() > delta.y.abs() {
                delta.x
            } else {
                delta.y
            },
            px(0.),
        )
    } else {
        point(px(0.), delta.y)
    }
}

pub struct Editor {
    pub composition: rsx_video_editor::composition::Composition,
    pub graph_inputs: Vec<Entity<InputState>>,
    pub graph_history: Vec<rsx_video_editor::composition::Composition>,
    pub graph_future: Vec<rsx_video_editor::composition::Composition>,
    pub graph_parts: Vec<Vec<Entity<InputState>>>,
    pub graph_search: Entity<InputState>,
    pub graph_category: Option<rsx_video_editor::composition::Category>,
    pub graph_zoom: f32,
    pub graph_bounds: [f32; 2],
    pub graph_time: Entity<InputState>,
    pub graph_duration: Entity<InputState>,
    pub graph_scroll: ScrollHandle,
    pub graph_focus: FocusHandle,
    pub graph_drag: Option<NodeGesture>,
    pub graph_drag_recorded: bool,
    pub graph_rendering: bool,
    pub graph_listening: bool,
    pub graph_result: Option<rsx_video_editor::compositing::RenderedComposition>,
    pub graph_frame: Option<Arc<RenderImage>>,
    pub graph_render_revision: Option<u64>,
    pub state: EditorState,
    pub search: Entity<InputState>,
    pub inline_edit: Option<InlineEdit>,
    pub clip_gesture: Option<ClipGesture>,
    pub suppress_clip_click: bool,
    pub name_input: Entity<InputState>,
    pub timing_inputs: Vec<Entity<InputState>>,
    pub header_search: Entity<InputState>,
    pub sliders: Vec<Entity<SliderState>>,
    pub seek: Entity<SliderState>,
    pub _subscriptions: Vec<Subscription>,
    pub edit_workspace: crate::workspace::Workspace,
    pub library_workspace: crate::workspace::Workspace,
    pub dragging_playhead: bool,
    pub timeline_origin: f32,
    pub track_controls_width: f32,
    pub dragging_track_divider: bool,
    pub timeline_scroll: ScrollHandle,
    pub timeline_vertical_scroll: ScrollHandle,
    pub timeline_zoom: Entity<SliderState>,
    pub timeline_scale: f32,
    pub timeline_focus: FocusHandle,
    pub media_scroll: ScrollHandle,
    pub preset_scroll: ScrollHandle,
    pub history: Vec<rsx_video_editor::project::Timeline>,
    pub future: Vec<rsx_video_editor::project::Timeline>,
    pub processing: Vec<rsx_video_editor::processing::Update>,
    pub exporting: bool,
    pub export_cancel: Arc<AtomicBool>,
    pub export_progress: Option<rsx_video_editor::export::Progress>,
    pub export_path: Option<PathBuf>,
    pub project_path: Option<PathBuf>,
    pub preview_still: Option<PathBuf>,
    pub importing: bool,
    pub import_cancel: Arc<AtomicBool>,
    pub cache: Option<Arc<Workspace>>,
    pub preview_frame: Option<Arc<RenderImage>>,
    pub preview_worker: PreviewWorker,
    pub preview_generation: u64,
    pub preview_offset: f32,
    pub preview_loop_start: f32,
    pub preview_end: f32,
    pub output_worker: OutputWorker,
    pub transport_generation: u64,
    pub playback_plan: Option<PlaybackPlan>,
    pub active_video: Option<usize>,
    pub playback_gap: bool,
}
impl Drop for Editor {
    fn drop(&mut self) {
        self.export_cancel.store(true, Ordering::Relaxed);
        self.import_cancel.store(true, Ordering::Relaxed);
        self.output_worker.pause();
    }
}
impl Editor {
    pub fn scroll_timeline(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let movement =
            timeline_wheel_movement(event.delta, event.modifiers.shift, window.line_height());
        let handle = if event.modifiers.shift || !movement.x.is_zero() {
            &self.timeline_scroll
        } else {
            &self.timeline_vertical_scroll
        };
        let offset = handle.offset();
        let max = handle.max_offset();
        handle.set_offset(point(
            (offset.x + movement.x).clamp(-max.x, px(0.)),
            (offset.y + movement.y).clamp(-max.y, px(0.)),
        ));
        // Consume even at an edge so another axis or panel cannot also scroll.
        cx.stop_propagation();
        if handle.offset() != offset {
            cx.notify();
        }
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut state = EditorState::default();
        if let Some(screen) = std::env::args().find_map(|arg| match arg.as_str() {
            "--library" => Some(Screen::Library),
            "--overview" => Some(Screen::Overview),

            "--compositing" => Some(Screen::Compositing),
            _ => None,
        }) {
            state.apply(Action::Screen(screen));
        }
        if std::env::args().any(|arg| arg == "--empty-timeline") {
            state.clips.clear();
            state.tracks.clear();
            state.position = 0.;
            state.project_min_duration = 1.;
        }
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search media…"));
        let header_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search media, folders, or tags…"));
        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("Name"));
        let graph_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search nodes…"));
        let graph_inputs = (0..2)
            .map(|_| cx.new(|cx| InputState::new(window, cx)))
            .collect();
        let graph_time = cx.new(|cx| {
            let mut input = InputState::new(window, cx);
            input.set_value("0", window, cx);
            input
        });
        let graph_duration = cx.new(|cx| {
            let mut input = InputState::new(window, cx);
            input.set_value("5", window, cx);
            input
        });
        let timing_inputs: Vec<_> = ["Start seconds", "Source-in seconds", "Duration seconds"]
            .into_iter()
            .map(|label| cx.new(|cx| InputState::new(window, cx).placeholder(label)))
            .collect();
        let mut subscriptions = vec![
            cx.subscribe_in(
                &search,
                window,
                |this, input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        let value = input.read(cx).value().to_string();
                        this.state.apply(Action::Search(value.clone()));
                        this.header_search
                            .update(cx, |input, cx| input.set_value(value, window, cx));
                        cx.notify();
                    }
                },
            ),
            cx.subscribe_in(
                &header_search,
                window,
                |this, input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        let value = input.read(cx).value().to_string();
                        this.state.apply(Action::Search(value.clone()));
                        this.search
                            .update(cx, |input, cx| input.set_value(value, window, cx));
                        cx.notify();
                    }
                },
            ),
        ];
        subscriptions.push(cx.subscribe_in(
            &graph_search,
            window,
            |_, _, _: &InputEvent, _, cx| cx.notify(),
        ));
        for input in [&graph_time, &graph_duration] {
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.composition.revision += 1;
                        cx.notify();
                    }
                },
            ));
        }
        subscriptions.push(cx.subscribe_in(
            &name_input,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur)
                    && matches!(
                        this.inline_edit,
                        Some(InlineEdit::Track(_) | InlineEdit::Clip(_))
                    )
                {
                    this.commit_inline(window, cx);
                }
            },
        ));
        for input in &timing_inputs {
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                |this, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.commit_inline(window, cx);
                    }
                },
            ));
        }
        let sliders = state
            .controls
            .iter()
            .enumerate()
            .map(|(i, &value)| {
                let (min, max) = match i {
                    0 => (10., 200.),
                    1 => (-180., 180.),
                    _ => (0., 100.),
                };
                let slider = cx.new(|_| {
                    SliderState::new()
                        .min(min)
                        .max(max)
                        .step(1.)
                        .default_value(value)
                });
                subscriptions.push(cx.subscribe_in(
                    &slider,
                    window,
                    move |this, _, event: &SliderEvent, window, cx| {
                        if let SliderEvent::Change(value) = event {
                            this.dispatch(Action::Control(i, value.end()), window, cx);
                        }
                    },
                ));
                slider
            })
            .collect();
        let seek = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(1.)
                .step(0.001)
                .default_value(state.position / state.seek_limit())
        });
        subscriptions.push(cx.subscribe_in(
            &seek,
            window,
            |this, _, event: &SliderEvent, window, cx| {
                if let SliderEvent::Change(value) = event {
                    this.dispatch(
                        Action::Seek(value.end() * this.state.seek_limit()),
                        window,
                        cx,
                    );
                }
            },
        ));
        let timeline_zoom = cx.new(|_| {
            SliderState::new()
                .min(0.25)
                .max(8.)
                .step(0.05)
                .default_value(state.zoom)
        });
        subscriptions.push(cx.subscribe_in(
            &timeline_zoom,
            window,
            |this, _, event: &SliderEvent, window, cx| {
                if let SliderEvent::Change(value) = event {
                    this.dispatch(Action::Zoom(value.end() - this.state.zoom), window, cx);
                }
            },
        ));
        let owner = cx.weak_entity();
        let edit_workspace = crate::workspace::workspace(owner.clone(), true, window, cx);
        let library_workspace = crate::workspace::workspace(owner, false, window, cx);
        let (preview_worker, preview_events) = PreviewWorker::start();
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(event) = preview_events.recv().await {
                if this
                    .update_in(cx, |this, window, cx| {
                        match event {
                            PreviewEvent::Frame { generation, frame }
                                if generation == this.preview_generation =>
                            {
                                if this.state.playing && !this.output_worker.clock.running() {
                                    this.state.position = (frame.timestamp as f32
                                        + this.preview_offset)
                                        .clamp(0., this.state.seek_limit());
                                    this.seek.update(cx, |slider, cx| {
                                        slider.set_value(
                                            this.state.position / this.state.seek_limit(),
                                            window,
                                            cx,
                                        )
                                    });
                                }
                                // Release the old GPU image rather than growing a frame texture cache.
                                if let Some(old) = this.preview_frame.take() {
                                    let _ = window.drop_image(old);
                                }
                                this.preview_frame = Some(frame.render_image());
                                cx.notify();
                            }
                            PreviewEvent::End(generation)
                                if generation == this.preview_generation =>
                            {
                                if this.output_worker.clock.running() {
                                    return;
                                }
                                if this.state.looping && this.state.playing {
                                    this.state.position = this.preview_loop_start;
                                    this.refresh_preview(window, cx);
                                } else {
                                    this.state.playing = false;
                                    this.state.position = this.preview_end;
                                    this.seek.update(cx, |slider, cx| {
                                        slider.set_value(
                                            this.state.position / this.state.seek_limit(),
                                            window,
                                            cx,
                                        )
                                    });
                                    cx.notify();
                                }
                            }
                            PreviewEvent::Error(generation, error)
                                if generation == this.preview_generation =>
                            {
                                this.state.playing = false;
                                this.output_worker.pause();
                                this.state.notice = Some(format!("Video preview: {error}"));
                                cx.notify();
                            }
                            _ => {}
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let (output_worker, output_events) = OutputWorker::start();
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(OutputEvent::Error(generation, error)) = output_events.recv().await {
                if this
                    .update(cx, |this, cx| {
                        if generation == this.transport_generation {
                            this.state.playing = false;
                            this.graph_listening = false;
                            this.preview_worker.pause();
                            this.state.notice = Some(format!("Audio output: {error}"));
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                if this
                    .update_in(cx, |this, window, cx| this.playback_tick(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let mut editor = Self {
            composition: Default::default(),
            graph_inputs,
            graph_history: vec![],
            graph_future: vec![],
            graph_parts: vec![],
            graph_search,
            graph_category: None,
            graph_zoom: 0.8,
            graph_bounds: [800., 500.],
            graph_time,
            graph_duration,
            graph_scroll: ScrollHandle::new(),
            graph_focus: cx.focus_handle(),
            graph_drag: None,
            graph_drag_recorded: false,
            graph_rendering: false,
            graph_listening: false,
            graph_result: None,
            graph_frame: None,
            graph_render_revision: None,
            state,
            search,
            inline_edit: None,
            clip_gesture: None,
            suppress_clip_click: false,
            name_input,
            timing_inputs,
            header_search,
            sliders,
            seek,
            _subscriptions: subscriptions,
            edit_workspace,
            library_workspace,
            dragging_playhead: false,
            timeline_origin: 0.,
            track_controls_width: 176.,
            dragging_track_divider: false,
            timeline_scroll: ScrollHandle::new(),
            timeline_vertical_scroll: ScrollHandle::new(),
            timeline_zoom,
            timeline_scale: 1.,
            timeline_focus: cx.focus_handle(),
            media_scroll: ScrollHandle::new(),
            preset_scroll: ScrollHandle::new(),
            history: vec![],
            future: vec![],
            processing: vec![],
            exporting: false,
            export_cancel: Arc::new(AtomicBool::new(false)),
            export_progress: None,
            export_path: None,
            project_path: None,
            preview_still: None,
            importing: false,
            import_cancel: Arc::new(AtomicBool::new(false)),
            cache: Workspace::new().ok().map(Arc::new),
            preview_frame: None,
            preview_worker,
            preview_generation: 0,
            preview_offset: 0.,
            preview_loop_start: 0.,
            preview_end: 0.,
            output_worker,
            transport_generation: 0,
            playback_plan: None,
            active_video: None,
            playback_gap: false,
        };
        let args: Vec<_> = std::env::args_os().collect();
        if let Some(pair) = args.windows(2).find(|pair| pair[0] == "--import") {
            editor.import_paths(vec![PathBuf::from(&pair[1])], window, cx);
        } else if args.iter().any(|arg| arg == "--demo") {
            editor.import_paths(
                vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/media")],
                window,
                cx,
            );
        }
        editor
    }
    pub fn begin_clip_gesture(
        &mut self,
        index: usize,
        kind: ClipDragKind,
        pointer_x: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.inline_edit.is_some() || self.state.clips.get(index).is_none() {
            return;
        }
        self.suppress_clip_click = false;
        self.dispatch(Action::TargetClip(index), window, cx);
        self.timeline_focus.focus(window, cx);
        if self.state.linked_locked(index) {
            self.state.notice = Some("Unlock the linked tracks before dragging.".into());
            cx.notify();
            return;
        }
        if self.state.playing {
            self.dispatch(Action::Play, window, cx);
        }
        self.timeline_focus.focus(window, cx);
        self.clip_gesture = Some(ClipGesture {
            index,
            kind,
            pointer_x,
            scroll_x: self.timeline_scroll.offset().x.into(),
            scale: self.timeline_scale.max(0.001),
            duration: self.state.project_duration(),
            snapshot: self.state.timeline_snapshot(),
            changed: false,
            timing: None,
        });
        self.state.notice = None;
        cx.notify();
    }
    pub fn update_clip_gesture(&mut self, pointer_x: f32, cx: &mut Context<Self>) {
        let Some(gesture) = self.clip_gesture.as_mut() else {
            return;
        };
        let pixels = pointer_x
            - gesture.pointer_x
            - (f32::from(self.timeline_scroll.offset().x) - gesture.scroll_x);
        if !gesture.changed && pixels.abs() < 3. {
            return;
        }
        gesture.changed = true;
        self.state.restore_timeline(&gesture.snapshot);
        match self
            .state
            .drag_clip_timing(gesture.index, gesture.kind, pixels / gesture.scale)
        {
            Ok(timing) => {
                if let Err(error) = self.state.edit_clip_timing(
                    gesture.index,
                    timing.start,
                    timing.source_start,
                    timing.length,
                ) {
                    self.state.notice = Some(error);
                } else {
                    gesture.timing = Some(timing);
                }
            }
            Err(error) => self.state.notice = Some(error),
        }
        cx.notify();
    }
    pub fn finish_clip_gesture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(gesture) = self.clip_gesture.take() else {
            return;
        };
        self.suppress_clip_click = gesture.changed || gesture.kind != ClipDragKind::Move;
        if gesture.changed
            && let Some(timing) = gesture.timing
        {
            self.state.restore_timeline(&gesture.snapshot);
            self.dispatch(
                Action::EditClipTiming {
                    index: self.state.selected_clip,
                    start: timing.start,
                    source_start: timing.source_start,
                    length: timing.length,
                },
                window,
                cx,
            );
        }
        cx.notify();
    }
    pub fn cancel_clip_gesture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(gesture) = self.clip_gesture.take() {
            self.state.restore_timeline(&gesture.snapshot);
            self.suppress_clip_click = true;
            self.refresh_preview(window, cx);
            cx.notify();
        }
    }
    pub fn begin_inline(
        &mut self,
        target: InlineEdit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_inline(cx);
        match target {
            InlineEdit::Track(index) => {
                let Some(track) = self.state.tracks.get(index) else {
                    return;
                };
                let value = track.name.clone();
                self.name_input
                    .update(cx, |input, cx| input.set_value(value, window, cx));
            }
            InlineEdit::Clip(index) | InlineEdit::Timing(index) => {
                if self.state.clips.get(index).is_none() {
                    return;
                }
                if self.state.linked_locked(index) {
                    self.state.notice = Some("Unlock the linked tracks before editing.".into());
                    cx.notify();
                    return;
                }
                self.dispatch(Action::TargetClip(index), window, cx);
                if matches!(target, InlineEdit::Clip(_)) {
                    let value = self.state.clip_label(index);
                    self.name_input
                        .update(cx, |input, cx| input.set_value(value, window, cx));
                } else {
                    let clip = &self.state.clips[index];
                    for (input, value) in
                        self.timing_inputs
                            .iter()
                            .zip([clip.start, clip.source_start, clip.length])
                    {
                        input.update(cx, |input, cx| {
                            input.set_value(format!("{value:.6}"), window, cx)
                        });
                    }
                }
            }
        }
        self.inline_edit = Some(target);
        cx.defer_in(window, move |this, window, cx| {
            if this.inline_edit != Some(target) {
                return;
            }
            let input = if matches!(target, InlineEdit::Timing(_)) {
                this.timing_inputs[0].clone()
            } else {
                this.name_input.clone()
            };
            input.update(cx, |input, cx| {
                input.focus_handle(cx).focus(window, cx);
                input.select_all(window, cx);
            });
            cx.notify();
        });
        cx.notify();
    }
    pub fn cancel_inline(&mut self, cx: &mut Context<Self>) {
        self.inline_edit = None;
        cx.notify();
    }
    pub fn commit_inline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.inline_edit else {
            return;
        };
        let action = match target {
            InlineEdit::Track(index) | InlineEdit::Clip(index) => {
                let name = self.name_input.read(cx).value().to_string();
                let name = match EditorState::valid_name(name) {
                    Ok(name) => name,
                    Err(error) => {
                        self.state.notice = Some(error);
                        cx.notify();
                        return;
                    }
                };
                if matches!(target, InlineEdit::Track(_)) {
                    Action::RenameTrack(index, name)
                } else {
                    Action::RenameClip(index, name)
                }
            }
            InlineEdit::Timing(index) => {
                let values: Result<Vec<f32>, _> = self
                    .timing_inputs
                    .iter()
                    .map(|input| input.read(cx).value().trim().parse())
                    .collect();
                let values = match values {
                    Ok(values) => values,
                    Err(_) => {
                        self.state.notice =
                            Some("Enter start, source-in and duration as decimal seconds.".into());
                        cx.notify();
                        return;
                    }
                };
                if let Err(error) = self
                    .state
                    .validate_clip_timing(index, values[0], values[1], values[2])
                {
                    self.state.notice = Some(error);
                    cx.notify();
                    return;
                }
                Action::EditClipTiming {
                    index,
                    start: values[0],
                    source_start: values[1],
                    length: values[2],
                }
            }
        };
        self.inline_edit = None;
        self.dispatch(action, window, cx);
    }
    fn choose_media(&mut self, folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.importing {
            self.state.notice =
                Some("An import is already running. Cancel it or wait for it to finish.".into());
            cx.notify();
            return;
        }
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: !folder,
            directories: folder,
            multiple: !folder,
            prompt: Some(
                if folder {
                    "Import folder"
                } else {
                    "Import media"
                }
                .into(),
            ),
        });
        cx.spawn_in(window, async move |this, cx| match choice.await {
            Ok(Ok(Some(paths))) if !paths.is_empty() => {
                let _ = this.update_in(cx, |this, window, cx| this.import_paths(paths, window, cx));
            }
            Ok(Err(error)) => {
                let _ = this.update(cx, |this, cx| {
                    this.state.notice = Some(format!("Could not open file picker: {error}"));
                    cx.notify();
                });
            }
            _ => {}
        })
        .detach();
    }
    pub fn import_paths(
        &mut self,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.import_paths_mode(paths, false, window, cx);
    }
    pub fn import_paths_mode(
        &mut self,
        paths: Vec<PathBuf>,
        composition_result: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.importing {
            return;
        }
        self.processing.clear();
        self.importing = true;
        self.import_cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.import_cancel.clone();
        let cache = self.cache.clone();
        let existing = self
            .state
            .assets
            .iter()
            .filter(|asset| {
                asset.metadata_error.is_none()
                    && asset.prepared.as_ref().is_some_and(|prepared| {
                        asset.path.as_ref().is_some_and(|path| {
                            rsx_video_editor::preprocess::SourceFingerprint::read(path)
                                .ok()
                                .as_ref()
                                == Some(&prepared.fingerprint)
                        })
                    })
            })
            .filter_map(|asset| asset.path.clone())
            .collect();
        self.state.notice = Some(
            "Preprocessing media, separating audio channels, and generating thumbnails…".into(),
        );
        cx.spawn_in(window, async move |this, cx| {
            let (sender, receiver) = async_channel::bounded(128);
            let work = cx.background_spawn(async move {
                catalog::import_with_progress(paths, existing, cache.as_ref().map(|cache| cache.path.as_path()), cancel,
                    &mut |update| { let _=sender.send_blocking(update); })
            });
            while let Ok(update)=receiver.recv().await {
                if this.update(cx,|this,cx|{this.update_processing(update);cx.notify();}).is_err(){return;}
            }
            let report = work.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.importing = false;
                this.finish_processing();
                {
                    this.cancel_clip_gesture(window, cx);
                    let before = rsx_video_editor::project::Timeline::capture(&this.state);
                    this.inline_edit=None;
                    let screen = this.state.screen;
                    let (added, duplicates, placed) = if composition_result {this.state.import_composition_result(report.assets)} else {this.state.import_into_project(report.assets)};
                    if screen == Screen::Compositing { this.seed_composition(window,cx); }
                    if !screen.has_timeline() {this.state.apply(Action::Screen(screen));}
                    this.search.update(cx, |input, cx| input.set_value(this.state.query.clone(), window, cx));
                    this.header_search.update(cx, |input, cx| input.set_value(this.state.query.clone(), window, cx));
                    let mut message = format!("Imported {added} file(s) · {placed} added to timeline · {duplicates} duplicate(s) skipped.");
                    if let Some(error) = report.errors.first() { message.push_str(&format!(" {} issue(s): {error}", report.errors.len())); }
                    if added == 0 && duplicates == 0 && report.errors.is_empty() { message = "No supported media files found in the selection.".into(); }
                    if report.cancelled {message.push_str(" Remaining import cancelled.");}
                    this.state.notice = Some(message);
                    this.record_edit(before);
                    this.media_scroll.set_offset(point(px(0.), px(0.)));
                    this.refresh_preview(window, cx);
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
    pub(crate) fn refresh_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.playing {
            self.restart_playback(window, cx);
            return;
        }
        self.output_worker.pause();
        self.preview_still = None;
        if self.state.screen.has_timeline() {
            self.playback_plan = Some(PlaybackPlan::from_state(&self.state));
            self.active_video = None;
            self.schedule_video(true, window, cx);
            self.seek.update(cx, |slider, cx| {
                slider.set_value(self.state.position / self.state.seek_limit(), window, cx)
            });
            return;
        }
        self.playback_plan = None;
        self.active_video = None;
        self.playback_gap = false;
        self.preview_generation = self.preview_generation.wrapping_add(1);
        if let Some(old) = self.preview_frame.take() {
            let _ = window.drop_image(old);
        }
        let Some(asset) = self.state.assets.get(self.state.selected) else {
            self.preview_worker.pause();
            return;
        };
        let component = self
            .state
            .screen
            .has_timeline()
            .then(|| {
                self.state
                    .clips
                    .get(self.state.selected_clip)
                    .filter(|clip| clip.asset == self.state.selected)
            })
            .flatten()
            .and_then(|clip| clip.component);
        if matches!(
            component,
            Some(crate::state::ClipComponent::AudioChannel { .. })
        ) {
            self.state.playing = false;
            self.preview_worker.pause();
            return;
        }
        let Some(path) = asset
            .path
            .clone()
            .filter(|_| asset.kind == crate::state::Kind::Video)
        else {
            self.preview_worker.pause();
            return;
        };
        let (position, end, offset, loop_start) = if self.state.screen.has_timeline() {
            let Some(clip) = self
                .state
                .clips
                .get(self.state.selected_clip)
                .filter(|clip| {
                    clip.asset == self.state.selected
                        && self.state.position >= clip.start
                        && self.state.position <= clip.start + clip.length
                })
            else {
                self.preview_worker.pause();
                if self.state.playing {
                    self.state.notice = Some("Select a timeline clip and place the playhead inside it, or open Media to preview the source video.".into());
                }
                self.state.playing = false;
                return;
            };
            (
                clip.source_start + self.state.position - clip.start,
                clip.source_start + clip.length,
                clip.start - clip.source_start,
                clip.start,
            )
        } else {
            (self.state.position, asset.duration, 0., 0.)
        };
        if end <= 0. {
            self.state.playing = false;
            self.preview_worker.pause();
            return;
        }
        let position = position.clamp(0., (end - 0.001).max(0.));
        self.preview_offset = offset;
        self.preview_loop_start = loop_start;
        self.preview_end = end + offset;
        self.preview_worker.request(PreviewRequest {
            path,
            position: position as f64,
            end: end as f64,
            playing: self.state.playing,
            generation: self.preview_generation,
            stream: match component {
                Some(crate::state::ClipComponent::Video(stream)) => Some(stream),
                _ => asset
                    .prepared
                    .as_ref()
                    .and_then(|prepared| prepared.primary_video),
            },
            clock: None,
            clock_offset: 0.,
            keyframes: asset
                .prepared
                .as_ref()
                .and_then(|prepared| {
                    prepared.videos.iter().find(|video| {
                        Some(video.index)
                            == match component {
                                Some(crate::state::ClipComponent::Video(stream)) => Some(stream),
                                _ => prepared.primary_video,
                            }
                    })
                })
                .map(|video| video.keyframes.clone()),
        });
        self.seek.update(cx, |slider, cx| {
            slider.set_value(self.state.position / self.state.seek_limit(), window, cx)
        });
    }
    fn restart_playback(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let plan = PlaybackPlan::from_state(&self.state);
        if plan.audio.is_empty() && plan.videos.is_empty() && plan.images.is_empty() {
            self.state.playing = false;
            self.state.notice = Some("Import source media to play the project.".into());
            self.output_worker.pause();
            return;
        }
        self.state.notice = None;
        self.transport_generation = self.transport_generation.wrapping_add(1);
        self.preview_end = plan.end as f32;
        self.preview_loop_start = 0.;
        self.output_worker.play(
            plan.clone(),
            self.state.position as f64,
            self.transport_generation,
        );
        self.playback_plan = Some(plan);
        self.active_video = None;
        self.schedule_video(true, window, cx);
    }
    fn schedule_video(&mut self, force: bool, window: &mut Window, _cx: &mut Context<Self>) {
        let mut video = self
            .playback_plan
            .as_ref()
            .and_then(|plan| plan.video_at(self.state.position as f64))
            .map(|(index, region)| (index, region.clone()));
        let image = self
            .playback_plan
            .as_ref()
            .and_then(|plan| plan.image_at(self.state.position as f64))
            .map(|(index, image)| (index, image.clone()));
        let image = image.filter(|(_, image)| {
            video
                .as_ref()
                .is_none_or(|(_, video)| (image.track, image.order) >= (video.track, video.order))
        });
        if image.is_some() {
            video = None;
        }
        let index = image
            .as_ref()
            .map(|(index, _)| {
                index
                    + self
                        .playback_plan
                        .as_ref()
                        .map_or(0, |plan| plan.videos.len())
            })
            .or_else(|| video.as_ref().map(|(index, _)| *index));
        if !force && index == self.active_video {
            return;
        }
        self.active_video = index;
        self.preview_generation = self.preview_generation.wrapping_add(1);
        if let Some(old) = self.preview_frame.take() {
            let _ = window.drop_image(old);
        }
        self.preview_still = image.map(|(_, image)| image.path);
        self.playback_gap = video.is_none() && self.preview_still.is_none();
        if let Some((_, region)) = video {
            let offset = region.start - region.source_start;
            self.preview_worker.request(PreviewRequest {
                path: region.path,
                position: (self.state.position as f64 - offset).max(region.source_start),
                end: region.source_start + region.duration,
                playing: self.state.playing,
                generation: self.preview_generation,
                stream: Some(region.stream),
                keyframes: Some(region.keyframes),
                clock: self.state.playing.then(|| self.output_worker.clock.clone()),
                clock_offset: offset,
            });
        } else {
            self.preview_worker.pause();
        }
    }
    fn playback_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.state.playing || !self.output_worker.clock.running() {
            return;
        }
        self.state.position =
            (self.output_worker.clock.position() as f32).clamp(0., self.preview_end);
        if self.output_worker.clock.ended() {
            if self.state.looping {
                self.state.position = 0.;
                self.restart_playback(window, cx);
            } else {
                self.state.playing = false;
                self.state.position = self.preview_end;
                self.output_worker.pause();
                self.preview_worker.pause();
            }
        } else {
            self.schedule_video(false, window, cx);
        }
        self.seek.update(cx, |slider, cx| {
            slider.set_value(self.state.position / self.state.seek_limit(), window, cx)
        });
        cx.notify();
    }
    pub fn timeline_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key == "escape" && self.clip_gesture.is_some() {
            self.cancel_clip_gesture(window, cx);
            cx.stop_propagation();
            return;
        }
        // Native input controls retain their own text editing shortcuts.
        if !self.timeline_focus.is_focused(window)
            || self.inline_edit.is_some()
            || !self.state.screen.has_timeline()
        {
            return;
        }
        if let Some(action) = crate::state::timeline_shortcut(
            &event.keystroke.key,
            event.keystroke.modifiers.platform || event.keystroke.modifiers.control,
            event.keystroke.modifiers.alt,
            event.keystroke.modifiers.shift,
        ) {
            self.dispatch(action, window, cx);
            cx.stop_propagation();
            window.prevent_default();
        }
    }
    pub fn dispatch(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_clip_gesture(window, cx);
        match action {
            Action::Export => {
                self.choose_export(window, cx);
                return;
            }
            Action::SaveProject => {
                self.choose_save(window, cx);
                return;
            }
            Action::OpenProject => {
                self.choose_project(window, cx);
                return;
            }
            Action::CancelExport => {
                self.export_cancel.store(true, Ordering::Relaxed);
                return;
            }
            Action::Undo | Action::Redo => {
                self.undo_timeline(matches!(action, Action::Redo), window, cx);
                return;
            }
            _ => {}
        }
        if self.graph_listening {
            self.output_worker.pause();
            self.graph_listening = false;
        }
        if matches!(action, Action::Screen(_)) {
            self.cancel_graph_gesture(cx);
        }
        if matches!(action, Action::ResetWorkspace) && self.state.screen == Screen::Compositing {
            self.arrange_graph(cx);
            return;
        }
        if matches!(action, Action::Screen(Screen::Compositing)) {
            if self.state.playing {
                self.state.playing = false;
                self.output_worker.pause();
                self.preview_worker.pause();
            }
            self.seed_composition(window, cx);
        }
        if matches!(
            action,
            Action::Select(_)
                | Action::SelectClip(_)
                | Action::Screen(_)
                | Action::Category(_)
                | Action::MoveTrack { .. }
                | Action::DuplicateClip(_)
                | Action::CutClips
                | Action::PasteClips
                | Action::DeleteTrack(_)
                | Action::AddToTimeline
                | Action::Tool("Split" | "Delete")
        ) {
            self.inline_edit = None;
        }
        if matches!(action, Action::Import | Action::ImportFolder) {
            self.state.apply(action.clone());
            self.choose_media(matches!(action, Action::ImportFolder), window, cx);
            return;
        }
        if matches!(action, Action::CancelAll) && self.importing {
            self.import_cancel.store(true, Ordering::Relaxed);
        }
        let reveal_asset = match action {
            Action::ShowInFinder => Some(self.state.selected),
            Action::ShowAssetInFinder(index) => Some(index),
            _ => None,
        };
        if let Some(index) = reveal_asset
            && let Some(path) = self
                .state
                .assets
                .get(index)
                .and_then(|asset| asset.path.as_ref())
        {
            cx.reveal_path(path);
        }
        if matches!(action, Action::Play)
            && !self.state.playing
            && self.preview_end > 0.
            && self.state.position >= self.preview_end
        {
            self.state.position = self.preview_loop_start;
        }
        if self.state.playing && self.output_worker.clock.running() {
            self.state.position = self.output_worker.clock.position() as f32;
        }
        let preview_changed = matches!(
            action,
            Action::Select(_)
                | Action::MultiSelect(_)
                | Action::SelectClip(_)
                | Action::Category(_)
                | Action::Screen(_)
                | Action::Play
                | Action::Seek(_)
                | Action::Step(_)
                | Action::AddToTimeline
                | Action::Tool("Split" | "Delete")
                | Action::Mute
                | Action::TrackMuted(_)
                | Action::TrackVisible(_)
                | Action::Control(8, _)
                | Action::MoveTrack { .. }
                | Action::DuplicateClip(_)
                | Action::CutClips
                | Action::PasteClips
                | Action::DeleteTrack(_)
                | Action::EditClipTiming { .. }
        ) || (matches!(action, Action::TargetClip(_)) && !self.state.playing);
        let clear = matches!(action, Action::ClearFilters | Action::Category(_));
        let sync_controls = matches!(action, Action::SelectClip(_) | Action::TargetClip(_));
        if matches!(action, Action::ResetWorkspace) {
            let editing = self.state.screen.has_timeline();
            let area = crate::workspace::workspace(cx.weak_entity(), editing, window, cx);
            if editing {
                self.edit_workspace = area;
            } else {
                self.library_workspace = area;
            }
        }
        let reveal_browser = matches!(
            action,
            Action::Category(_) | Action::Folder(_) | Action::AddToTimeline
        );
        if clear || reveal_browser {
            self.media_scroll.set_offset(point(px(0.), px(0.)));
            self.preset_scroll.set_offset(point(px(0.), px(0.)));
        }
        let zooming = matches!(action, Action::Zoom(_));
        let old_zoom = self.state.zoom;
        let offset = self.timeline_scroll.offset();
        let playhead_x = self.state.position * self.timeline_scale + f32::from(offset.x);
        let before = rsx_video_editor::project::Timeline::capture(&self.state);
        self.state.apply(action);
        self.record_edit(before);
        if preview_changed {
            self.refresh_preview(window, cx);
        }
        if zooming {
            let new_scale = self.timeline_scale * self.state.zoom / old_zoom;
            self.timeline_scroll.set_offset(point(
                px((playhead_x - self.state.position * new_scale).min(0.)),
                offset.y,
            ));
            self.timeline_zoom.update(cx, |slider, cx| {
                slider.set_value(self.state.zoom, window, cx)
            });
        }
        if reveal_browser {
            let workspace = if self.state.screen.has_timeline() {
                &self.edit_workspace
            } else {
                &self.library_workspace
            };
            workspace.area.update(cx, |area, cx| {
                area.select_panel(workspace.browser, window, cx)
            });
        }
        if clear {
            self.search
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.header_search
                .update(cx, |input, cx| input.set_value("", window, cx));
        }
        if sync_controls {
            for (i, slider) in self.sliders.iter().enumerate() {
                slider.update(cx, |s, cx| s.set_value(self.state.controls[i], window, cx));
            }
        }
        self.seek.update(cx, |s, cx| {
            s.set_value(self.state.position / self.state.seek_limit(), window, cx)
        });
        cx.notify();
    }
}
impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::generated::ui::editor_view(self, window, cx)
    }
}

#[cfg(test)]
mod scroll_tests {
    use super::timeline_wheel_movement;
    use gpui_kit::{ScrollDelta, point, px};

    #[test]
    fn unmodified_trackpad_follows_the_gesture_axis() {
        assert_eq!(
            timeline_wheel_movement(
                ScrollDelta::Pixels(point(px(-90.), px(-12.))),
                false,
                px(20.)
            ),
            point(px(-90.), px(0.))
        );
        assert_eq!(
            timeline_wheel_movement(ScrollDelta::Pixels(point(px(-90.), px(0.))), false, px(20.)),
            point(px(-90.), px(0.))
        );
        assert_eq!(
            timeline_wheel_movement(ScrollDelta::Pixels(point(px(2.), px(-60.))), false, px(20.)),
            point(px(0.), px(-60.))
        );
        assert_eq!(
            timeline_wheel_movement(ScrollDelta::Pixels(point(px(60.), px(0.))), false, px(20.)),
            point(px(60.), px(0.))
        );
    }

    #[test]
    fn shift_routes_wheel_and_platform_remapped_events_horizontally() {
        for delta in [
            point(px(0.), px(-60.)),
            point(px(-60.), px(0.)),
            point(px(-60.), px(2.)),
        ] {
            assert_eq!(
                timeline_wheel_movement(ScrollDelta::Pixels(delta), true, px(20.)),
                point(px(-60.), px(0.))
            );
        }
        assert_eq!(
            timeline_wheel_movement(ScrollDelta::Pixels(point(px(0.), px(60.))), true, px(20.)),
            point(px(60.), px(0.))
        );
    }

    #[test]
    fn mouse_wheel_lines_use_the_window_line_height() {
        let delta = ScrollDelta::Lines(point(0., -3.));
        assert_eq!(
            timeline_wheel_movement(delta, false, px(18.)),
            point(px(0.), px(-54.))
        );
        assert_eq!(
            timeline_wheel_movement(delta, true, px(18.)),
            point(px(-54.), px(0.))
        );
        assert_eq!(
            timeline_wheel_movement(ScrollDelta::Lines(point(-3., 0.)), false, px(18.)),
            point(px(-54.), px(0.))
        );
    }
}
