// Offline recording analysis. No decoder, FFT or project mutation runs in a device callback.
use crate::model::*;
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::{fs::File, path::Path, sync::Arc};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, errors::Error, formats::FormatOptions,
    io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

pub const MAX_SECONDS: u32 = 120;
pub struct Recording {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}
pub struct AnalysisResult {
    pub sounds: Vec<Sound>,
    pub duration_seconds: f64,
}

/// Decode the first audio track and downmix to mono; reject changing rates and oversized takes.
pub fn decode(path: &Path) -> Result<Recording, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|v| v.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("Cannot decode this recording: {e}"))?
        .format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or("Recording contains no audio track")?;
    let id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Unsupported recording codec: {e}"))?;
    let mut samples = Vec::new();
    let mut sample_rate = 0;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(format!("Recording read failed: {e}")),
        };
        if packet.track_id() != id {
            continue;
        }
        let decoded = decoder
            .decode(&packet)
            .map_err(|e| format!("Recording decode failed: {e}"))?;
        let spec = *decoded.spec();
        let channels = spec.channels.count();
        if !(8000..=192000).contains(&spec.rate) || channels == 0 || channels > 32 {
            return Err("Recording must have 8–192 kHz audio and 1–32 channels".into());
        }
        if sample_rate != 0 && sample_rate != spec.rate {
            return Err("Recording sample rate changed".into());
        }
        sample_rate = spec.rate;
        if samples.len() + decoded.frames() > sample_rate as usize * MAX_SECONDS as usize {
            return Err(format!(
                "Analyze recordings of at most {MAX_SECONDS} seconds"
            ));
        }
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);
        for frame in buffer.samples().chunks_exact(channels) {
            if frame.iter().any(|v| !v.is_finite()) {
                return Err("Recording contains invalid samples".into());
            }
            samples.push((frame.iter().sum::<f32>() / channels as f32).clamp(-1., 1.));
        }
    }
    if sample_rate == 0 || samples.is_empty() {
        return Err("Recording is empty".into());
    }
    Ok(Recording {
        samples,
        sample_rate,
    })
}
pub fn analyze_file(path: &Path, settings: &AnalysisSettings) -> Result<AnalysisResult, String> {
    analyze(&decode(path)?, settings)
}

