use rsx_overtone::{engine::*, model::*, persistence};
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|a| a == "--benchmark") {
        let config = EngineConfig {
            voices: 64,
            ..Default::default()
        };
        let api = SynthApi::new(config, Tuning::default())?;
        let mut sound = Sound::new(0, "Benchmark");
        sound.resize_harmonics(32);
        for h in &mut sound.harmonics {
            h.amplitude = 1. / h.multiple as f32;
        }
        let mut engine = SynthEngine::new(config)?;
        for id in 0..64 {
            engine.command(Command::Note(api.note(
                &sound,
                NoteRequest {
                    voice_id: id,
                    owner: 0,
                    midi: 48 + (id % 12) as u8,
                    velocity: 0.8,
                    gate_seconds: 10.,
                    gain: 1.,
                    pan: 0.,
                },
            )?));
        }
        let mut block = [0.; 256];
        engine.render(&mut block);
        let mut timings = Vec::with_capacity(375);
        let start = std::time::Instant::now();
        let mut checksum = 0.;
        for _ in 0..375 {
            let begin = std::time::Instant::now();
            engine.render(&mut block);
            timings.push(begin.elapsed().as_secs_f64());
            checksum += block.iter().sum::<f32>();
        }
        let elapsed = start.elapsed().as_secs_f64();
        timings.sort_by(f64::total_cmp);
        println!(
            "64 voices × 32 harmonics, 48 kHz, 128-frame stereo blocks: 1.000s audio in {elapsed:.4}s ({:.2}× realtime); p99 {:.3}ms / 2.667ms deadline; dropped {}; checksum {checksum:.4}",
            1. / elapsed,
            timings[371] * 1000.,
            engine.dropped_notes
        );
        return Ok(());
    }
    let (project, path) = if args.len() == 2 && args[0] == "--demo" {
        let mut p = Project::default();
        p.draft.noise.level = 0.;
        p.add_clip(p.draft.clone(), None);
        for midi in [60, 64, 67] {
            p.add_note(midi);
        }
        (p, std::path::PathBuf::from(&args[1]))
    } else if args.len() == 2 {
        (
            persistence::load(std::path::Path::new(&args[0]))?.project,
            std::path::PathBuf::from(&args[1]),
        )
    } else {
        return Err("Usage: overtone-render <project.overtone> <output.wav> | --demo <output.wav> | --benchmark".into());
    };
    let api = SynthApi::new(
        EngineConfig {
            voices: 64,
            ..Default::default()
        },
        project.tuning.clone(),
    )?;
    api.export_wav(&project, &path)?;
    println!("Rendered {}", path.display());
    Ok(())
}
