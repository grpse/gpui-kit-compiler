//! Reproducible, opt-in hardware proof. Produces bounded offline audio plus device
//! callback / shared-clock evidence; never records the microphone or loopback.
use crate::{
    Workspace, catalog,
    media::{PreviewEvent, PreviewRequest, PreviewWorker},
    playback::{Mixer, OutputEvent, OutputWorker, PlaybackPlan},
    state::{Action, EditorState},
};
use std::{
    fs::{self, File},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
fn checksum(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = [0; 65536];
    let mut hash = 14695981039346656037u64;
    loop {
        let count = file.read(&mut bytes).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        for byte in &bytes[..count] {
            hash = (hash ^ *byte as u64).wrapping_mul(1099511628211);
        }
    }
    Ok(format!("{hash:016x}"))
}
fn wav(plan: &PlaybackPlan, path: &Path) -> Result<(), String> {
    let frames = (plan.end * 48000.).ceil() as u32;
    let mut writer = BufWriter::new(File::create(path).map_err(|e| e.to_string())?);
    let mut header = Vec::new();
    header.extend(b"RIFF");
    header.extend((36 + frames * 4).to_le_bytes());
    header.extend(b"WAVEfmt ");
    header.extend(16u32.to_le_bytes());
    header.extend(1u16.to_le_bytes());
    header.extend(2u16.to_le_bytes());
    header.extend(48000u32.to_le_bytes());
    header.extend(192000u32.to_le_bytes());
    header.extend(4u16.to_le_bytes());
    header.extend(16u16.to_le_bytes());
    header.extend(b"data");
    header.extend((frames * 4).to_le_bytes());
    writer.write_all(&header).map_err(|e| e.to_string())?;
    let mut mixer = Mixer::new(plan, 0., 48000);
    let mut block = [[0.; 2]; 512];
    let mut remaining = frames as usize;
    while remaining > 0 {
        let count = remaining.min(block.len());
        mixer.render(&mut block[..count])?;
        for frame in &block[..count] {
            for sample in frame {
                let sample = (sample.clamp(-1., 1.) * 32767.).round() as i16;
                writer
                    .write_all(&sample.to_le_bytes())
                    .map_err(|e| e.to_string())?;
            }
        }
        remaining -= count;
    }
    writer.flush().map_err(|e| e.to_string())
}
pub fn run(source: &Path, directory: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let original = checksum(source)?;
    let workspace = Workspace::new()?;
    let report = catalog::import(
        vec![source.to_owned()],
        Default::default(),
        Some(&workspace.path),
        Arc::new(AtomicBool::new(false)),
    );
    if !report.errors.is_empty() {
        return Err(report.errors.join("\n"));
    }
    let mut state = EditorState::default();
    state.clips.clear();
    state.tracks.clear();
    state.project_min_duration = 0.;
    state.import_assets(report.assets);
    state.apply(Action::AddToTimeline);
    for track in &mut state.tracks {
        track.gain = 0.12;
    }
    let video = state.clips[state.selected_clip].clone();
    let split_seconds = video.start as f64 + 2.;
    if video.length > 3. {
        state.position = split_seconds as f32;
        state.apply(Action::Tool("Split"));
        let right_group = state.clips[state.selected_clip].link_group;
        for clip in &mut state.clips {
            if clip.link_group == right_group {
                clip.start += 1.;
            }
        }
    }
    let plan = PlaybackPlan::from_state(&state);
    if plan.audio.is_empty() || plan.videos.is_empty() || plan.end <= 0. || plan.end > 15. {
        return Err("Proof needs an audio/video source up to 15 seconds long".into());
    }
    let media = state.assets[state.selected]
        .prepared
        .as_ref()
        .ok_or("Source was not preprocessed")?;
    let preview = directory.join("monitor-mix.wav");
    wav(&plan, &preview)?;
    let (output, errors) = OutputWorker::start();
    let (video, frames) = PreviewWorker::start();
    output.play(plan.clone(), 0., 1);
    let deadline = Instant::now() + Duration::from_secs_f64(plan.end + 10.);
    let mut active = None;
    let mut video_generation = 0;
    let mut video_offset = 0.;
    let mut presented = 0;
    let mut differences = Vec::new();
    let mut last = 0.;
    while !output.clock.ended() {
        if Instant::now() > deadline {
            return Err("Audio device proof timed out".into());
        }
        if let Ok(OutputEvent::Error(_, error)) = errors.try_recv() {
            return Err(error);
        }
        let position = output.clock.position().max(0.);
        let current = plan.video_at(position);
        if current.map(|(index, _)| index) != active {
            active = current.map(|(index, _)| index);
            video_generation += 1;
            if let Some((_, region)) = current {
                video_offset = region.start - region.source_start;
                video.request(PreviewRequest {
                    path: region.path.clone(),
                    position: (position - video_offset).max(region.source_start),
                    end: region.source_start + region.duration,
                    playing: true,
                    generation: video_generation,
                    stream: Some(region.stream),
                    keyframes: Some(region.keyframes.clone()),
                    clock: Some(output.clock.clone()),
                    clock_offset: video_offset,
                });
            } else {
                video.pause();
            }
        }
        while let Ok(event) = frames.try_recv() {
            match event {
                PreviewEvent::Frame { generation, frame } if generation == video_generation => {
                    presented += 1;
                    differences.push(
                        (output.clock.position() - (frame.timestamp + video_offset)).abs() * 1000.,
                    );
                }
                PreviewEvent::Error(_, error) => return Err(error),
                _ => {}
            }
        }
        last = position;
        thread::sleep(Duration::from_millis(2));
    }
    let end = output.clock.position();
    output.pause();
    video.pause();
    differences.sort_by(f64::total_cmp);
    let checksum_after = checksum(source)?;
    let result = serde_json::json!({"source":source,"source_checksum_algorithm":"FNV-1a 64 (integrity check, not cryptographic)","source_checksum_before":original,"source_checksum_after":checksum_after,"source_unchanged":original==checksum_after,
        "video":media.videos.iter().map(|stream|serde_json::json!({"stream":stream.index,"pixel_format":stream.pixel_format,"keyframes":stream.keyframes.entries})).collect::<Vec<_>>(),
        "audio":media.audio.iter().map(|stream|serde_json::json!({"stream":stream.index,"sample_rate":stream.sample_rate,"encoding":stream.encoding.label(),"channels":stream.channels.len(),"samples":stream.samples})).collect::<Vec<_>>(),
        "timeline_tracks":state.tracks.len(),"video_regions":plan.videos.len(),"linked_split_seconds":split_seconds,"inserted_gap_seconds":if plan.videos.len()>1{1.}else{0.},"audio_queue_frames":crate::playback::AUDIO_QUEUE_FRAMES,"output_rate":output.clock.rate.load(Ordering::Relaxed),"device_frames_consumed":output.clock.consumed.load(Ordering::Relaxed),"nonzero_device_frames":output.clock.nonzero.load(Ordering::Relaxed),"output_clipped_frames":output.clock.clipped.load(Ordering::Relaxed),"device_underrun_callbacks":output.clock.underruns.load(Ordering::Relaxed),"video_frames_presented":presented,"video_clock_max_difference_ms":differences.last().copied().unwrap_or(0.),"video_clock_p95_difference_ms":differences.get(differences.len().saturating_sub(1)*95/100).copied().unwrap_or(0.),"planned_end_seconds":plan.end,"device_end_seconds":end,"last_observed_seconds":last,"monitor_wav":preview});
    if original != checksum_after
        || presented == 0
        || output.clock.nonzero.load(Ordering::Relaxed) == 0
    {
        return Err(
            "Proof did not demonstrate unchanged source and actual audio/video delivery".into(),
        );
    }
    let path = directory.join("playback.json");
    fs::write(
        &path,
        serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(path)
}

/// Reproducible native graph render, independent of GUI and device availability.
pub fn run_composition(directory: &Path) -> Result<PathBuf, String> {
    use crate::composition::{Composition, Operation};
    if directory.exists() {
        return Err("Choose a new composition proof directory.".into());
    }
    let workspace = Workspace::new()?;
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/media/murchison-falls.webm");
    let source_before = checksum(&source)?;
    let report = catalog::import(
        vec![source.clone()],
        Default::default(),
        Some(&workspace.path),
        Arc::new(AtomicBool::new(false)),
    );
    if let Some(error) = report.errors.first() {
        return Err(error.clone());
    }
    let cache_paths: Vec<_> = report
        .assets
        .iter()
        .flat_map(|a| {
            a.prepared.iter().flat_map(|m| {
                m.audio
                    .iter()
                    .flat_map(|s| s.channels.iter().map(|c| c.path.clone()))
            })
        })
        .collect();
    let cache_before = cache_paths
        .iter()
        .map(|p| checksum(p))
        .collect::<Result<Vec<_>, _>>()?;
    let mut graph = Composition::default();
    graph.seed(&report.assets);
    let source_node = graph
        .nodes
        .iter()
        .find(|n| {
            matches!(
                n.operation,
                Operation::Source {
                    component: crate::state::ClipComponent::Video(_),
                    ..
                }
            )
        })
        .ok_or("Missing video source")?
        .id;
    let color = graph
        .nodes
        .iter()
        .find(|n| matches!(n.operation, Operation::Blur { .. }))
        .ok_or("Missing blur node")?
        .id;
    let flip = graph.add(Operation::Flip, [285., 40.])?;
    graph.connect(source_node, flip, 0)?;
    graph.connect(flip, color, 0)?;
    graph.update(color, &["1".into()])?;
    let started = Instant::now();
    let result = crate::compositing::render(&graph, &report.assets, 1., 2., directory)?;
    let elapsed = started.elapsed().as_secs_f64();
    let source_after = checksum(&source)?;
    let cache_after = cache_paths
        .iter()
        .map(|p| checksum(p))
        .collect::<Result<Vec<_>, _>>()?;
    if source_before != source_after || cache_before != cache_after {
        return Err("Compositing altered source media or channel caches.".into());
    }
    let image = result.video.as_ref().ok_or("No rendered video frame")?;
    let audio = result
        .audio_media
        .as_ref()
        .and_then(|m| m.audio.first())
        .ok_or("No rendered audio stream")?;
    let proof = serde_json::json!({"recorded_at_unix_seconds":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_secs(),"backend":"native libavfilter through ffmpeg-next 9, no subprocess","source":source,"checksum_algorithm":"FNV-1a 64 (integrity check, not cryptographic)","source_checksum_before":source_before,"source_checksum_after":source_after,"source_unchanged":true,"channel_cache_checksums_before":cache_before,"channel_cache_checksums_after":cache_after,"channel_caches_unchanged":true,"source_time_seconds":1.,"audio_duration_seconds":2.,"render_seconds":elapsed,"video":{"width":image.width,"height":image.height,"pixel_format":"BGRA 8-bit preview","file":result.image},"audio":{"sample_rate":audio.sample_rate,"channels":audio.channels.len(),"samples_per_channel":audio.samples,"encoding":audio.encoding.label(),"peak":result.peak,"file":result.audio},"nodes":graph.nodes.iter().map(|n|serde_json::json!({"id":n.id,"operation":n.operation.label(),"description":n.operation.summary(),"inputs":n.inputs})).collect::<Vec<_>>()});
    let path = directory.join("composition.json");
    fs::write(
        &path,
        serde_json::to_vec_pretty(&proof).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(path)
}