struct Pitch {
    forward: Arc<dyn Fft<f32>>,
    inverse: Arc<dyn Fft<f32>>,
    buffer: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    energy: Vec<f64>,
    difference: Vec<f64>,
}
impl Pitch {
    fn new() -> Self {
        let mut p = FftPlanner::new();
        let forward = p.plan_fft_forward(4096);
        let inverse = p.plan_fft_inverse(4096);
        let scratch_len = forward
            .get_inplace_scratch_len()
            .max(inverse.get_inplace_scratch_len());
        Self {
            forward,
            inverse,
            buffer: vec![Complex::default(); 4096],
            scratch: vec![Complex::default(); scratch_len],
            energy: vec![0.; 2049],
            difference: vec![0.; 2049],
        }
    }
    // FFT autocorrelation gives the YIN squared difference in O(N log N).
    fn estimate(&mut self, samples: &[f32], rate: f64) -> (Option<f64>, f32) {
        let n = samples.len().min(2048);
        if n < 256 {
            return (None, 0.);
        }
        let samples = &samples[..n];
        let mean = samples.iter().map(|v| *v as f64).sum::<f64>() / n as f64;
        self.buffer.fill(Complex::default());
        self.energy[0] = 0.;
        for (i, v) in samples.iter().enumerate() {
            let x = *v as f64 - mean;
            self.buffer[i].re = x as f32;
            self.energy[i + 1] = self.energy[i] + x * x;
        }
        if self.energy[n] < 1e-7 {
            return (None, 0.);
        }
        self.forward
            .process_with_scratch(&mut self.buffer, &mut self.scratch);
        for x in &mut self.buffer {
            *x = Complex::new(x.norm_sqr(), 0.);
        }
        self.inverse
            .process_with_scratch(&mut self.buffer, &mut self.scratch);
        let low = (rate / 1600.).ceil().max(2.) as usize;
        let high = ((rate / 40.) as usize).min(n / 2);
        let mut cumulative = 0.;
        for lag in 1..=high {
            let d = (self.energy[n - lag] + self.energy[n]
                - self.energy[lag]
                - 2. * self.buffer[lag].re as f64 / 4096.)
                .max(0.);
            cumulative += d;
            self.difference[lag] = d * lag as f64 / cumulative.max(1e-12);
        }
        let Some(mut lag) = (low..high).find(|&lag| self.difference[lag] < 0.18) else {
            return (None, 0.);
        };
        while lag + 1 < high && self.difference[lag + 1] < self.difference[lag] {
            lag += 1;
        }
        let (a, b, c) = (
            self.difference[lag - 1],
            self.difference[lag],
            self.difference[lag + 1],
        );
        let offset = if (a - 2. * b + c).abs() > 1e-9 {
            (0.5 * (a - c) / (a - 2. * b + c)).clamp(-0.5, 0.5)
        } else {
            0.
        };
        (
            Some(rate / (lag as f64 + offset)),
            (1. - b).clamp(0., 1.) as f32,
        )
    }
}
// Favor a simple shared harmonic basis when a take contains more than one stable pitch.
// Candidate work is bounded by eight pitch groups and eight divisors, not recording length.
fn shared_fundamental(values: &mut [f64], maximum: usize) -> Option<f64> {
    if values.is_empty() { return None; }
    values.sort_by(f64::total_cmp);
    let mut groups: Vec<(f64, usize)> = Vec::new();
    for &hz in values.iter() {
        if let Some((mean, count)) = groups.last_mut().filter(|(mean, _)| (1200.*(hz / *mean).log2()).abs()<35.) {
            *mean = (*mean * *count as f64 + hz) / (*count+1) as f64; *count+=1;
        } else { groups.push((hz, 1)); }
    }
    groups.sort_by(|a,b|b.1.cmp(&a.1));
    let minimum=(groups[0].1 as f64*0.1).ceil() as usize;
    groups.retain(|(_,count)|*count>=minimum);groups.truncate(8);
    let mut best=None;let mut best_score=f64::INFINITY;
    for &(hz, _) in &groups { for divisor in 1..=8 {
        let base=hz/divisor as f64;if base<40. {continue;}
        let score=groups.iter().map(|(pitch,count)|{
            let multiple=(pitch/base).round().clamp(1.,maximum as f64);
            let cents=1200.*(pitch/(base*multiple)).log2();
            (cents*cents+0.05*multiple*multiple)* *count as f64
        }).sum::<f64>();
        if score<best_score {best_score=score;best=Some(base);}
    } }
    best
}
fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|v| v * v).sum::<f32>() / samples.len().max(1) as f32).sqrt()
}

