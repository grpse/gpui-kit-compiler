// Studio control plane for recording, analysis and individual synthesized sound playback.
impl Studio {
    pub fn recording_active(&self) -> bool {
        #[cfg(feature = "audio-output")]
        {
            self.capture_starting || self.capture.is_some()
        }
        #[cfg(not(feature = "audio-output"))]
        {
            self.capture_starting
        }
    }
    pub fn set_analysis_option(&mut self, which: u8, delta: i32) {
        let s = &mut self.project.recording;
        match which {
            0 => s.max_harmonics = (s.max_harmonics as i32 + delta).clamp(1, 32) as usize,
            1 => s.silence_db = (s.silence_db + delta as f32).clamp(-80., -10.),
            2 => s.split_gap_ms = (s.split_gap_ms as i32 + delta).clamp(50, 1000) as u32,
            _ => s.min_sound_ms = (s.min_sound_ms as i32 + delta).clamp(50, 2000) as u32,
        }
    }
    pub fn start_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.recording_active() || self.pending.is_some() {
            return;
        }
        #[cfg(not(feature = "audio-output"))]
        {
            let _ = window;
            self.notice = "Microphone capture requires --features audio-output.".into();
            self.show_status = true;
            cx.notify();
        }
        #[cfg(feature = "audio-output")]
        {
            self.stop_audio();
            self.busy = true;
            self.capture_starting = true;
            self.capture_seconds = 0.;
            self.capture_peak = 0.;
            self.capture_epoch = self.capture_epoch.wrapping_add(1);
            let epoch = self.capture_epoch;
            let generation = self.generation;
            self.notice = "Opening microphone…".into();
            cx.notify();
            cx.spawn_in(window, async move |this, cx| {
                let result = cx
                    .background_spawn(async move { rsx_overtone::capture::CaptureSession::start() })
                    .await;
                let installed = this
                    .update_in(cx, |s, window, cx| {
                        if s.generation != generation || s.capture_epoch != epoch {
                            return false;
                        }
                        s.busy = false;
                        s.capture_starting = false;
                        match result {
                            Ok(capture) => {
                                s.capture_device = capture.device_name.clone();
                                s.capture = Some(capture);
                                s.notice =
                                    "Recording · Stop & split adds individual sounds to the timeline".into();
                                if s.capture_after.is_some() || s.capture_save.is_some() {
                                    s.finish_recording(false, window, cx);
                                    return false;
                                }
                                cx.notify();
                                true
                            }
                            Err(e) => {
                                s.notice = e;
                                s.show_status = true;
                                s.capture_after = None;
                                s.capture_save = None;
                                cx.notify();
                                false
                            }
                        }
                    })
                    .unwrap_or(false);
                if !installed {
                    return;
                }
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(100))
                        .await;
                    let keep = this
                        .update_in(cx, |s, window, cx| {
                            if s.capture_epoch != epoch {
                                return false;
                            }
                            let Some(capture) = s.capture.as_ref() else {
                                return false;
                            };
                            let status = capture.status();
                            s.capture_seconds = status.seconds;
                            s.capture_peak = status.peak;
                            if status.failed || status.complete {
                                s.finish_recording(false, window, cx);
                                return false;
                            }
                            cx.notify();
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
    pub fn finish_recording(&mut self, discard: bool, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(not(feature = "audio-output"))]
        {
            let _ = (discard, window, cx);
        }
        #[cfg(feature = "audio-output")]
        {
            let Some(capture) = self.capture.take() else {
                return;
            };
            self.capture_epoch = self.capture_epoch.wrapping_add(1);
            self.capture_peak = 0.;
            self.busy = true;
            let settings = self.project.recording.clone();
            let generation = self.generation;
            self.notice = if discard {
                "Discarding take…"
            } else {
                "Splitting the recording into sounds…"
            }
            .into();
            cx.notify();
            cx.spawn_in(window, async move |this, cx| {
                if discard {
                    let result = cx.background_spawn(async move { capture.discard() }).await;
                    let _ = this.update(cx, |s, cx| {
                        s.busy = false;
                        s.capture_after = None;
                        s.capture_save = None;
                        s.notice = match result {
                            Ok(()) => "Take discarded".into(),
                            Err(e) => e,
                        };
                        cx.notify();
                    });
                    return;
                }
                let result = cx
                    .background_spawn(async move {
                        let path = capture.finish()?;
                        let mut source = persistence::import_source(&path)?;
                        source.analysis = settings.clone();
                        let analysis = rsx_overtone::analysis::split_file(&path, &settings);
                        Ok::<_, String>((source, analysis))
                    })
                    .await;
                let _ = this.update_in(cx, |s, window, cx| {
                    if s.generation != generation {
                        return;
                    }
                    s.busy = false;
                    match result {
                        Ok((mut source, analysis)) => {
                            source.id = s.project.allocate();
                            source.name = format!("Recording {}", s.project.sources.len() + 1);
                            let id = source.id;
                            s.project.sources.push(source);
                            s.project.draft.source = Some(id);
                            s.project.draft.capture = None;
                            s.accept_analysis(id, analysis, window, cx);
                            if let Some(pending) = s.capture_after.take() {
                                s.capture_save = None;
                                s.request_replace(pending, window, cx);
                            } else if let Some(as_new) = s.capture_save.take() {
                                s.save(as_new, window, cx);
                            }
                        }
                        Err(e) => {
                            s.notice = e;
                            s.show_status = true;
                            s.capture_after = None;
                            s.capture_save = None;
                            cx.notify();
                        }
                    }
                });
            })
            .detach();
        }
    }
    pub fn analyze_voice(&mut self, id: Id, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.recording_active() || self.pending.is_some() {
            return;
        }
        let Some(source) = self.project.sources.iter().find(|s| s.id == id) else {
            return;
        };
        let path = source.path.clone();
        let hash = source.sha256.clone();
        let bytes = source.bytes;
        let settings = self.project.recording.clone();
        let generation = self.generation;
        self.stop_audio();
        self.busy = true;
        self.notice = "Splitting the recording into sounds…".into();
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let current = persistence::import_source(&path)?;
                    if current.sha256 != hash || current.bytes != bytes {
                        return Err("Recording changed; import it again before analyzing".into());
                    }
                    rsx_overtone::analysis::split_file(&path, &settings)
                })
                .await;
            let _ = this.update_in(cx, |s, window, cx| {
                if s.generation != generation {
                    return;
                }
                s.busy = false;
                if let Some(source) = s.project.sources.iter_mut().find(|v| v.id == id) {
                    source.analysis = s.project.recording.clone();
                }
                s.accept_analysis(id, result, window, cx);
            });
        })
        .detach();
    }
    /// One undo checkpoint for the recording, its sounds and timeline clips.
    pub fn accept_analysis(
        &mut self,
        id: Id,
        result: Result<rsx_overtone::analysis::AnalysisResult, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(result) => {
                if self.project.library.len() + result.sounds.len() > 10000 {
                    self.notice =
                        "Library limit reached; recording retained for later analysis".into();
                    self.show_status = true;
                    self.changed(cx);
                    return;
                }
                let name = self
                    .project
                    .sources
                    .iter()
                    .find(|s| s.id == id)
                    .map(|s| {
                        std::path::Path::new(&s.name)
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    })
                    .unwrap_or_else(|| "Recording".into());
                let count=result.sounds.len();
                if self.project.clips.len()+count>100000 {
                    self.notice="Timeline limit reached; recording retained".into();self.changed(cx);return;
                }
                let clips=self.project.add_recorded_sounds(id,&name,result.sounds);
                let first=clips.first().and_then(|id|self.project.clips.iter().find(|c|c.id==*id)).map(|c|c.sound.clone());
                if let Some(sound) = first {
                    self.reconstruction_instance=false;self.waveform_position=None;
                    self.project.editing_sound = Some(sound.id);
                    self.project.draft = sound;
                    self.harmonic_bank = 0;
                    self.selected_harmonic = 0;
                    self.sound_name.update(cx, |s, cx| {
                        s.set_value(self.project.draft.name.clone(), window, cx)
                    });
                    self.sync_sliders(false, window, cx);
                }
                self.query.clear();
                self.search.update(cx, |s, cx| s.set_value("", window, cx));
                self.notice = if count == 0 {
                    "No sounds detected. Lower the silence threshold or minimum sound length, then split again.".into()
                } else {
                    format!(
                        "Added {count} individual sound(s) to Sounds and the timeline. Drag or duplicate clips to compose."
                    )
                };
            }
            Err(e) => {
                self.notice = format!("Analysis failed: {e}. Recording retained.");
                self.show_status = true;
            }
        }
        self.changed(cx);
    }
    pub fn play_sound(&mut self, id: Id, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sound)=self.project.library.iter().find(|s|s.id==id).cloned() else{return;};
        self.play_patch(sound,id,window,cx);
    }
    pub fn play_patch(&mut self,sound:Sound,id:Id,window:&mut Window,cx:&mut Context<Self>) {
        if self.busy || self.recording_active() || self.pending.is_some() {
            return;
        }
        if self.auditioning == Some(id) {
            self.stop_audio();
            cx.notify();
            return;
        }
        #[cfg(not(feature = "audio-output"))]
        {
            let _ = (sound,id, window);
            self.notice = "Sound playback requires --features audio-output.".into();
            self.show_status = true;
            cx.notify();
        }
        #[cfg(feature = "audio-output")]
        {
            self.stop_audio();
            self.audio_starting = true;
            self.auditioning = Some(id);
            let project=self.project.clone();
            let tuning = project.tuning.clone();
            let generation = self.generation;
            let audio_generation = self.audio_generation;
            cx.spawn_in(window, async move |this, cx| {
                let result = cx
                    .background_spawn(async move {
                        let mut audio = rsx_overtone::audio::AudioSession::start(None, tuning)?;
                        if sound.recorded_sample{audio.audition_recording(&project,&sound)?;}else{audio.audition(&sound,id)?;}
                        Ok::<_, String>(audio)
                    })
                    .await;
                let _ = this.update(cx, |s, cx| {
                    if s.generation != generation || s.audio_generation != audio_generation {
                        return;
                    }
                    s.audio_starting = false;
                    match result {
                        Ok(audio) => {
                            s.audio = Some(audio);
                            s.notice = "Playing sound".into();
                        }
                        Err(e) => {
                            s.auditioning = None;
                            s.notice = e;
                            s.show_status = true;
                        }
                    }
                    cx.notify();
                });
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(100))
                        .await;
                    let keep = this
                        .update(cx, |s, cx| {
                            if s.audio_generation != audio_generation || s.auditioning != Some(id) {
                                return false;
                            }
                            let Some(audio) = s.audio.as_ref() else {
                                return false;
                            };
                            if audio.failed() || audio.finished() {
                                let failed = audio.failed();
                                s.stop_audio();
                                if failed {
                                    s.notice = "Audio device playback failed".into();
                                    s.show_status = true;
                                }
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
}

impl Studio {
    pub fn prepare_sound_waveforms(&mut self,window:&mut Window,cx:&mut Context<Self>) {
        for source in self.project.sources.clone() {
            if self.sound_waveforms.contains_key(&source.sha256){continue;}
            self.sound_waveforms.insert(source.sha256.clone(),None);
            let generation=self.generation;
            cx.spawn_in(window,async move|this,cx|{
                let key=source.sha256.clone();
                let result=cx.background_spawn(async move{
                    let current=persistence::import_source(&source.path)?;
                    if current.sha256!=source.sha256 || current.bytes!=source.bytes{return Err("Recording changed".to_string());}
                    rsx_overtone::waveform::SourcePreview::new(rsx_overtone::analysis::decode(&source.path)?)
                }).await;
                let _=this.update(cx,|s,cx|{
                    if generation!=s.generation{return;}
                    match result{Ok(preview)=>{s.sound_waveforms.insert(key,Some(std::sync::Arc::new(preview)));},Err(e)=>{s.notice=format!("Waveform unavailable: {e}");}}
                    cx.notify();
                });
            }).detach();
        }
    }
}
