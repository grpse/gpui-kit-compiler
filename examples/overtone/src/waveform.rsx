// Bounded display data for recorded waveforms and the editable harmonic sum.
// These buffers are transient; projects store harmonic parameters and source references.
use crate::{analysis::Recording,model::*};
use std::{f64::consts::TAU,sync::Arc};
pub const PLOT_POINTS:usize=2048;
#[derive(Clone)]
pub struct SourcePreview {
    pub samples:Arc<[f32]>,pub sample_rate:f64,pub duration:f64,
    pub overview:Arc<[(f32,f32)]>,pub peak_time:f64,
}
impl SourcePreview {
    pub fn new(recording:Recording)->Result<Self,String> {
        if !(8000..=192000).contains(&recording.sample_rate) || recording.samples.is_empty()
            || recording.samples.len()>recording.sample_rate as usize*120
            || recording.samples.iter().any(|x|!x.is_finite()) {return Err("Invalid waveform source".into());}
        let duration=recording.samples.len() as f64/recording.sample_rate as f64;
        let overview=peak_bins(&recording.samples,PLOT_POINTS).into();
        let hop=(recording.sample_rate/100) as usize;
        let peak=recording.samples.chunks(hop).enumerate().max_by(|(_,a),(_,b)|{
            let energy=|s:&[f32]|s.iter().map(|v|(*v as f64).powi(2)).sum::<f64>()/s.len() as f64;
            energy(a).total_cmp(&energy(b))
        }).map(|(i,_)|i).unwrap_or(0);
        let reduction=recording.sample_rate.div_ceil(48000) as usize;
        let samples=recording.samples.chunks(reduction).map(|s|s.iter().sum::<f32>()/s.len() as f32).collect::<Vec<_>>().into();
        Ok(Self {samples,sample_rate:recording.sample_rate as f64/reduction as f64,duration,overview,peak_time:(peak as f64+0.5)*hop as f64/recording.sample_rate as f64})
    }
    pub fn excerpt(&self,start:f64,duration:f64)->Arc<[(f32,f32)]> {
        let mut out=Vec::with_capacity(PLOT_POINTS);
        for i in 0..PLOT_POINTS {
            let a=((start+duration*i as f64/PLOT_POINTS as f64)*self.sample_rate).max(0.);
            let b=((start+duration*(i+1) as f64/PLOT_POINTS as f64)*self.sample_rate).max(0.);
            if b-a>=1. {
                let from=(a.floor() as usize).min(self.samples.len());let to=(b.ceil() as usize).min(self.samples.len());
                let mut lo=f32::INFINITY;let mut hi=f32::NEG_INFINITY;
                for &v in &self.samples[from..to] {lo=lo.min(v);hi=hi.max(v);}
                out.push(if lo.is_finite(){(lo,hi)}else{(0.,0.)});
            } else {
                let at=(a.floor() as usize).min(self.samples.len()-1);let next=(at+1).min(self.samples.len()-1);
                let v=self.samples[at]+(self.samples[next]-self.samples[at])*(a-a.floor()) as f32;
                out.push((v,v));
            }
        }
        out.into()
    }
}
fn peak_bins(samples:&[f32],columns:usize)->Vec<(f32,f32)> {
    (0..columns).map(|i|{
        let from=i*samples.len()/columns;let to=((i+1)*samples.len()/columns).max(from+1).min(samples.len());
        samples[from.min(samples.len()-1)..to].iter().fold((f32::INFINITY,f32::NEG_INFINITY),|(lo,hi),&v|(lo.min(v),hi.max(v)))
    }).collect()
}
#[derive(Clone)]
pub struct SineDisplay {pub partials:Vec<Arc<[f32]>>,pub sum:Arc<[f32]>}
pub fn sine_display(sound:&Sound,hz:f64,start:f64,seconds:f64)->SineDisplay {
    let normalization=sound.gain as f64/(sound.harmonics.iter().map(|h|h.amplitude as f64).sum::<f64>()+sound.noise.level as f64).max(1.);
    let mut sum=vec![0.;PLOT_POINTS];let mut partials=Vec::with_capacity(sound.harmonics.len());
    for h in &sound.harmonics {
        let frequency=hz*h.multiple as f64*2_f64.powf(h.detune as f64/1200.);
        let phase=h.phase as f64*TAU/360.;let amplitude=h.amplitude as f64*normalization;
        let values=(0..PLOT_POINTS).map(|i|{
            let time=start+seconds*i as f64/(PLOT_POINTS-1) as f64;
            let value=(amplitude*(TAU*frequency*time+phase).sin()) as f32;sum[i]+=value;value
        }).collect::<Vec<_>>();partials.push(values.into());
    }
    SineDisplay {partials,sum:sum.into()}
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn overview_keeps_transients_and_excerpt_has_real_samples() {
        let mut values=vec![0.;48000];values[12345]=0.9;values[12346]=-0.7;
        let source=SourcePreview::new(Recording{samples:values,sample_rate:48000}).unwrap();
        assert!(source.overview.iter().any(|(_,hi)|*hi==0.9));assert!(source.overview.iter().any(|(lo,_)|*lo== -0.7));
        assert!(source.excerpt(0.25,0.02).iter().any(|(_,hi)|*hi>0.2));assert_eq!(source.duration,1.);
    }
    #[test] fn curves_sum_exactly_and_deleted_harmonics_keep_their_frequency() {
        let mut sound=Sound::new(0,"Wave");sound.resize_harmonics(3);sound.remove_harmonic(1);
        let wave=sine_display(&sound,220.,0.017,0.02);
        for i in 0..PLOT_POINTS {assert!((wave.partials.iter().map(|v|v[i]).sum::<f32>()-wave.sum[i]).abs()<1e-6);}
        let h=&sound.harmonics[1];assert_eq!(h.multiple,3);
        let scale=sound.gain as f64/(sound.harmonics.iter().map(|h|h.amplitude as f64).sum::<f64>()+sound.noise.level as f64).max(1.);
        let expected=h.amplitude as f64*scale*(TAU*660.*0.017).sin();assert!((wave.partials[1][0] as f64-expected).abs()<1e-6);
    }
    #[test] fn cleanup_keeps_strong_partials_and_noise_is_opt_in() {
        let mut sound=Sound::new(0,"Clean");sound.harmonics[0].phase=42.;sound.harmonics[1].amplitude=0.001;
        let original=sound.clone();let weak=sound.weak_harmonics(0.05);assert!(weak.contains(&2));
        let (removed,noise)=sound.clean_harmonics(0.05,false);assert!(removed>0);assert!(!noise);assert_eq!(sound.noise,original.noise);assert_eq!(sound.harmonics[0],original.harmonics[0]);
        assert_eq!(sound.clean_harmonics(0.05,true).1,true);assert_eq!(sound.noise.level,0.);
        for h in &mut sound.harmonics {h.amplitude=0.;}sound.clean_harmonics(0.5,false);assert_eq!(sound.harmonics.len(),1);
    }
}