/// One recording produces one aggregate harmonic patch; pauses never split library entries.
pub fn analyze(
    recording: &Recording,
    settings: &AnalysisSettings,
) -> Result<AnalysisResult, String> {
    let rate = recording.sample_rate;
    if !(8000..=192000).contains(&rate)
        || recording.samples.is_empty()
        || recording.samples.len() > rate as usize * MAX_SECONDS as usize
        || recording
            .samples
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1.)
    {
        return Err("Invalid or oversized recording".into());
    }
    if !(1..=MAX_HARMONICS).contains(&settings.max_harmonics)
        || !settings.silence_db.is_finite()
        || !(-80.0..=-10.0).contains(&settings.silence_db)
        || !(50..=1000).contains(&settings.split_gap_ms)
        || !(50..=2000).contains(&settings.min_sound_ms)
        || !settings.trim_start_seconds.is_finite()
        || settings.trim_start_seconds < 0.
        || settings
            .trim_end_seconds
            .is_some_and(|v| !v.is_finite() || v <= settings.trim_start_seconds)
        || settings
            .fundamental_hz
            .is_some_and(|v| !v.is_finite() || !(40.0..=2000.0).contains(&v))
    {
        return Err("Invalid analysis settings (pitch override: 40–2000 Hz)".into());
    }
    let full_duration = recording.samples.len() as f64 / rate as f64;
    let start = (settings.trim_start_seconds * rate as f64).round() as usize;
    let end = (settings
        .trim_end_seconds
        .unwrap_or(full_duration)
        .min(full_duration)
        * rate as f64)
        .round() as usize;
    if start >= end || end > recording.samples.len() {
        return Err("Recording trim is empty or outside the take".into());
    }
    let signal = &recording.samples[start..end];
    let hop = (rate / 100) as usize;
    let levels = signal.chunks(hop).map(rms).collect::<Vec<_>>();
    let peak = levels.iter().copied().fold(0., f32::max);
    let mut sorted = levels.clone();
    sorted.sort_by(f32::total_cmp);
    // Limit adaptive noise-floor influence so a continuous tone remains detectable.
    let floor = sorted[sorted.len() / 10].min(peak * 0.08);
    let threshold = 10_f32.powf(settings.silence_db / 20.).max(floor * 3.);
    if peak < threshold {
        return Ok(AnalysisResult {
            sounds: vec![],
            duration_seconds: full_duration,
        });
    }
    let decimation = (rate / 8000).max(1) as usize;
    let down = signal
        .chunks(decimation)
        .map(|s| s.iter().sum::<f32>() / s.len() as f32)
        .collect::<Vec<_>>();
    let pitch_rate = rate as f64 / decimation as f64;
    let mut estimator = Pitch::new();
    let mut pitches = vec![(None, 0.); levels.len()];
    for i in (0..levels.len()).step_by(5) {
        let center = i * hop / decimation;
        let width = (pitch_rate * 0.096) as usize;
        let from = center
            .saturating_sub(width / 2)
            .min(down.len().saturating_sub(width));
        let to = (from + width).min(down.len());
        let pitch = if levels[i] >= threshold {
            estimator.estimate(&down[from..to], pitch_rate)
        } else {
            (None, 0.)
        };
        for item in pitches.iter_mut().skip(i).take(5) {
            *item = pitch;
        }
    }
    let mut estimates = pitches.iter().zip(&levels)
        .filter_map(|((hz, confidence), level)| if *confidence>0.82 && *level>=threshold { *hz } else { None })
        .collect::<Vec<_>>();
    let hz = settings.fundamental_hz.or_else(|| shared_fundamental(&mut estimates, settings.max_harmonics));
    let confidence = if settings.fundamental_hz.is_some() {1.} else {
        let active = pitches.iter().zip(&levels).filter(|(_, level)| **level>=threshold).collect::<Vec<_>>();
        active.iter().map(|((_, confidence), _)| *confidence).sum::<f32>() / active.len().max(1) as f32
    };
    // Analyze the entire selected take. Average only representative audible windows;
    // internal pauses remain part of the stored duration and envelope, not new patches.
    let reduction = rate.div_ceil(48000) as usize;
    let reduced = signal.chunks(reduction)
        .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32).collect::<Vec<_>>();
    let mut loud_prefix=vec![0usize;levels.len()+1];
    for (i,level) in levels.iter().enumerate(){loud_prefix[i+1]=loud_prefix[i]+usize::from(*level>=threshold);}
    let width=((rate as f64/reduction as f64*0.12) as usize).clamp(512,8192).min(reduced.len());
    let all_centers=levels.iter().enumerate().filter(|(_, level)| **level>=threshold)
        .map(|(i, _)| (i*hop+hop/2)/reduction).collect::<Vec<_>>();
    let steady_centers=all_centers.iter().copied().filter(|center|{
        let from=center.saturating_sub(width/2).min(reduced.len()-width);
        let first=from*reduction/hop;let last=((from+width)*reduction).div_ceil(hop).min(levels.len());
        (loud_prefix[last]-loud_prefix[first])*5>=(last-first)*4
    }).collect::<Vec<_>>();
    // Avoid treating the edges of pauses as broadband noise in an otherwise clean tone.
    let centers=if steady_centers.is_empty(){all_centers}else{steady_centers};
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(16384);
    let mut sound = fit(&reduced, rate as f64/reduction as f64, hz, settings.max_harmonics, &centers, &fft);
    sound.capture = Some(SoundCapture {
        start_seconds: start as f64 / rate as f64,
        duration_seconds: (end-start) as f64 / rate as f64,
        fundamental_hz: hz, confidence,
    });
    Ok(AnalysisResult { sounds: vec![sound], duration_seconds: full_duration })
}

