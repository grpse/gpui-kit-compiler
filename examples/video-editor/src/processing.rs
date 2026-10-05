//! Progress reports describe actual work; packet positions measure the demux pass.
use std::path::PathBuf;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Queued,
    Inspecting,
    Decoding,
    Finalizing,
    Thumbnail,
    Ready,
    Failed,
    Cancelled,
}
impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Inspecting => "Inspecting streams",
            Self::Decoding => "Indexing video · decoding audio · measuring waveforms",
            Self::Finalizing => "Saving channel caches, waveforms and source index",
            Self::Thumbnail => "Generating source thumbnail",
            Self::Ready => "Ready for editing",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
        }
    }
}
#[derive(Clone, Debug)]
pub struct Update {
    pub path: PathBuf,
    pub stage: Stage,
    pub progress: Option<u8>,
    pub error: Option<String>,
}
impl Update {
    pub fn new(path: &std::path::Path, stage: Stage, progress: Option<u8>) -> Self {
        Self {
            path: path.into(),
            stage,
            progress,
            error: None,
        }
    }
}
