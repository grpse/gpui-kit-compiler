//! Full compositor evaluation in an isolated Blender worker. Audio stays in libavfilter.
use crate::{
    composition::{Composition, Operation, Signal},
    media::VideoFrame,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub fn executable() -> Option<PathBuf> {
    let mut candidates = vec![];
    if let Some(path) = std::env::var_os("FLOWCUT_BLENDER") {
        candidates.push(PathBuf::from(path));
    }
    candidates.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/blender-runtime/Blender.app/Contents/MacOS/Blender"),
    );
    candidates.push(PathBuf::from(
        "/Applications/Blender.app/Contents/MacOS/Blender",
    ));
    if let Some(paths) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&paths).map(|p| {
            p.join(if cfg!(windows) {
                "blender.exe"
            } else {
                "blender"
            })
        }));
    }
    candidates.into_iter().find(|p| p.is_file())
}
fn worker(mode: &str, request: &Path, directory: &Path) -> Result<(), String> {
    let executable = executable().ok_or("Blender 4.5 LTS is required for this graph. Set FLOWCUT_BLENDER to its executable or run scripts/setup_blender.py.")?;
    let log = directory.join("blender.log");
    let output = fs::File::create(&log).map_err(|e| e.to_string())?;
    let mut process = Command::new(executable)
        .args([
            "--background",
            "--factory-startup",
            "--disable-autoexec",
            "--threads",
            "2",
            "--python-exit-code",
            "1",
            "--python",
        ])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/blender_bridge.py"))
        .args(["--", mode])
        .arg(request)
        .stdout(Stdio::from(output.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(output))
        .spawn()
        .map_err(|e| format!("Cannot start Blender: {e}"))?;
    let started = Instant::now();
    loop {
        if let Some(status) = process.try_wait().map_err(|e| e.to_string())? {
            if status.success() {
                return Ok(());
            }
            let message = fs::read_to_string(&log).unwrap_or_default();
            let reason = message
                .lines()
                .find_map(|line| line.strip_prefix("FLOWCUT_ERROR: "))
                .unwrap_or("The Blender compositor could not evaluate this graph.");
            return Err(reason.to_string());
        }
        if started.elapsed() > Duration::from_secs(120) {
            let _ = process.kill();
            let _ = process.wait();
            return Err(
                "Blender preview exceeded two minutes. Simplify the graph or reduce preview size."
                    .into(),
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn describe(
    operation: &Operation,
    directory: &Path,
) -> Result<crate::blender_catalog::Definition, String> {
    let definition = operation.definition().ok_or("Not a Blender node")?;
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let request = directory.join("describe.json");
    fs::write(&request, json!({"type": definition.id, "settings": operation.settings()?, "output": directory.join("definition.json")}).to_string()).map_err(|e| e.to_string())?;
    worker("describe", &request, directory)?;
    serde_json::from_slice(&fs::read(directory.join("definition.json")).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

pub fn render(
    graph: &Composition,
    sources: &[(u64, VideoFrame)],
    time: f64,
    destination: &Path,
) -> Result<VideoFrame, String> {
    let scratch = Scratch(destination.with_extension("blender-work"));
    fs::create_dir(&scratch.0).map_err(|e| e.to_string())?;
    let (width, height) = sources
        .first()
        .map_or((960, 540), |(_, f)| (f.width, f.height));
    let mut nodes = vec![];
    let mut links = vec![];
    let mut extra = graph.nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
    let mut viewers = vec![];
    for id in graph.order()? {
        let node = graph.node(id).unwrap();
        if node.operation.signal() == Signal::Audio {
            continue;
        }
        let mut mapping: Vec<usize> = (0..node.inputs.len()).collect();
        let (kind, settings): (&str, Value) = match &node.operation {
            Operation::Source { .. } => {
                let frame = &sources
                    .iter()
                    .find(|(source, _)| *source == id)
                    .ok_or("Missing source frame")?
                    .1;
                let path = scratch.0.join(format!("source-{id}.png"));
                let mut rgba = frame.bgra.clone();
                for pixel in rgba.as_chunks_mut::<4>().0 {
                    pixel.swap(0, 2);
                }
                image::save_buffer(
                    &path,
                    &rgba,
                    frame.width,
                    frame.height,
                    image::ColorType::Rgba8,
                )
                .map_err(|e| e.to_string())?;
                ("CompositorNodeImage", json!({"image": path}))
            }
            Operation::Blender { .. } => (
                node.operation.definition().unwrap().id.as_str(),
                node.operation.settings()?,
            ),
            Operation::Blur { sigma } => (
                "CompositorNodeBlur",
                json!({"filter_type": "GAUSS", "input:1": [sigma, sigma]}),
            ),
            Operation::Scale { width: target } => (
                "CompositorNodeScale",
                json!({"space": "ABSOLUTE", "input:1": target, "input:2": (*target as f64 * height as f64 / width as f64).round()}),
            ),
            Operation::Flip => ("CompositorNodeFlip", json!({"axis": "X"})),
            Operation::Color { saturation } => {
                ("CompositorNodeHueSat", json!({"input:2": saturation}))
            }
            Operation::Exposure { stops } => ("CompositorNodeExposure", json!({"input:1": stops})),
            Operation::Opacity { factor } => (
                "CompositorNodeSetAlpha",
                json!({"mode": "APPLY", "input:1": factor}),
            ),
            Operation::Overlay { x, y } => {
                let translated = extra;
                extra += 1;
                nodes.push(json!({"id": translated, "type": "CompositorNodeTranslate", "settings": {"input:1": x, "input:2": -y}}));
                if let Some(source) = node.inputs[1] {
                    links.push(json!({"source": source, "output": node.input_ports[1], "target": translated, "input": 0}));
                }
                links.push(json!({"source": translated, "output": 0, "target": id, "input": 2}));
                mapping = vec![1, usize::MAX];
                ("CompositorNodeAlphaOver", json!({"input:0": 1.0}))
            }
            Operation::VideoOutput => ("CompositorNodeComposite", json!({})),
            _ => return Err("Unsupported video node.".into()),
        };
        if matches!(
            kind,
            "CompositorNodeViewer" | "CompositorNodeComposite" | "CompositorNodeOutputFile"
        ) {
            viewers.push((id, kind.to_string()));
        }
        nodes.push(json!({"id": id, "type": kind, "settings": settings, "muted": node.muted}));
        for (input, source) in node.inputs.iter().enumerate() {
            if let Some(source) = source
                && mapping[input] != usize::MAX
            {
                links.push(json!({"source": source, "output": node.input_ports[input], "target": id, "input": mapping[input]}));
            }
        }
    }
    let request = scratch.0.join("render.json");
    let output = scratch.0.join("result.png");
    fs::write(&request, json!({"width": width, "height": height, "frame": (time * 24.).round() as u64 + 1, "output": output, "nodes": nodes, "links": links, "viewer": viewers.iter().find(|(id,_)|Some(*id)==graph.selected).or_else(||viewers.iter().rev().find(|(_,kind)|kind=="CompositorNodeViewer")).or_else(||viewers.iter().rev().find(|(_,kind)|kind=="CompositorNodeComposite")).or_else(||viewers.last()).ok_or("Connect a Composite, Viewer or File Output node.")?.0}).to_string()).map_err(|e| e.to_string())?;
    worker("render", &request, &scratch.0)?;
    let image = image::open(&output)
        .map_err(|e| format!("Blender produced no preview image: {e}"))?
        .to_rgba8();
    let (width, height) = image.dimensions();
    let mut bgra = image.into_raw();
    for pixel in bgra.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let files = scratch.0.join("files");
    if files.exists() {
        fs::create_dir(destination).map_err(|e| e.to_string())?;
        fs::rename(files, destination.join("files")).map_err(|e| e.to_string())?;
    }
    Ok(VideoFrame {
        width,
        height,
        bgra,
        timestamp: time,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn operation(name: &str) -> Operation {
        Operation::blender(
            crate::blender_catalog::catalog()
                .nodes
                .iter()
                .position(|n| n.id == name)
                .unwrap(),
        )
    }
    fn set(op: &Operation, key: &str, value: &str) -> Operation {
        let mut values = op
            .parameters()
            .into_iter()
            .map(|(_, v)| v)
            .collect::<Vec<_>>();
        let i = op
            .definition()
            .unwrap()
            .parameters
            .iter()
            .position(|p| p.key == key)
            .unwrap();
        values[i] = value.into();
        op.with_parameters(&values).unwrap()
    }
    #[test]
    fn blender_renders_selected_output_defaults_mute_and_file_output() {
        if executable().is_none() {
            eprintln!(
                "Blender runtime unavailable; run scripts/setup_blender.py to exercise integration"
            );
            return;
        }
        let workspace = crate::Workspace::new().unwrap();
        let mut graph = Composition::default();
        let color = graph
            .add(
                set(&operation("CompositorNodeRGB"), "output:0", "[1,0,0,1]"),
                [0., 0.],
            )
            .unwrap();
        let separate = graph
            .add(operation("CompositorNodeSeparateColor"), [250., 0.])
            .unwrap();
        let combine = graph
            .add(operation("CompositorNodeCombineColor"), [500., 0.])
            .unwrap();
        let invert = graph
            .add(operation("CompositorNodeInvert"), [750., 0.])
            .unwrap();
        let output = graph
            .add(operation("CompositorNodeComposite"), [1000., 0.])
            .unwrap();
        graph.connect(color, separate, 0).unwrap();
        graph.connect_port(separate, 0, combine, 1).unwrap();
        graph.connect(combine, invert, 1).unwrap();
        graph.connect(invert, output, 0).unwrap();
        let frame = render(&graph, &[], 0., &workspace.path.join("green-invert")).unwrap();
        let center = ((frame.height / 2 * frame.width + frame.width / 2) * 4) as usize;
        assert!(
            frame.bgra[center] > 240 && frame.bgra[center + 1] < 10 && frame.bgra[center + 2] > 240,
            "{:?}",
            &frame.bgra[center..center + 4]
        );
        graph
            .nodes
            .iter_mut()
            .find(|n| n.id == invert)
            .unwrap()
            .muted = true;
        let source = workspace.path.join("source.png");
        image::save_buffer(
            &source,
            &[0, 255, 0, 255].repeat(64 * 64),
            64,
            64,
            image::ColorType::Rgba8,
        )
        .unwrap();
        let image = graph
            .add(
                set(
                    &operation("CompositorNodeImage"),
                    "image",
                    source.to_str().unwrap(),
                ),
                [700., 300.],
            )
            .unwrap();
        let file = graph
            .add(operation("CompositorNodeOutputFile"), [1000., 300.])
            .unwrap();
        graph.connect(image, file, 0).unwrap();
        let directory = workspace.path.join("green-muted");
        let frame = render(&graph, &[], 0., &directory).unwrap();
        assert!(
            frame.bgra[center] < 10 && frame.bgra[center + 1] > 240 && frame.bgra[center + 2] < 10,
            "{:?}",
            &frame.bgra[center..center + 4]
        );
        assert!(directory.join("files").is_dir());
        assert!(
            std::fs::read_dir(directory.join("files"))
                .unwrap()
                .next()
                .is_some()
        );
    }
    #[test]
    fn shared_procedural_nodes_and_imported_groups_render_in_blender() {
        if executable().is_none() {
            return;
        }
        let workspace = crate::Workspace::new().unwrap();
        let mut graph = Composition::default();
        let coordinates = graph
            .add(operation("CompositorNodeImageCoordinates"), [0., 0.])
            .unwrap();
        let noise = graph
            .add(operation("ShaderNodeTexNoise"), [250., 0.])
            .unwrap();
        let output = graph
            .add(operation("CompositorNodeComposite"), [500., 0.])
            .unwrap();
        graph.connect(coordinates, noise, 0).unwrap();
        graph.connect_port(noise, 1, output, 0).unwrap();
        let frame = render(&graph, &[], 0., &workspace.path.join("noise")).unwrap();
        let pixels = frame.bgra.as_chunks::<4>().0;
        assert!(
            pixels.iter().any(|p| p != &pixels[0]),
            "Procedural noise should vary across coordinates"
        );
        let fixture = workspace.path.join("group.blend");
        let request = workspace.path.join("fixture.json");
        fs::write(&request, json!({"output":fixture}).to_string()).unwrap();
        worker("fixture", &request, &workspace.path).unwrap();
        let group = set(
            &operation("CompositorNodeGroup"),
            "group_file",
            fixture.to_str().unwrap(),
        );
        let layout = describe(&group, &workspace.path.join("group-layout")).unwrap();
        assert_eq!(layout.inputs.len(), 1);
        assert_eq!(layout.outputs.len(), 1);
        let group = group.with_layout(layout);
        let mut graph = Composition::default();
        let value = graph
            .add(
                set(&operation("ShaderNodeValue"), "output:0", "1"),
                [0., 0.],
            )
            .unwrap();
        let group = graph.add(group, [250., 0.]).unwrap();
        let output = graph
            .add(operation("CompositorNodeComposite"), [500., 0.])
            .unwrap();
        graph.connect(value, group, 0).unwrap();
        graph.connect(group, output, 0).unwrap();
        let frame = render(&graph, &[], 0., &workspace.path.join("group-render")).unwrap();
        assert!(
            (180..=195).contains(&frame.bgra[0]),
            "A -1 stop group must halve scene-linear white: {:?}",
            &frame.bgra[..4]
        );
    }
    #[test]
    fn glare_modes_accept_inspector_values_and_refresh_integer_sockets() {
        if executable().is_none() {
            return;
        }
        let workspace = crate::Workspace::new().unwrap();
        let glare = operation("CompositorNodeGlare");
        let iterations = glare
            .definition()
            .unwrap()
            .parameters
            .iter()
            .find(|p| p.label == "Iterations")
            .unwrap();
        assert_eq!(iterations.kind, "INT");
        assert!(iterations.parse("2.5").is_err());
        let fog = set(&glare, "glare_type", "FOG_GLOW");
        let definition = describe(&fog, &workspace.path.join("fog-layout")).unwrap();
        let inputs: Vec<_> = definition
            .inputs
            .iter()
            .filter(|socket| socket.enabled)
            .map(|socket| socket.name.as_str())
            .collect();
        assert!(inputs.contains(&"Size"));
        assert!(!inputs.contains(&"Streaks"));
    }
    #[test]
    fn changed_math_mode_refreshes_visible_sockets_and_preserves_links() {
        if executable().is_none() {
            return;
        }
        let workspace = crate::Workspace::new().unwrap();
        let mut graph = Composition::default();
        let value = graph
            .add(operation("CompositorNodeValue"), [0., 0.])
            .unwrap();
        let math = graph
            .add(
                set(
                    &operation("CompositorNodeMath"),
                    "operation",
                    "MULTIPLY_ADD",
                ),
                [250., 0.],
            )
            .unwrap();
        graph.connect(value, math, 0).unwrap();
        let definition = describe(
            &graph.node(math).unwrap().operation,
            &workspace.path.join("describe"),
        )
        .unwrap();
        graph.refresh_layout(math, definition);
        assert_eq!(
            graph.node(math).unwrap().operation.visible_inputs().len(),
            3
        );
        assert_eq!(
            graph.node(math).unwrap().inputs,
            vec![Some(value), None, None]
        );
    }
}