/// Split at sustained silence. Bounds refer to the original take; no samples are rewritten.
pub fn split_file(path:&Path, settings:&AnalysisSettings)->Result<AnalysisResult,String> {
    split_recording(&decode(path)?,settings)
}
pub fn split_recording(recording:&Recording,settings:&AnalysisSettings)->Result<AnalysisResult,String> {
    if !settings.valid() || !(8000..=192000).contains(&recording.sample_rate)
        || recording.samples.is_empty() || recording.samples.len()>recording.sample_rate as usize*MAX_SECONDS as usize
        || recording.samples.iter().any(|v|!v.is_finite() || v.abs()>1.) {
        return Err("Invalid recording or split settings".into());
    }
    let rate=recording.sample_rate as usize;
    let duration=recording.samples.len() as f64/rate as f64;
    let start=(settings.trim_start_seconds*rate as f64).round() as usize;
    let end=(settings.trim_end_seconds.unwrap_or(duration).min(duration)*rate as f64).round() as usize;
    if start>=end || end>recording.samples.len(){return Err("Recording trim is empty or outside the take".into());}
    let hop=(rate/100).max(1);
    let threshold=10_f32.powf(settings.silence_db/20.);
    let gap=(settings.split_gap_ms as usize*rate).div_ceil(1000);
    let minimum=(settings.min_sound_ms as usize*rate).div_ceil(1000);
    let mut regions=Vec::new();let mut from=None;let mut last_end=start;
    for (i,chunk) in recording.samples[start..end].chunks(hop).enumerate() {
        let at=start+i*hop;
        if rms(chunk)>=threshold {
            if from.is_none(){from=Some(at);}
            last_end=at+chunk.len();
        } else if from.is_some() && at+chunk.len()-last_end>=gap {
            let begin=from.take().unwrap();if last_end-begin>=minimum{regions.push((begin,last_end));}
        }
    }
    if let Some(begin)=from {if last_end-begin>=minimum{regions.push((begin,last_end));}}
    let sounds=regions.into_iter().enumerate().map(|(i,(a,b))|{
        let mut sound=Sound::new(0,&format!("Sound {}",i+1));
        sound.recorded_sample=true;sound.gain=1.;sound.noise.level=0.;
        for h in &mut sound.harmonics{h.amplitude=0.;}
        sound.capture=Some(SoundCapture{start_seconds:a as f64/rate as f64,duration_seconds:(b-a) as f64/rate as f64,fundamental_hz:None,confidence:0.});
        sound
    }).collect();
    Ok(AnalysisResult{sounds,duration_seconds:duration})
}

