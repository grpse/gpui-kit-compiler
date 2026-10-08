impl Studio {
    pub fn reconstruction_sound(&self)->&Sound {
        if self.reconstruction_instance {self.project.selected().map(|c|&c.sound).unwrap_or(&self.project.draft)}else{&self.project.draft}
    }
    pub fn prepare_reconstruction(&mut self,window:&mut Window,cx:&mut Context<Self>) {
        if self.project.selected().is_none(){self.reconstruction_instance=false;}
        if !self.project.profile().panels.iter().any(|p|p.module==Module::Reconstruction && p.visible){return;}
        let key=self.reconstruction_sound().source.and_then(|id|self.project.sources.iter().find(|s|s.id==id)).map(|s|crate::reconstruction::SourceKey{id:s.id,path:s.path.clone(),hash:s.sha256.clone(),bytes:s.bytes});
        if self.waveform_source.key!=key {
            self.waveform_epoch=self.waveform_epoch.wrapping_add(1);self.waveform_position=None;
            self.waveform_source=crate::reconstruction::SourceState{key:key.clone(),loading:key.is_some(),..Default::default()};
            if let Some(key)=key {
                let epoch=self.waveform_epoch;let generation=self.generation;
                cx.spawn_in(window,async move|this,cx|{
                    let result=cx.background_spawn(async move{
                        let current=persistence::import_source(&key.path)?;
                        if current.sha256!=key.hash || current.bytes!=key.bytes{return Err("Recording changed; import and analyze it again".into());}
                        let recording=rsx_overtone::analysis::decode(&key.path)?;
                        rsx_overtone::waveform::SourcePreview::new(recording).map(std::sync::Arc::new)
                    }).await;
                    let _=this.update(cx,|s,cx|{if s.waveform_epoch!=epoch || s.generation!=generation{return;}
                        s.waveform_source.loading=false;match result{Ok(source)=>s.waveform_source.preview=Some(source),Err(e)=>s.waveform_source.error=Some(e)}
                        s.reconstruction_plot_key=None;cx.notify();
                    });
                }).detach();
            }
        }
        let sound=self.reconstruction_sound().clone();
        let hz=sound.capture.as_ref().and_then(|c|c.fundamental_hz).unwrap_or_else(||self.project.tuning.reference_hz*2_f64.powf((60.-self.project.tuning.reference_midi as f64)/12.));
        let source_from=sound.capture.as_ref().map(|c|c.start_seconds).unwrap_or(0.);
        let duration=sound.capture.as_ref().map(|c|c.duration_seconds).or_else(||self.waveform_source.preview.as_ref().map(|p|(p.duration-source_from).max(0.001))).unwrap_or(2.);
        let seconds=(self.project.reconstruction.cycles as f64/hz).min(duration).max(0.00005);
        let default_time=self.waveform_source.preview.as_ref().map(|p|(p.peak_time-source_from-seconds/2.).clamp(0.,(duration-seconds).max(0.))).unwrap_or(0.);
        let time=self.waveform_position.unwrap_or(default_time).clamp(0.,(duration-seconds).max(0.));
        let plot_key=(sound.clone(),hz,time,self.project.reconstruction.cycles,self.waveform_epoch,self.waveform_source.preview.is_some());
        if self.reconstruction_plot_key.as_ref()!=Some(&plot_key) {
            let sines=rsx_overtone::waveform::sine_display(&sound,hz,time,seconds);
            let original=self.waveform_source.preview.as_ref().map(|p|p.excerpt(source_from+time,seconds));
            self.reconstruction_plot=Some(std::sync::Arc::new(crate::reconstruction::PlotData{sines,original,hz,time,seconds,source_from}));
            self.reconstruction_plot_key=Some(plot_key);
        }
    }
    pub fn seek_reconstruction(&mut self,fraction:f64,cx:&mut Context<Self>) {
        let Some(preview)=&self.waveform_source.preview else{return;};
        let Some(plot)=&self.reconstruction_plot else{return;};
        self.waveform_position=Some((fraction.clamp(0.,1.)*preview.duration-plot.source_from-plot.seconds/2.).max(0.));cx.notify();
    }
    pub fn clean_reconstruction(&mut self,window:&mut Window,cx:&mut Context<Self>) {
        let instance=self.reconstruction_instance;let threshold=self.project.reconstruction.weak_threshold;let noise=self.project.reconstruction.remove_noise;
        let selected=if instance{self.instance_harmonic}else{self.selected_harmonic};
        let mut selection=0;
        if let Some(sound)=self.edit_sound(instance) {
            let multiple=sound.harmonics.get(selected).map(|h|h.multiple);
            let (count,noise_removed)=sound.clean_harmonics(threshold,noise);
            selection=multiple.and_then(|m|sound.harmonics.iter().position(|h|h.multiple==m)).unwrap_or(selected.min(sound.harmonics.len()-1));
            self.notice=format!("Removed {count} weak harmonics{} · Undo restores the sound",if noise_removed{" and residual noise"}else{""});
        }
        if instance{self.instance_harmonic=selection;}else{self.selected_harmonic=selection;self.harmonic_bank=selection/8;}
        self.sync_sliders(instance,window,cx);self.changed(cx);
    }
    pub fn preview_reconstruction(&mut self,window:&mut Window,cx:&mut Context<Self>) {
        let owner=if self.reconstruction_instance{self.project.selected_clip.unwrap_or(0)}else{0};
        self.play_patch(self.reconstruction_sound().clone(),owner,window,cx);
    }
}
