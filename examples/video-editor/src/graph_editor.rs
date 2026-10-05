//! Native interaction adapter for the RSX compositing workspace.
use crate::editor::{Editor, NodeGesture};
use gpui_kit::component::input::InputState;
use gpui_kit::{prelude::*, *};
use rsx_video_editor::{
    compositing,
    composition::{Composition, NodeId, Operation},
};
use std::sync::atomic::{AtomicU64, Ordering};

impl Editor {
    pub fn remember_graph(&mut self) {
        if self.graph_history.len() == 100 {
            self.graph_history.remove(0);
        }
        self.graph_history.push(self.composition.clone());
        self.graph_future.clear();
    }
    pub fn undo_graph(&mut self, redo: bool, window: &mut Window, cx: &mut Context<Self>) {
        let snapshot = if redo {
            self.graph_future.pop()
        } else {
            self.graph_history.pop()
        };
        if let Some(mut snapshot) = snapshot {
            snapshot.revision = self.composition.revision + 1;
            if redo {
                self.graph_history.push(self.composition.clone());
            } else {
                self.graph_future.push(self.composition.clone());
            }
            self.composition = snapshot;
            self.graph_drag = None;
            self.composition.pending = None;
            if let Some(id) = self.composition.selected {
                self.select_node(id, window, cx);
            }
            cx.notify();
        }
    }
    pub fn seed_composition(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.composition.seed(&self.state.assets);
        // New workspaces use Blender nodes; the native operations remain compatible with existing graphs.
        let catalog = &rsx_video_editor::blender_catalog::catalog().nodes;
        let blender = |name: &str, settings: &[(&str, String)]| {
            let op = Operation::blender(catalog.iter().position(|n| n.id == name).unwrap());
            let mut values = op
                .parameters()
                .into_iter()
                .map(|(_, v)| v)
                .collect::<Vec<_>>();
            for (key, value) in settings {
                if let Some(i) = op
                    .definition()
                    .unwrap()
                    .parameters
                    .iter()
                    .position(|p| p.key == *key)
                {
                    values[i] = value.clone();
                }
            }
            op.with_parameters(&values).unwrap()
        };
        for node in &mut self.composition.nodes {
            let replacement = match &node.operation {
                Operation::Blur { sigma } => Some(blender(
                    "CompositorNodeBlur",
                    &[
                        ("filter_type", "GAUSS".into()),
                        ("input:1", format!("[{sigma},{sigma}]")),
                    ],
                )),
                Operation::Scale { width } => Some(blender(
                    "CompositorNodeScale",
                    &[
                        ("space", "ABSOLUTE".into()),
                        ("input:1", width.to_string()),
                        ("input:2", (*width as f64 * 9. / 16.).to_string()),
                    ],
                )),
                Operation::Opacity { factor } => Some(blender(
                    "CompositorNodeSetAlpha",
                    &[("mode", "APPLY".into()), ("input:1", factor.to_string())],
                )),
                Operation::VideoOutput => Some(blender("CompositorNodeComposite", &[])),
                _ => None,
            };
            if let Some(operation) = replacement {
                node.inputs.resize(operation.inputs(), None);
                node.input_ports.resize(operation.inputs(), 0);
                node.operation = operation;
            }
        }
        let overlays: Vec<_> = self
            .composition
            .nodes
            .iter()
            .filter_map(|node| {
                if let Operation::Overlay { x, y } = node.operation {
                    Some((node.id, x, y, node.inputs.clone(), node.input_ports.clone()))
                } else {
                    None
                }
            })
            .collect();
        for (id, x, y, inputs, ports) in overlays {
            if let Ok(translate) = self.composition.add(
                blender(
                    "CompositorNodeTranslate",
                    &[("input:1", x.to_string()), ("input:2", (-y).to_string())],
                ),
                [0., 0.],
            ) {
                if let Some(source) = inputs[1] {
                    let _ = self
                        .composition
                        .connect_port(source, ports[1], translate, 0);
                }
                let node = self
                    .composition
                    .nodes
                    .iter_mut()
                    .find(|n| n.id == id)
                    .unwrap();
                node.operation = blender("CompositorNodeAlphaOver", &[]);
                node.inputs = vec![None; node.operation.inputs()];
                node.input_ports = vec![0; node.operation.inputs()];
                node.inputs[1] = inputs[0];
                node.inputs[2] = Some(translate);
                node.input_ports[1] = ports[0];
            }
        }
        self.composition.arrange();
        if let Some(id) = self.composition.selected {
            self.select_node(id, window, cx);
        }
    }
    pub fn select_node(&mut self, id: NodeId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(node) = self.composition.node(id) else {
            return;
        };
        let values = node.operation.parameters();
        let meta = node.operation.definition().cloned();
        self.composition.selected = Some(id);
        while self.graph_inputs.len() < values.len() {
            let input = cx.new(|cx| InputState::new(window, cx));
            cx.subscribe_in(
                &input,
                window,
                |_, _, _: &gpui_kit::component::input::InputEvent, _, cx| cx.notify(),
            )
            .detach();
            self.graph_inputs.push(input);
        }
        self.graph_parts.clear();
        for (i, (_, value)) in values.iter().enumerate() {
            self.graph_inputs[i].update(cx, |input, cx| input.set_value(value.clone(), window, cx));
            let parts = meta
                .as_ref()
                .and_then(|d| d.parameters.get(i))
                .map(|p| rsx_video_editor::blender_catalog::components(p, value))
                .unwrap_or_default();
            self.graph_parts.push(
                parts
                    .into_iter()
                    .map(|(_, value)| {
                        let input = cx.new(|cx| {
                            let mut input = InputState::new(window, cx);
                            input.set_value(value, window, cx);
                            input
                        });
                        cx.subscribe_in(
                            &input,
                            window,
                            |_, _, _: &gpui_kit::component::input::InputEvent, _, cx| cx.notify(),
                        )
                        .detach();
                        input
                    })
                    .collect(),
            );
        }
        cx.notify();
    }
    pub fn browse_graph_asset(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected = self.composition.selected;
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose compositor asset".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = choice.await
                && let Some(path) = paths.first()
            {
                let _ = this.update_in(cx, |this, window, cx| {
                    if this.composition.selected == selected {
                        this.set_graph_parameter(
                            index,
                            path.to_string_lossy().into_owned(),
                            window,
                            cx,
                        );
                    }
                });
            }
        })
        .detach();
    }
    pub fn edit_graph_points(
        &mut self,
        index: usize,
        curve: Option<usize>,
        add: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.composition.selected else {
            return;
        };
        let Ok(mut values) = self.graph_values(cx) else {
            return;
        };
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&values[index]) else {
            return;
        };
        if let Some(curve) = curve {
            if let Some(points) = value.get_mut(curve).and_then(|v| v.as_array_mut()) {
                if add && points.len() < 32 {
                    points.push(serde_json::json!([0.5, 0.5]));
                } else if !add && points.len() > 2 {
                    points.pop();
                }
            }
        } else if let Some(stops) = value.as_array_mut() {
            if add && stops.len() < 32 {
                stops.push(serde_json::json!({"position":0.5,"color":[0.5,0.5,0.5,1]}));
            } else if !add && stops.len() > 2 {
                stops.pop();
            }
        }
        values[index] = value.to_string();
        self.remember_graph();
        if self.composition.update(id, &values).is_ok() {
            self.select_node(id, window, cx);
        }
    }
    pub fn zoom_graph(&mut self, factor: f32, cx: &mut Context<Self>) {
        self.graph_zoom = (self.graph_zoom * factor).clamp(0.35, 1.5);
        cx.notify();
    }
    pub fn fit_graph(&mut self, cx: &mut Context<Self>) {
        let width = self
            .composition
            .nodes
            .iter()
            .map(|n| n.position[0] + rsx_video_editor::composition::NODE_WIDTH + 40.)
            .fold(400., f32::max);
        let height = self
            .composition
            .nodes
            .iter()
            .map(|n| n.position[1] + n.height() + 40.)
            .fold(250., f32::max);
        self.graph_zoom = (self.graph_bounds[0] / width)
            .min(self.graph_bounds[1] / height)
            .clamp(0.35, 1.5);
        self.graph_scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }
    pub fn duplicate_node(&mut self, id: NodeId, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(node) = self.composition.node(id).cloned() {
            self.remember_graph();
            if let Ok(new) = self.composition.add(
                node.operation,
                [node.position[0] + 36., node.position[1] + 36.],
            ) {
                if let Some(copy) = self.composition.nodes.iter_mut().find(|n| n.id == new) {
                    copy.inputs = node.inputs;
                    copy.input_ports = node.input_ports;
                    copy.muted = node.muted;
                }
                self.select_node(new, window, cx);
            }
            cx.notify();
        }
    }
    pub fn frame_branch(&mut self, id: NodeId, window: &mut Window, cx: &mut Context<Self>) {
        self.remember_graph();
        match self.composition.frame_branch(id) {
            Ok(frame) => self.select_node(frame, window, cx),
            Err(error) => self.state.notice = Some(error),
        }
        cx.notify();
    }
    pub fn mute_node(&mut self, id: NodeId, cx: &mut Context<Self>) {
        self.remember_graph();
        if let Some(node) = self.composition.nodes.iter_mut().find(|n| n.id == id) {
            node.muted = !node.muted;
            self.composition.revision += 1;
            cx.notify();
        }
    }
    pub fn add_node(&mut self, operation: Operation, window: &mut Window, cx: &mut Context<Self>) {
        let probe = rsx_video_editor::composition::Node {
            id: 0,
            operation: operation.clone(),
            position: [0., 0.],
            inputs: vec![],
            input_ports: vec![],
            muted: false,
            frame: None,
        };
        let position = (0..20)
            .flat_map(|row| {
                (0..8).map(move |column| [32. + column as f32 * 256., 52. + row as f32 * 178.])
            })
            .find(|position| {
                self.composition
                    .nodes
                    .iter()
                    .filter(|node| !node.is_frame())
                    .all(|node| {
                        position[0] + probe.width() + 12. <= node.position[0]
                            || node.position[0] + probe.width() + 12. <= position[0]
                            || position[1] + probe.height() + 12. <= node.position[1]
                            || node.position[1] + probe.height() + 12. <= position[1]
                    })
            })
            .unwrap_or([32., 52.]);
        self.remember_graph();
        match self.composition.add(operation, position) {
            Ok(id) => {
                self.select_node(id, window, cx);
                let node = self.composition.node(id).unwrap();
                let offset = self.graph_scroll.offset();
                let mut x = f32::from(offset.x);
                let mut y = f32::from(offset.y);
                let left = node.position[0] * self.graph_zoom;
                let top = node.position[1] * self.graph_zoom;
                if left + x < 20. {
                    x = 20. - left;
                } else if left + node.width() * self.graph_zoom + x > self.graph_bounds[0] - 20. {
                    x = self.graph_bounds[0] - 20. - left - node.width() * self.graph_zoom;
                }
                if top + y < 20. {
                    y = 20. - top;
                } else if top + node.height() * self.graph_zoom + y > self.graph_bounds[1] - 20. {
                    y = self.graph_bounds[1] - 20. - top - node.height() * self.graph_zoom;
                }
                self.graph_scroll
                    .set_offset(point(px(x.min(0.)), px(y.min(0.))));
                self.state.notice = None;
            }
            Err(e) => self.state.notice = Some(e),
        }
        cx.notify();
    }
    pub fn graph_values(&self, cx: &Context<Self>) -> Result<Vec<String>, String> {
        let Some(node) = self
            .composition
            .selected
            .and_then(|id| self.composition.node(id))
        else {
            return Ok(vec![]);
        };
        node.operation
            .parameters()
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let text = self.graph_inputs[i].read(cx).value().to_string();
                let parts = self
                    .graph_parts
                    .get(i)
                    .map(|parts| {
                        parts
                            .iter()
                            .map(|input| input.read(cx).value().to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if let Some(meta) = node.operation.parameter_meta(i) {
                    rsx_video_editor::blender_catalog::assemble(meta, &text, &parts)
                } else {
                    Ok(text)
                }
            })
            .collect()
    }
    pub fn apply_node_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.composition.selected else {
            return;
        };
        let values = match self.graph_values(cx) {
            Ok(values) => values,
            Err(error) => {
                self.state.notice = Some(error);
                cx.notify();
                return;
            }
        };
        self.remember_graph();
        if let Err(error) = self.composition.update(id, &values) {
            self.state.notice = Some(error);
            cx.notify();
            return;
        }
        self.state.notice = Some("Settings applied. Render to update the viewer.".into());
        let operation = self.composition.node(id).unwrap().operation.clone();
        if operation.definition().is_some()
            && let Some(cache) = self.cache.clone()
        {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let directory = cache.path.join(format!(
                "node-layout-{}",
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let before = operation.clone();
            cx.spawn_in(window, async move |this, cx| {
                let result = cx
                    .background_spawn(async move {
                        let result =
                            rsx_video_editor::blender_backend::describe(&operation, &directory);
                        let _ = std::fs::remove_dir_all(directory);
                        drop(cache);
                        result
                    })
                    .await;
                let _ = this.update_in(cx, |this, window, cx| {
                    let inputs_unchanged = this.composition.selected != Some(id)
                        || this.graph_values(cx).is_ok_and(|values| {
                            values
                                == before
                                    .parameters()
                                    .into_iter()
                                    .map(|(_, v)| v)
                                    .collect::<Vec<_>>()
                        });
                    if inputs_unchanged
                        && this
                            .composition
                            .node(id)
                            .is_some_and(|node| node.operation == before)
                    {
                        match result {
                            Ok(definition) => {
                                this.composition.refresh_layout(id, definition);
                                if this.composition.selected == Some(id) {
                                    this.select_node(id, window, cx);
                                }
                            }
                            Err(error) => this.state.notice = Some(error),
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }
        cx.notify();
    }
    pub fn set_graph_parameter(
        &mut self,
        index: usize,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(input) = self.graph_inputs.get(index) {
            input.update(cx, |input, cx| input.set_value(value, window, cx));
            cx.notify();
        }
    }
    pub fn output_port(&mut self, id: NodeId, port: usize, cx: &mut Context<Self>) {
        self.composition.pending =
            if self.composition.pending == Some(id) && self.composition.pending_port == port {
                None
            } else {
                Some(id)
            };
        self.composition.pending_port = port;
        self.state.notice = None;
        cx.notify();
    }
    pub fn input_port(&mut self, id: NodeId, port: usize, cx: &mut Context<Self>) {
        if let Some(source) = self.composition.pending {
            self.remember_graph();
            if let Err(e) =
                self.composition
                    .connect_port(source, self.composition.pending_port, id, port)
            {
                self.state.notice = Some(e);
            }
        } else {
            self.state.notice=Some("Click an output port, then the input you want to connect. Right-click an input to disconnect.".into());
        }
        cx.notify();
    }
    pub fn begin_node_drag(
        &mut self,
        id: NodeId,
        pointer: [f32; 2],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_node(id, window, cx);
        self.graph_drag_recorded = false;
        if let Some(node) = self.composition.node(id) {
            let offset = self.graph_scroll.offset();
            self.graph_drag = Some(NodeGesture {
                id,
                pointer,
                position: node.position,
                scroll: [offset.x.into(), offset.y.into()],
            });
            self.graph_focus.focus(window, cx);
            cx.notify();
        }
    }
    pub fn update_node_drag(&mut self, pointer: [f32; 2], cx: &mut Context<Self>) {
        if let Some(NodeGesture {
            id,
            pointer: initial,
            position,
            scroll,
        }) = self.graph_drag
        {
            if !self.graph_drag_recorded {
                if (pointer[0] - initial[0]).abs() + (pointer[1] - initial[1]).abs() < 3. {
                    return;
                }
                self.remember_graph();
                self.graph_drag_recorded = true;
            }
            let offset = self.graph_scroll.offset();
            let mut movement = None;
            if let Some(node) = self.composition.nodes.iter_mut().find(|n| n.id == id) {
                let old = node.position;
                node.position = [
                    (position[0]
                        + (pointer[0] - initial[0] + scroll[0] - f32::from(offset.x))
                            / self.graph_zoom)
                        .clamp(12., 2800.),
                    (position[1]
                        + (pointer[1] - initial[1] + scroll[1] - f32::from(offset.y))
                            / self.graph_zoom)
                        .clamp(12., 1800.),
                ];
                if node.is_frame() {
                    movement = Some([node.position[0] - old[0], node.position[1] - old[1]]);
                }
                cx.notify();
            }
            if let Some(delta) = movement {
                for node in &mut self.composition.nodes {
                    if node.frame == Some(id) {
                        node.position[0] += delta[0];
                        node.position[1] += delta[1];
                    }
                }
            }
        }
    }
    pub fn cancel_graph_gesture(&mut self, cx: &mut Context<Self>) {
        if let Some(NodeGesture { id, position, .. }) = self.graph_drag.take()
            && let Some(node) = self.composition.nodes.iter_mut().find(|n| n.id == id)
        {
            if self.graph_drag_recorded {
                self.graph_history.pop();
            }
            self.graph_drag_recorded = false;
            let delta = [
                position[0] - node.position[0],
                position[1] - node.position[1],
            ];
            let frame = node.is_frame();
            node.position = position;
            if frame {
                for node in &mut self.composition.nodes {
                    if node.frame == Some(id) {
                        node.position[0] += delta[0];
                        node.position[1] += delta[1];
                    }
                }
            }
        }
        self.composition.pending = None;
        cx.notify();
    }
    pub fn delete_node(&mut self, id: NodeId, cx: &mut Context<Self>) {
        self.remember_graph();
        self.graph_drag = None;
        self.composition.remove(id);
        cx.notify();
    }
    pub fn arrange_graph(&mut self, cx: &mut Context<Self>) {
        self.remember_graph();
        self.composition.arrange();
        self.graph_scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }
    pub fn reset_graph(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.remember_graph();
        let revision = self.composition.revision + 1;
        self.composition = Composition::default();
        self.composition.revision = revision;
        self.graph_drag = None;
        self.seed_composition(window, cx);
        self.graph_scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }
    pub fn render_composition(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.graph_rendering {
            return;
        }
        let time = self.graph_time.read(cx).value().trim().parse::<f64>();
        let duration = self.graph_duration.read(cx).value().trim().parse::<f64>();
        let (Ok(time), Ok(duration)) = (time, duration) else {
            self.state.notice = Some("Enter source time and audio duration in seconds.".into());
            cx.notify();
            return;
        };
        if let Err(e) = self.composition.order() {
            self.state.notice = Some(e);
            cx.notify();
            return;
        }
        let Some(cache) = self.cache.clone() else {
            self.state.notice = Some("A writable media cache is needed for rendering.".into());
            cx.notify();
            return;
        };
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let directory = cache.path.join(format!(
            "composite-{}",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let graph = self.composition.clone();
        let assets = self.state.assets.clone();
        let revision = graph.revision;
        self.output_worker.pause();
        self.graph_listening = false;
        self.graph_render_revision = None;
        self.graph_rendering = true;
        self.state.notice = None;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let result = compositing::render(&graph, &assets, time, duration, &directory);
                    drop(cache);
                    result
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.graph_rendering = false;
                match result {
                    Ok(mut result) => {
                        if let Some(old) = this.graph_frame.take() {
                            let _ = window.drop_image(old);
                        }
                        this.graph_frame = result.video.take().map(|f| f.render_image());
                        let kind = match (result.image.is_some(), result.audio.is_some()) {
                            (true, true) => "a video frame and stereo audio excerpt",
                            (true, false) => "a video frame",
                            _ => "a stereo audio excerpt",
                        };
                        this.state.notice = Some(format!(
                            "Rendered {kind}. Add the result to use it in the project."
                        ));
                        this.graph_result = Some(result);
                        this.graph_render_revision = Some(revision);
                    }
                    Err(e) => this.state.notice = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn add_composition_result(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.graph_render_revision != Some(self.composition.revision) {
            self.state.notice = Some("Render the current graph before adding its result.".into());
            cx.notify();
            return;
        }
        let paths = self
            .graph_result
            .as_ref()
            .map(|r| {
                r.image
                    .iter()
                    .chain(r.audio.iter())
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !paths.is_empty() {
            self.import_paths_mode(paths, true, window, cx);
        }
    }
}

impl Editor {
    pub fn listen_composition(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.graph_listening {
            self.output_worker.pause();
            self.graph_listening = false;
            cx.notify();
            return;
        }
        if self.graph_render_revision != Some(self.composition.revision) {
            return;
        }
        let Some(media) = self
            .graph_result
            .as_ref()
            .and_then(|r| r.audio_media.clone())
        else {
            return;
        };
        let mut plan = rsx_video_editor::playback::PlaybackPlan::default();
        for stream in &media.audio {
            for channel in &stream.channels {
                plan.audio.push(rsx_video_editor::playback::AudioRegion {
                    media: media.clone(),
                    stream: stream.index,
                    channel: channel.index,
                    start: 0.,
                    duration: stream.duration(),
                    source_start: 0.,
                    gain: 1.,
                    pan: if channel.index == 0 {
                        [1., 0.]
                    } else {
                        [0., 1.]
                    },
                });
                plan.end = plan.end.max(stream.duration());
            }
        }
        self.state.playing = false;
        self.preview_worker.pause();
        self.transport_generation = self.transport_generation.wrapping_add(1);
        let generation = self.transport_generation;
        self.output_worker.play(plan, 0., generation);
        self.graph_listening = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
                let keep = this
                    .update(cx, |this, cx| {
                        if this.transport_generation != generation || !this.graph_listening {
                            return false;
                        }
                        if this.output_worker.clock.ended() {
                            this.graph_listening = false;
                            cx.notify();
                            return false;
                        }
                        true
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        })
        .detach();
    }
}