fn fit(
    signal: &[f32],
    rate: f64,
    hz: Option<f64>,
    maximum: usize,
    centers: &[usize],
    fft: &Arc<dyn Fft<f32>>,
) -> Sound {
    let mut sound = Sound::new(0, "Analyzed sound");
    sound.resize_harmonics(maximum);
    for h in &mut sound.harmonics {
        h.amplitude = 0.;
    }
    let width = ((rate * 0.12) as usize).clamp(512, 8192).min(signal.len());
    let window = (0..width)
        .map(|i| 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / (width - 1).max(1) as f64).cos())
        .collect::<Vec<_>>();
    let mut spectrum = vec![Complex::<f32>::default(); 16384];
    let mut scratch = vec![Complex::default(); fft.get_inplace_scratch_len()];
    let mut starts=centers.iter().map(|center|center.saturating_sub(width/2).min(signal.len()-width)).collect::<Vec<_>>();
    starts.dedup();if starts.is_empty(){starts.push(0);}
    let frames=starts.len().min(12);
    let selected=(0..frames).map(|i|starts[if frames==1{0}else{(starts.len()-1)*i/(frames-1)}]).collect::<Vec<_>>();
    let mut amplitudes = vec![0.; maximum];
    let mut best_level = 0.;
    let mut noise_energy = 0.;
    let mut total_energy = 0.;
    let mut low_energy = 0.;
    let mut high_energy = 0.;
    for start in selected {
        let chunk = &signal[start..start + width];
        let level = rms(chunk);
        let mean = chunk.iter().map(|v| *v as f64).sum::<f64>() / width as f64;
        spectrum.fill(Complex::default());
        for i in 0..width {
            spectrum[i].re = ((chunk[i] as f64 - mean) * window[i]) as f32;
        }
        fft.process_with_scratch(&mut spectrum, &mut scratch);
        let mut residual = chunk.iter().map(|v| *v as f64 - mean).collect::<Vec<_>>();
        if let Some(base) = hz {
            for (index, h) in sound.harmonics.iter_mut().enumerate() {
                let expected = base * (index + 1) as f64;
                if expected >= rate * 0.45 {
                    continue;
                }
                let spread = (expected * 0.055).min(base * 0.4);
                let low = ((expected - spread) * 16384. / rate).floor().max(1.) as usize;
                let high = ((expected + spread) * 16384. / rate).floor().min(8190.) as usize;
                if low > high {
                    continue;
                }
                let bin = (low..=high)
                    .max_by(|&a, &b| spectrum[a].norm_sqr().total_cmp(&spectrum[b].norm_sqr()))
                    .unwrap();
                let (a, b, c) = (
                    spectrum[bin - 1].norm().max(1e-12).ln() as f64,
                    spectrum[bin].norm().max(1e-12).ln() as f64,
                    spectrum[bin + 1].norm().max(1e-12).ln() as f64,
                );
                let offset = if (a - 2. * b + c).abs() > 1e-9 {
                    (0.5 * (a - c) / (a - 2. * b + c)).clamp(-0.5, 0.5)
                } else {
                    0.
                };
                let frequency = (bin as f64 + offset) * rate / 16384.;
                let detune = (1200. * (frequency / expected).log2()).clamp(-100., 100.);
                let frequency = expected * 2_f64.powf(detune / 1200.);
                let (step_s, step_c) = (std::f64::consts::TAU * frequency / rate).sin_cos();
                let (mut s, mut c) = (0., 1.);
                let (mut ss, mut cc, mut sc, mut xs, mut xc) = (0., 0., 0., 0., 0.);
                for i in 0..width {
                    let w = window[i];
                    let x = chunk[i] as f64 - mean;
                    ss += s * s * w;
                    cc += c * c * w;
                    sc += s * c * w;
                    xs += x * s * w;
                    xc += x * c * w;
                    (s, c) = (s * step_c + c * step_s, c * step_c - s * step_s);
                }
                let determinant = ss * cc - sc * sc;
                if determinant.abs() < 1e-10 {
                    continue;
                }
                let sine = (xs * cc - xc * sc) / determinant;
                let cosine = (xc * ss - xs * sc) / determinant;
                let amplitude = sine.hypot(cosine);
                amplitudes[index] += amplitude / frames as f64;
                if level >= best_level {
                    h.detune = detune as f32;
                    let phase = cosine.atan2(sine)
                        - std::f64::consts::TAU * frequency * start as f64 / rate;
                    h.phase = ((phase.to_degrees() + 180.).rem_euclid(360.) - 180.) as f32;
                }
                (s, c) = (0., 1.);
                for x in &mut residual {
                    *x -= sine * s + cosine * c;
                    (s, c) = (s * step_c + c * step_s, c * step_c - s * step_s);
                }
            }
        }
        best_level = best_level.max(level);
        noise_energy += residual
            .iter()
            .zip(&window)
            .map(|(v, w)| v * v * w)
            .sum::<f64>();
        total_energy += chunk
            .iter()
            .zip(&window)
            .map(|(v, w)| (*v as f64 - mean).powi(2) * w)
            .sum::<f64>();
        // Broad spectral slope approximates the existing three editable noise colors.
        spectrum.fill(Complex::default());
        for i in 0..width {
            spectrum[i].re = (residual[i] * window[i]) as f32;
        }
        fft.process_with_scratch(&mut spectrum, &mut scratch);
        for bin in 1..8192 {
            let f = bin as f64 * rate / 16384.;
            if (100.0..1000.0).contains(&f) {
                low_energy += spectrum[bin].norm_sqr() as f64 / 900.;
            }
            if (2000.0..5000.0).contains(&f) {
                high_energy += spectrum[bin].norm_sqr() as f64 / 3000.;
            }
        }
    }
    let maximum_amp = amplitudes.iter().copied().fold(0., f64::max).max(1e-9);
    for (h, a) in sound.harmonics.iter_mut().zip(amplitudes) {
        h.amplitude = if a > maximum_amp * 0.015 {
            (a / maximum_amp * 0.85).clamp(0., 1.) as f32
        } else {
            0.
        };
        if h.amplitude == 0. {
            h.phase = 0.;
            h.detune = 0.;
        }
    }
    sound.noise.level = if hz.is_none() {
        0.8
    } else {
        (noise_energy / total_energy.max(1e-12))
            .sqrt()
            .clamp(0., 1.) as f32
            * 0.65
    };
    sound.noise.color = if low_energy > high_energy * 20. {
        NoiseColor::Brown
    } else if low_energy > high_energy * 3. {
        NoiseColor::Pink
    } else {
        NoiseColor::White
    };
    sound.noise.seed = signal.iter().step_by(31).fold(0x12345678u64, |seed, v| {
        seed.rotate_left(7) ^ v.to_bits() as u64
    });
    sound.brightness = 1.;
    sound.gain = (signal.iter().map(|v| v.abs()).fold(0., f32::max) * 1.5).clamp(0.15, 1.);
    let hop = (rate / 100.0) as usize;
    let envelope = signal.chunks(hop).map(rms).collect::<Vec<_>>();
    let peak = envelope.iter().copied().fold(0., f32::max);
    let peak_at = envelope.iter().position(|v| *v >= peak * 0.95).unwrap_or(0);
    let first = envelope.iter().position(|v| *v > peak * 0.1).unwrap_or(0);
    let last = envelope
        .iter()
        .rposition(|v| *v > peak * 0.1)
        .unwrap_or(peak_at);
    let tail = envelope
        .iter()
        .rposition(|v| *v > peak * 0.8)
        .unwrap_or(last);
    let sustain = envelope
        .iter()
        .skip(peak_at + 1)
        .take(last.saturating_sub(peak_at))
        .sum::<f32>()
        / last.saturating_sub(peak_at).max(1) as f32
        / peak.max(1e-6);
    sound.envelope = Envelope {
        attack_ms: ((peak_at - first) as f32 * 10.).clamp(1., 10000.),
        decay_ms: ((last - peak_at) as f32 * 3.).clamp(10., 10000.),
        sustain: sustain.clamp(0.05, 1.),
        release_ms: ((last.saturating_sub(tail) + 2) as f32 * 10.).clamp(20., 10000.),
    };
    sound
}
#[cfg(test)]
mod tests {
    include!(concat!(env!("OUT_DIR"), "/analysis_tests.rs"));
}
