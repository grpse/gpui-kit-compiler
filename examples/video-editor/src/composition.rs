//! Typed, project-local compositing graph. Edits never mutate imported media.
use crate::state::{Asset, ClipComponent};
use std::collections::{BTreeMap, BTreeSet};

pub type NodeId = u64;
pub const NODE_WIDTH: f32 = 220.;
pub const NODE_HEIGHT: f32 = 140.;
pub const PORT_Y: f32 = 96.;
pub const PORT_SPACING: f32 = 24.;
pub const MAX_NODES: usize = 256;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Video,
    Audio,
}
impl Signal {
    pub fn label(self) -> &'static str {
        match self {
            Self::Video => "Video",
            Self::Audio => "Audio",
        }
    }
}
/// Shared by the palette, context menu and node headers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Input,
    Transform,
    Color,
    Filter,
    Composite,
    Audio,
    Output,
    Keying,
    Mask,
    Tracking,
    Utilities,
    Vector,
    Layout,
    Texture,
}
impl Category {
    pub const ALL: [Self; 14] = [
        Self::Input,
        Self::Transform,
        Self::Color,
        Self::Filter,
        Self::Composite,
        Self::Audio,
        Self::Output,
        Self::Keying,
        Self::Mask,
        Self::Tracking,
        Self::Utilities,
        Self::Vector,
        Self::Layout,
        Self::Texture,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Input => "Input",
            Self::Transform => "Transform",
            Self::Color => "Color",
            Self::Filter => "Filter",
            Self::Composite => "Composite",
            Self::Audio => "Audio",
            Self::Output => "Output",
            Self::Keying => "Keying",
            Self::Mask => "Mask",
            Self::Tracking => "Tracking",
            Self::Utilities => "Utilities",
            Self::Vector => "Vector",
            Self::Layout => "Layout",
            Self::Texture => "Texture",
        }
    }
    pub fn color(self) -> u32 {
        match self {
            Self::Input => 0x68b9cf,
            Self::Transform => 0x7b9ee0,
            Self::Color => 0xd8b36a,
            Self::Filter => 0xa595d6,
            Self::Composite => 0xdb8b9b,
            Self::Audio => 0x64c5a0,
            Self::Output => 0x9da9bb,
            Self::Keying => 0x97bf83,
            Self::Mask => 0xbeaa80,
            Self::Tracking => 0x72b0a9,
            Self::Utilities => 0x8d9db5,
            Self::Vector => 0x8098df,
            Self::Layout => 0x88919f,
            Self::Texture => 0xc38f70,
        }
    }
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Operation {
    Blender {
        kind: usize,
        values: Vec<String>,
        layout: Option<Box<crate::blender_catalog::Definition>>,
    },
    Source {
        asset: usize,
        component: ClipComponent,
    },
    Scale {
        width: u32,
    },
    Flip,
    Color {
        saturation: f64,
    },
    Overlay {
        x: i32,
        y: i32,
    },
    Blur {
        sigma: f64,
    },
    Exposure {
        stops: f64,
    },
    Opacity {
        factor: f64,
    },
    LowPass {
        hz: f64,
    },
    HighPass {
        hz: f64,
    },
    Gain {
        db: f64,
    },
    Mix,
    VideoOutput,
    AudioOutput,
}
impl Operation {
    pub fn blender(kind: usize) -> Self {
        let values = crate::blender_catalog::catalog().nodes[kind]
            .parameters
            .iter()
            .map(|p| p.text())
            .collect();
        Self::Blender {
            kind,
            values,
            layout: None,
        }
    }
    pub fn definition(&self) -> Option<&crate::blender_catalog::Definition> {
        match self {
            Self::Blender { kind, layout, .. } => Some(
                layout
                    .as_deref()
                    .unwrap_or(&crate::blender_catalog::catalog().nodes[*kind]),
            ),
            _ => None,
        }
    }
    pub fn outputs(&self) -> usize {
        self.definition()
            .map_or_else(|| usize::from(!self.is_output()), |d| d.outputs.len())
    }
    pub fn visible_inputs(&self) -> Vec<usize> {
        self.definition().map_or_else(
            || (0..self.inputs()).collect(),
            |d| {
                d.inputs
                    .iter()
                    .filter(|s| s.enabled)
                    .map(|s| s.index)
                    .collect()
            },
        )
    }
    pub fn visible_outputs(&self) -> Vec<usize> {
        self.definition().map_or_else(
            || (0..self.outputs()).collect(),
            |d| {
                d.outputs
                    .iter()
                    .filter(|s| s.enabled)
                    .map(|s| s.index)
                    .collect()
            },
        )
    }
    pub fn socket_kind(&self, port: usize, output: bool) -> &str {
        if let Some(d) = self.definition()
            && let Some(s) = if output {
                d.outputs.get(port)
            } else {
                d.inputs.get(port)
            }
        {
            return &s.kind;
        }
        if self.signal() == Signal::Audio {
            "AUDIO"
        } else {
            "RGBA"
        }
    }
    pub fn output_label(&self, port: usize) -> String {
        self.definition()
            .and_then(|d| d.outputs.get(port))
            .map(|s| s.name.clone())
            .unwrap_or_else(|| {
                if self.signal() == Signal::Audio {
                    "Audio".into()
                } else {
                    "Image".into()
                }
            })
    }
    pub fn parameter_meta(&self, index: usize) -> Option<&crate::blender_catalog::Parameter> {
        self.definition()?.parameters.get(index)
    }
    pub fn settings(&self) -> Result<serde_json::Value, String> {
        let mut map = serde_json::Map::new();
        if let Self::Blender { values, .. } = self {
            for (parameter, value) in self.definition().unwrap().parameters.iter().zip(values) {
                map.insert(parameter.key.clone(), parameter.parse(value)?);
            }
        }
        Ok(serde_json::Value::Object(map))
    }
    pub fn with_layout(&self, definition: crate::blender_catalog::Definition) -> Self {
        let Self::Blender { kind, values, .. } = self else {
            return self.clone();
        };
        let current = self.definition().unwrap();
        let values = definition
            .parameters
            .iter()
            .map(|p| {
                current
                    .parameters
                    .iter()
                    .position(|old| old.key == p.key)
                    .and_then(|i| values.get(i))
                    .filter(|value| p.parse(value).is_ok())
                    .cloned()
                    .unwrap_or_else(|| p.text())
            })
            .collect();
        Self::Blender {
            kind: *kind,
            values,
            layout: Some(Box::new(definition)),
        }
    }
    pub fn category(&self) -> Category {
        match self {
            Self::Blender { .. } => Category::ALL
                .into_iter()
                .find(|c| {
                    c.label() == self.definition().unwrap().category
                        || (*c == Category::Input && self.definition().unwrap().category == "Input")
                })
                .unwrap_or(Category::Color),
            Self::Source { .. } => Category::Input,
            Self::Scale { .. } | Self::Flip => Category::Transform,
            Self::Color { .. } | Self::Exposure { .. } => Category::Color,
            Self::Blur { .. } => Category::Filter,
            Self::Overlay { .. } | Self::Opacity { .. } => Category::Composite,
            Self::Gain { .. } | Self::Mix | Self::LowPass { .. } | Self::HighPass { .. } => {
                Category::Audio
            }
            Self::VideoOutput | Self::AudioOutput => Category::Output,
        }
    }
    pub fn palette() -> Vec<Self> {
        let mut nodes: Vec<_> = (0..crate::blender_catalog::catalog().nodes.len())
            .map(Self::blender)
            .collect();
        nodes.extend([
            Self::Gain { db: 0. },
            Self::Mix,
            Self::LowPass { hz: 8000. },
            Self::HighPass { hz: 80. },
            Self::AudioOutput,
        ]);
        nodes
    }
    pub fn input_label(&self, port: usize) -> String {
        if let Some(s) = self.definition().and_then(|d| d.inputs.get(port)) {
            return s.name.clone();
        }
        (match (self, port) {
            (Self::Overlay { .. }, 0) => "Background",
            (Self::Overlay { .. }, _) => "Foreground",
            (Self::Mix, 0) => "Audio A",
            (Self::Mix, _) => "Audio B",
            _ if self.signal() == Signal::Video => "Image",
            _ => "Audio",
        })
        .into()
    }
    pub fn signal(&self) -> Signal {
        match self {
            Self::Source {
                component: ClipComponent::AudioChannel { .. },
                ..
            }
            | Self::LowPass { .. }
            | Self::HighPass { .. }
            | Self::Gain { .. }
            | Self::Mix
            | Self::AudioOutput => Signal::Audio,
            _ => Signal::Video,
        }
    }
    pub fn inputs(&self) -> usize {
        match self {
            Self::Blender { .. } => self.definition().unwrap().inputs.len(),
            Self::Source { .. } => 0,
            Self::Overlay { .. } | Self::Mix => 2,
            _ => 1,
        }
    }
    pub fn is_output(&self) -> bool {
        matches!(self, Self::VideoOutput | Self::AudioOutput)
            || self.definition().is_some_and(|d| {
                matches!(
                    d.id.as_str(),
                    "CompositorNodeComposite" | "CompositorNodeViewer" | "CompositorNodeOutputFile"
                )
            })
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Blender { kind, .. } => &crate::blender_catalog::catalog().nodes[*kind].label,
            Self::Source { .. } => "Media source",
            Self::Scale { .. } => "Scale",
            Self::Flip => "Horizontal flip",
            Self::Color { .. } => "Saturation",
            Self::Overlay { .. } => "Alpha over",
            Self::Blur { .. } => "Gaussian blur",
            Self::Exposure { .. } => "Exposure",
            Self::Opacity { .. } => "Opacity",
            Self::LowPass { .. } => "Low-pass filter",
            Self::HighPass { .. } => "High-pass filter",
            Self::Gain { .. } => "Gain",
            Self::Mix => "Audio mix",
            Self::VideoOutput => "Composite",
            Self::AudioOutput => "Audio output",
        }
    }
    pub fn summary(&self) -> String {
        match self {
            Self::Blender { .. } => self.definition().unwrap().description.clone(),
            Self::Source { component, .. } => match component {
                ClipComponent::Video(stream) => format!("Video stream {stream}"),
                ClipComponent::AudioChannel { stream, channel } => {
                    format!("Stream {stream} · channel {}", channel + 1)
                }
            },
            Self::Scale { width } => format!("{width}px · Lanczos"),
            Self::Flip => "Mirror horizontally".into(),
            Self::Color { saturation } => format!("Saturation {saturation:.2}"),
            Self::Overlay { x, y } => format!("Foreground at {x}, {y}"),
            Self::Blur { sigma } => format!("Sigma {sigma:.1} px"),
            Self::Exposure { stops } => format!("{stops:+.2} stops"),
            Self::Opacity { factor } => format!("Alpha × {factor:.2}"),
            Self::LowPass { hz } | Self::HighPass { hz } => format!("Cutoff {hz:.0} Hz"),
            Self::Gain { db } => format!("{db:+.1} dB"),
            Self::Mix => "Two inputs · normalized mix".into(),
            Self::VideoOutput => "Render one preview frame".into(),
            Self::AudioOutput => "Render stereo audio excerpt".into(),
        }
    }
    pub fn parameters(&self) -> Vec<(String, String)> {
        if let Self::Blender { values, .. } = self {
            return self
                .definition()
                .unwrap()
                .parameters
                .iter()
                .zip(values)
                .map(|(p, v)| (p.label.clone(), v.clone()))
                .collect();
        }
        (match self {
            Self::Scale { width } => vec![("Width (2–960 px)", width.to_string())],
            Self::Color { saturation } => vec![("Saturation (0–3)", saturation.to_string())],
            Self::Overlay { x, y } => vec![
                ("Horizontal position (px)", x.to_string()),
                ("Vertical position (px)", y.to_string()),
            ],
            Self::Blur { sigma } => vec![("Sigma (0–20 px)", sigma.to_string())],
            Self::Exposure { stops } => vec![("Exposure (−4 to +4 stops)", stops.to_string())],
            Self::Opacity { factor } => vec![("Opacity (0–1)", factor.to_string())],
            Self::LowPass { hz } | Self::HighPass { hz } => {
                vec![("Cutoff (20–20000 Hz)", hz.to_string())]
            }
            Self::Gain { db } => vec![("Gain (−60 to +12 dB)", db.to_string())],
            _ => vec![],
        })
        .into_iter()
        .map(|(name, value)| (name.into(), value))
        .collect()
    }
    pub fn with_parameters(&self, values: &[String]) -> Result<Self, String> {
        let number = |i: usize| {
            values
                .get(i)
                .and_then(|s| s.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite())
                .ok_or_else(|| "Enter a finite number for each setting.".to_string())
        };
        if let Self::Blender { kind, layout, .. } = self {
            let definition = self.definition().unwrap();
            if values.len() < definition.parameters.len() {
                return Err("Missing node settings.".into());
            }
            for (parameter, value) in definition.parameters.iter().zip(values) {
                parameter.parse(value)?;
            }
            return Ok(Self::Blender {
                kind: *kind,
                values: values[..definition.parameters.len()].to_vec(),
                layout: layout.clone(),
            });
        }
        Ok(match self {
            Self::Scale { .. } => {
                let v = number(0)?;
                if !(2. ..=960.).contains(&v) || v.fract() != 0. {
                    return Err("Width must be a whole number from 2 to 960.".into());
                }
                Self::Scale { width: v as u32 }
            }
            Self::Color { .. } => {
                let v = number(0)?;
                if !(0. ..=3.).contains(&v) {
                    return Err("Saturation must be from 0 to 3.".into());
                }
                Self::Color { saturation: v }
            }
            Self::Overlay { .. } => {
                let x = number(0)?;
                let y = number(1)?;
                if [x, y]
                    .iter()
                    .any(|v| !(-1920. ..=1920.).contains(v) || v.fract() != 0.)
                {
                    return Err(
                        "Overlay coordinates must be whole pixels from −1920 to 1920.".into(),
                    );
                }
                Self::Overlay {
                    x: x as i32,
                    y: y as i32,
                }
            }
            Self::Blur { .. } => {
                let sigma = number(0)?;
                if !(0. ..=20.).contains(&sigma) {
                    return Err("Blur sigma must be from 0 to 20 pixels.".into());
                }
                Self::Blur { sigma }
            }
            Self::Exposure { .. } => {
                let stops = number(0)?;
                if !(-4. ..=4.).contains(&stops) {
                    return Err("Exposure must be from −4 to +4 stops.".into());
                }
                Self::Exposure { stops }
            }
            Self::Opacity { .. } => {
                let factor = number(0)?;
                if !(0. ..=1.).contains(&factor) {
                    return Err("Opacity must be from 0 to 1.".into());
                }
                Self::Opacity { factor }
            }
            Self::LowPass { .. } | Self::HighPass { .. } => {
                let hz = number(0)?;
                if !(20. ..=20000.).contains(&hz) {
                    return Err("Cutoff must be from 20 to 20000 Hz.".into());
                }
                if matches!(self, Self::LowPass { .. }) {
                    Self::LowPass { hz }
                } else {
                    Self::HighPass { hz }
                }
            }
            Self::Gain { .. } => {
                let v = number(0)?;
                if !(-60. ..=12.).contains(&v) {
                    return Err("Gain must be from −60 to +12 dB.".into());
                }
                Self::Gain { db: v }
            }
            _ => self.clone(),
        })
    }
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub operation: Operation,
    pub position: [f32; 2],
    pub inputs: Vec<Option<NodeId>>,
    pub input_ports: Vec<usize>,
    pub muted: bool,
    pub frame: Option<NodeId>,
}
impl Node {
    pub fn is_frame(&self) -> bool {
        self.operation
            .definition()
            .is_some_and(|d| d.id == "NodeFrame")
    }
    pub fn width(&self) -> f32 {
        if self
            .operation
            .definition()
            .is_some_and(|d| d.id == "NodeReroute")
        {
            80.
        } else {
            NODE_WIDTH
        }
    }
    pub fn height(&self) -> f32 {
        if self
            .operation
            .definition()
            .is_some_and(|d| d.id == "NodeReroute")
        {
            return 48.;
        }
        (PORT_Y
            + self
                .operation
                .visible_inputs()
                .len()
                .max(self.operation.visible_outputs().len()) as f32
                * PORT_SPACING
            + 10.)
            .max(NODE_HEIGHT)
    }
    pub fn input_y(&self, port: usize) -> f32 {
        if self
            .operation
            .definition()
            .is_some_and(|d| d.id == "NodeReroute")
        {
            return 24.;
        }
        PORT_Y
            + self
                .operation
                .visible_inputs()
                .iter()
                .position(|&p| p == port)
                .unwrap_or(0) as f32
                * PORT_SPACING
    }
    pub fn output_y(&self, port: usize) -> f32 {
        if self
            .operation
            .definition()
            .is_some_and(|d| d.id == "NodeReroute")
        {
            return 24.;
        }
        PORT_Y
            + self
                .operation
                .visible_outputs()
                .iter()
                .position(|&p| p == port)
                .unwrap_or(0) as f32
                * PORT_SPACING
    }
}
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Composition {
    pub nodes: Vec<Node>,
    pub selected: Option<NodeId>,
    pub pending: Option<NodeId>,
    pub pending_port: usize,
    pub revision: u64,
    next_id: NodeId,
}
impl Composition {
    pub fn validate_project(&mut self, assets: usize) -> Result<(), String> {
        if self.nodes.len() > MAX_NODES {
            return Err("Too many composition nodes".into());
        }
        let ids: BTreeSet<_> = self.nodes.iter().map(|node| node.id).collect();
        if ids.len() != self.nodes.len() {
            return Err("Duplicate composition node IDs".into());
        }
        for node in &self.nodes {
            if !node.position.iter().all(|v| v.is_finite())
                || node.inputs.iter().flatten().any(|id| !ids.contains(id))
                || node.frame.is_some_and(|id| !ids.contains(&id))
            {
                return Err("Invalid composition references".into());
            }
            match &node.operation {
                Operation::Source { asset, .. } if *asset >= assets => {
                    return Err("Missing composition source".into());
                }
                Operation::Blender { kind, .. }
                    if *kind >= crate::blender_catalog::catalog().nodes.len() =>
                {
                    return Err("Unknown composition node".into());
                }
                _ => {}
            }
            if node.inputs.len() != node.operation.inputs()
                || node.input_ports.len() != node.inputs.len()
            {
                return Err("Invalid composition sockets".into());
            }
        }
        self.next_id = ids.last().copied().unwrap_or(0);
        if self.next_id == u64::MAX {
            return Err("Invalid composition IDs".into());
        }
        self.pending = None;
        self.selected = self.selected.filter(|id| ids.contains(id));
        Ok(())
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub fn add(&mut self, operation: Operation, position: [f32; 2]) -> Result<NodeId, String> {
        if self.nodes.len() >= MAX_NODES {
            return Err(format!(
                "This preview graph supports up to {MAX_NODES} nodes."
            ));
        }
        if matches!(operation, Operation::VideoOutput | Operation::AudioOutput)
            && self.nodes.iter().any(|n| n.operation == operation)
        {
            return Err("There is already an output for this media type.".into());
        }
        self.next_id += 1;
        let id = self.next_id;
        let inputs = vec![None; operation.inputs()];
        self.nodes.push(Node {
            id,
            operation,
            position,
            input_ports: vec![0; inputs.len()],
            muted: false,
            frame: None,
            inputs,
        });
        self.selected = Some(id);
        self.revision += 1;
        Ok(id)
    }
    pub fn remove(&mut self, id: NodeId) {
        self.nodes.retain(|n| n.id != id);
        for node in &mut self.nodes {
            if node.frame == Some(id) {
                node.frame = None;
            }
            for input in &mut node.inputs {
                if *input == Some(id) {
                    *input = None;
                }
            }
        }
        if self.selected == Some(id) {
            self.selected = None;
        }
        if self.pending == Some(id) {
            self.pending = None;
        }
        self.revision += 1;
    }
    pub fn frame_branch(&mut self, id: NodeId) -> Result<NodeId, String> {
        let mut branch = BTreeSet::new();
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            if branch.insert(id) {
                let node = self.node(id).ok_or("Node no longer exists")?;
                pending.extend(node.inputs.iter().flatten().copied());
            }
        }
        let x = self
            .nodes
            .iter()
            .filter(|n| branch.contains(&n.id))
            .map(|n| n.position[0])
            .fold(f32::MAX, f32::min)
            - 24.;
        let y = self
            .nodes
            .iter()
            .filter(|n| branch.contains(&n.id))
            .map(|n| n.position[1])
            .fold(f32::MAX, f32::min)
            - 42.;
        let kind = crate::blender_catalog::catalog()
            .nodes
            .iter()
            .position(|n| n.id == "NodeFrame")
            .unwrap();
        let frame = self.add(Operation::blender(kind), [x.max(12.), y.max(12.)])?;
        for node in &mut self.nodes {
            if branch.contains(&node.id) {
                node.frame = Some(frame);
            }
        }
        Ok(frame)
    }
    pub fn frame_size(&self, frame: &Node) -> [f32; 2] {
        self.nodes
            .iter()
            .filter(|n| n.frame == Some(frame.id))
            .fold([320., 180.], |size, n| {
                [
                    size[0].max(n.position[0] + n.width() + 24. - frame.position[0]),
                    size[1].max(n.position[1] + n.height() + 24. - frame.position[1]),
                ]
            })
    }
    pub fn connect(&mut self, source: NodeId, target: NodeId, port: usize) -> Result<(), String> {
        self.connect_port(source, 0, target, port)
    }
    pub fn connect_port(
        &mut self,
        source: NodeId,
        output: usize,
        target: NodeId,
        port: usize,
    ) -> Result<(), String> {
        let from = self.node(source).ok_or("Source node no longer exists")?;
        let to = self.node(target).ok_or("Target node no longer exists")?;
        if output >= from.operation.outputs() {
            return Err("An output node cannot feed another node.".into());
        }
        if from.operation.signal() != to.operation.signal() {
            return Err("Connect video to video, or audio to audio.".into());
        }
        if port >= to.inputs.len() {
            return Err("This input port does not exist.".into());
        }
        let mut pending = vec![source];
        let mut seen = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if id == target {
                return Err("This connection would create a cycle.".into());
            }
            if seen.insert(id) {
                pending.extend(
                    self.node(id)
                        .into_iter()
                        .flat_map(|n| n.inputs.iter().flatten())
                        .copied(),
                );
            }
        }
        self.nodes
            .iter_mut()
            .find(|n| n.id == target)
            .unwrap()
            .inputs[port] = Some(source);
        self.nodes
            .iter_mut()
            .find(|n| n.id == target)
            .unwrap()
            .input_ports[port] = output;
        self.pending = None;
        self.revision += 1;
        Ok(())
    }
    pub fn disconnect(&mut self, target: NodeId, port: usize) {
        if let Some(input) = self
            .nodes
            .iter_mut()
            .find(|n| n.id == target)
            .and_then(|n| n.inputs.get_mut(port))
        {
            *input = None;
            self.revision += 1;
        }
    }
    pub fn update(&mut self, id: NodeId, values: &[String]) -> Result<(), String> {
        let node = self
            .nodes
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or("Node no longer exists")?;
        node.operation = node.operation.with_parameters(values)?;
        self.revision += 1;
        Ok(())
    }
    pub fn refresh_layout(&mut self, id: NodeId, definition: crate::blender_catalog::Definition) {
        let Some(node) = self.nodes.iter_mut().find(|n| n.id == id) else {
            return;
        };
        let old = node.operation.definition().cloned();
        let previous = |index: usize, socket: &crate::blender_catalog::Socket| {
            old.as_ref().and_then(|d| {
                let occurrence = definition.inputs[..index]
                    .iter()
                    .filter(|s| s.name == socket.name && s.kind == socket.kind)
                    .count();
                d.inputs
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.name == socket.name && s.kind == socket.kind)
                    .nth(occurrence)
                    .map(|(i, _)| i)
            })
        };
        let inputs = definition
            .inputs
            .iter()
            .enumerate()
            .map(|(i, socket)| {
                if socket.enabled {
                    previous(i, socket)
                        .and_then(|i| node.inputs.get(i).copied())
                        .flatten()
                } else {
                    None
                }
            })
            .collect();
        let ports = definition
            .inputs
            .iter()
            .enumerate()
            .map(|(i, socket)| {
                previous(i, socket)
                    .and_then(|i| node.input_ports.get(i).copied())
                    .unwrap_or(0)
            })
            .collect();
        node.operation = node.operation.with_layout(definition);
        node.inputs = inputs;
        node.input_ports = ports;
        let outputs = node.operation.definition().unwrap().outputs.clone();
        for target in &mut self.nodes {
            for i in 0..target.inputs.len() {
                if target.inputs[i] == Some(id) {
                    let mapped = old
                        .as_ref()
                        .and_then(|d| d.outputs.get(target.input_ports[i]))
                        .and_then(|old| {
                            outputs
                                .iter()
                                .position(|s| s.name == old.name && s.kind == old.kind && s.enabled)
                        });
                    if let Some(port) = mapped {
                        target.input_ports[i] = port;
                    } else {
                        target.inputs[i] = None;
                    }
                }
            }
        }
        self.revision += 1;
    }
    /// Only dependencies of connected outputs are required. Unused palette nodes are allowed.
    pub fn order(&self) -> Result<Vec<NodeId>, String> {
        fn visit(
            graph: &Composition,
            id: NodeId,
            marks: &mut BTreeMap<NodeId, u8>,
            out: &mut Vec<NodeId>,
        ) -> Result<(), String> {
            match marks.get(&id) {
                Some(1) => return Err("Graph contains a cycle.".into()),
                Some(2) => return Ok(()),
                _ => {}
            }
            let node = graph
                .node(id)
                .ok_or("Connection references a missing node")?;
            marks.insert(id, 1);
            for (i, input) in node.inputs.iter().enumerate() {
                if input.is_none() && node.operation.definition().is_some() {
                    continue;
                }
                let input = input.ok_or_else(|| {
                    format!("Connect input {} of {}.", i + 1, node.operation.label())
                })?;
                let source = graph.node(input).ok_or("Missing source")?;
                if source.operation.signal() != node.operation.signal()
                    || node.input_ports[i] >= source.operation.outputs()
                {
                    return Err("Invalid media connection.".into());
                }
                visit(graph, input, marks, out)?;
            }
            marks.insert(id, 2);
            out.push(id);
            Ok(())
        }
        let outputs: Vec<_> = self
            .nodes
            .iter()
            .filter(|n| n.operation.is_output() && n.inputs.iter().any(Option::is_some))
            .collect();
        if outputs.is_empty() {
            return Err("Connect at least one Video or Audio output before rendering.".into());
        }
        let mut marks = BTreeMap::new();
        let mut order = vec![];
        for node in outputs {
            visit(self, node.id, &mut marks, &mut order)?;
        }
        Ok(order)
    }
    pub fn source_label(node: &Node, assets: &[Asset]) -> String {
        if let Operation::Source { asset, component } = node.operation {
            let name = assets
                .get(asset)
                .map(|a| a.name.as_str())
                .unwrap_or("Missing media");
            match component {
                ClipComponent::Video(_) => name.into(),
                ClipComponent::AudioChannel { stream, channel } => {
                    let label = assets
                        .get(asset)
                        .and_then(|a| a.prepared.as_ref())
                        .and_then(|m| m.audio.iter().find(|a| a.index == stream))
                        .and_then(|a| a.channels.get(channel))
                        .map(|c| c.name.as_str())
                        .unwrap_or("Audio");
                    format!("{name} · {label}")
                }
            }
        } else {
            node.operation.label().into()
        }
    }
    /// Lay out dependency columns with independent video and audio lanes.
    pub fn arrange(&mut self) {
        let mut levels = BTreeMap::<NodeId, usize>::new();
        for _ in 0..self.nodes.len() {
            for node in &self.nodes {
                let depth = node
                    .inputs
                    .iter()
                    .flatten()
                    .map(|id| levels.get(id).copied().unwrap_or(0) + 1)
                    .max()
                    .unwrap_or(0);
                levels.insert(node.id, depth);
            }
        }
        let mut top = 52.;
        let mut positions = BTreeMap::<NodeId, f32>::new();
        for signal in [Signal::Video, Signal::Audio] {
            let mut columns = BTreeMap::<usize, Vec<(f32, f32)>>::new();
            let mut indices: Vec<_> = self
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| n.operation.signal() == signal && !n.is_frame())
                .map(|(i, _)| i)
                .collect();
            indices.sort_by_key(|&i| levels[&self.nodes[i].id]);
            let mut bottom = top;
            for index in indices {
                let node = &mut self.nodes[index];
                let level = levels[&node.id];
                let parents: Vec<_> = node
                    .inputs
                    .iter()
                    .flatten()
                    .filter_map(|id| positions.get(id))
                    .copied()
                    .collect();
                let mut y = if parents.is_empty() {
                    top
                } else {
                    parents.iter().sum::<f32>() / parents.len() as f32
                };
                let occupied = columns.entry(level).or_default();
                while occupied.iter().any(|&(other, height)| {
                    y < other + height + 38. && y + node.height() + 38. > other
                }) {
                    y += NODE_HEIGHT + 38.;
                }
                node.position = [32. + level as f32 * (NODE_WIDTH + 36.), y];
                occupied.push((y, node.height()));
                positions.insert(node.id, y);
                bottom = bottom.max(y + node.height());
            }
            top = bottom + 60.;
        }
        let positions: Vec<_> = self
            .nodes
            .iter()
            .filter(|n| n.is_frame())
            .filter_map(|frame| {
                let children: Vec<_> = self
                    .nodes
                    .iter()
                    .filter(|n| n.frame == Some(frame.id))
                    .collect();
                if children.is_empty() {
                    None
                } else {
                    Some((
                        frame.id,
                        [
                            children
                                .iter()
                                .map(|n| n.position[0])
                                .fold(f32::MAX, f32::min)
                                - 24.,
                            children
                                .iter()
                                .map(|n| n.position[1])
                                .fold(f32::MAX, f32::min)
                                - 42.,
                        ],
                    ))
                }
            })
            .collect();
        for (id, position) in positions {
            self.nodes.iter_mut().find(|n| n.id == id).unwrap().position = position;
        }
    }
    pub fn seed(&mut self, assets: &[Asset]) {
        if !self.nodes.is_empty() {
            return;
        }
        if let Some((asset, media)) = assets.iter().enumerate().find_map(|(i, a)| {
            a.prepared
                .as_ref()
                .filter(|m| !m.videos.is_empty())
                .map(|m| (i, m))
        }) {
            let source = self
                .add(
                    Operation::Source {
                        asset,
                        component: ClipComponent::Video(media.videos[0].index),
                    },
                    [40., 40.],
                )
                .unwrap();
            // Blender's two-branch Alpha Over example, adapted to bundled footage:
            // soften the background and place a translucent inset above it.
            let blur = self.add(Operation::Blur { sigma: 3. }, [0., 0.]).unwrap();
            let scale = self.add(Operation::Scale { width: 320 }, [0., 0.]).unwrap();
            let opacity = self
                .add(Operation::Opacity { factor: 0.85 }, [0., 0.])
                .unwrap();
            let overlay = self
                .add(Operation::Overlay { x: 24, y: 24 }, [0., 0.])
                .unwrap();
            let out = self.add(Operation::VideoOutput, [0., 0.]).unwrap();
            self.connect(source, blur, 0).unwrap();
            self.connect(source, scale, 0).unwrap();
            self.connect(scale, opacity, 0).unwrap();
            self.connect(blur, overlay, 0).unwrap();
            self.connect(opacity, overlay, 1).unwrap();
            self.connect(overlay, out, 0).unwrap();
        }
        let preferred_audio = assets.iter().position(|a| {
            a.prepared
                .as_ref()
                .is_some_and(|m| !m.videos.is_empty() && !m.audio.is_empty())
        });
        let channels: Vec<_> = assets
            .iter()
            .enumerate()
            .filter(|(i, _)| preferred_audio.is_none_or(|preferred| preferred == *i))
            .flat_map(|(asset, a)| {
                a.prepared.iter().flat_map(move |m| {
                    m.audio.iter().flat_map(move |s| {
                        s.channels.iter().map(move |c| {
                            (
                                asset,
                                ClipComponent::AudioChannel {
                                    stream: s.index,
                                    channel: c.index,
                                },
                            )
                        })
                    })
                })
            })
            .take(2)
            .collect();
        for (i, (asset, component)) in channels.iter().enumerate() {
            let _ = self.add(
                Operation::Source {
                    asset: *asset,
                    component: *component,
                },
                [40., 230. + i as f32 * 155.],
            );
        }
        if !channels.is_empty() {
            let ids: Vec<_> = self
                .nodes
                .iter()
                .filter(|n| {
                    matches!(
                        n.operation,
                        Operation::Source {
                            component: ClipComponent::AudioChannel { .. },
                            ..
                        }
                    )
                })
                .map(|n| n.id)
                .collect();
            let input = if ids.len() == 2 {
                let mix = self.add(Operation::Mix, [300., 260.]).unwrap();
                self.connect(ids[0], mix, 0).unwrap();
                self.connect(ids[1], mix, 1).unwrap();
                mix
            } else {
                ids[0]
            };
            let out = self.add(Operation::AudioOutput, [560., 260.]).unwrap();
            self.connect(input, out, 0).unwrap();
        }
        self.arrange();
        self.selected = self.nodes.first().map(|n| n.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connections_enforce_media_cycles_and_required_inputs() {
        let mut g = Composition::default();
        let a = g.add(Operation::Flip, [0., 0.]).unwrap();
        let b = g
            .add(Operation::Color { saturation: 1. }, [0., 0.])
            .unwrap();
        let c = g.add(Operation::Gain { db: 0. }, [0., 0.]).unwrap();
        let out = g.add(Operation::VideoOutput, [0., 0.]).unwrap();
        g.connect(a, b, 0).unwrap();
        assert!(g.connect(b, a, 0).is_err());
        assert!(g.connect(c, b, 0).is_err());
        g.connect(b, out, 0).unwrap();
        assert!(g.order().unwrap_err().contains("Horizontal flip"));
        g.remove(a);
        assert_eq!(g.node(b).unwrap().inputs[0], None);
    }
    #[test]
    fn shared_sources_are_ordered_once_and_output_rewiring_is_atomic() {
        let mut g = Composition::default();
        let source = g
            .add(
                Operation::Source {
                    asset: 0,
                    component: ClipComponent::Video(0),
                },
                [0., 0.],
            )
            .unwrap();
        let overlay = g.add(Operation::Overlay { x: 0, y: 0 }, [0., 0.]).unwrap();
        let out = g.add(Operation::VideoOutput, [0., 0.]).unwrap();
        g.connect(source, overlay, 0).unwrap();
        g.connect(source, overlay, 1).unwrap();
        g.connect(overlay, out, 0).unwrap();
        assert_eq!(g.order().unwrap(), vec![source, overlay, out]);
        assert!(g.connect(out, overlay, 0).is_err());
        assert_eq!(g.node(overlay).unwrap().inputs[0], Some(source));
        g.disconnect(out, 0);
        assert!(g.order().is_err());
    }
    #[test]
    fn arrange_keeps_audio_below_video_and_dependencies_to_the_right() {
        let mut graph = Composition::default();
        let audio = graph.add(Operation::Gain { db: 0. }, [0., 0.]).unwrap();
        let video = graph.add(Operation::Flip, [0., 0.]).unwrap();
        let output = graph.add(Operation::VideoOutput, [0., 0.]).unwrap();
        graph.connect(video, output, 0).unwrap();
        let revision = graph.revision;
        graph.arrange();
        assert!(
            graph.node(audio).unwrap().position[1]
                > graph.node(video).unwrap().position[1] + NODE_HEIGHT
        );
        assert!(
            graph.node(output).unwrap().position[0]
                > graph.node(video).unwrap().position[0] + NODE_WIDTH
        );
        assert_eq!(revision, graph.revision);
    }
    #[test]
    fn frames_keep_branch_connections_and_enclose_arranged_nodes() {
        let mut graph = Composition::default();
        let source = graph
            .add(
                Operation::Source {
                    asset: 0,
                    component: ClipComponent::Video(0),
                },
                [40., 50.],
            )
            .unwrap();
        let flip = graph.add(Operation::Flip, [300., 50.]).unwrap();
        let out = graph.add(Operation::VideoOutput, [550., 50.]).unwrap();
        graph.connect(source, flip, 0).unwrap();
        graph.connect(flip, out, 0).unwrap();
        let order = graph.order().unwrap();
        let frame = graph.frame_branch(out).unwrap();
        assert_eq!(graph.order().unwrap(), order);
        graph.arrange();
        let parent = graph.node(frame).unwrap();
        let size = graph.frame_size(parent);
        for node in graph.nodes.iter().filter(|node| node.frame == Some(frame)) {
            assert!(node.position[0] >= parent.position[0]);
            assert!(node.position[1] >= parent.position[1]);
            assert!(node.position[0] + node.width() <= parent.position[0] + size[0]);
            assert!(node.position[1] + node.height() <= parent.position[1] + size[1]);
        }
        graph.remove(frame);
        assert!(graph.nodes.iter().all(|node| node.frame.is_none()));
        assert_eq!(graph.order().unwrap(), order);
    }
    #[test]
    fn new_effect_parameters_are_bounded() {
        for (operation, invalid) in [
            (Operation::Blur { sigma: 3. }, "21"),
            (Operation::Exposure { stops: 0. }, "5"),
            (Operation::Opacity { factor: 1. }, "1.1"),
            (Operation::LowPass { hz: 1000. }, "0"),
            (Operation::HighPass { hz: 80. }, "24000"),
        ] {
            assert!(operation.with_parameters(&[invalid.into()]).is_err());
            assert!(operation.with_parameters(&["NaN".into()]).is_err());
        }
    }
    #[test]
    fn settings_reject_nonfinite_and_out_of_budget_sizes() {
        assert!(
            Operation::Scale { width: 320 }
                .with_parameters(&["100000".into()])
                .is_err()
        );
        assert!(
            Operation::Gain { db: 0. }
                .with_parameters(&["NaN".into()])
                .is_err()
        );
        assert_eq!(
            Operation::Gain { db: 0. }
                .with_parameters(&["-6".into()])
                .unwrap(),
            Operation::Gain { db: -6. }
        );
    }
}
