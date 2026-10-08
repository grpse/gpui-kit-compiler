pub mod model {
    include!(concat!(env!("OUT_DIR"), "/model.rs"));
}
pub mod persistence {
    include!(concat!(env!("OUT_DIR"), "/persistence.rs"));
}
pub mod storage {
    include!(concat!(env!("OUT_DIR"), "/storage.rs"));
}

pub mod engine {
    include!(concat!(env!("OUT_DIR"), "/engine.rs"));
}
pub mod analysis {
    include!(concat!(env!("OUT_DIR"), "/analysis.rs"));
}
#[cfg(feature="audio-output")]
pub mod capture {
    include!(concat!(env!("OUT_DIR"), "/capture.rs"));
}
#[cfg(feature="audio-output")]
pub mod audio {
    include!(concat!(env!("OUT_DIR"), "/audio.rs"));
}

pub mod waveform {include!(concat!(env!("OUT_DIR"), "/waveform.rs"));}
