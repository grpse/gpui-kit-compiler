use crate::{editor::Editor, state::Action};
use gpui_kit::{prelude::*, *};
use rsx_video_editor::{
    processing::{Stage, Update},
    project::Timeline,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};
impl Editor {
    pub fn update_processing(&mut self, update: Update) {
        if let Some(job) = self
            .processing
            .iter_mut()
            .find(|job| job.path == update.path)
        {
            *job = update;
        } else {
            self.processing.push(update);
        }
    }
    pub fn finish_processing(&mut self) {
        for job in &mut self.processing {
            if !matches!(job.stage, Stage::Ready | Stage::Failed | Stage::Cancelled) {
                job.stage = Stage::Cancelled;
                job.progress = None;
            }
        }
    }
    pub fn record_edit(&mut self, before: Timeline) {
        let after = Timeline::capture(&self.state);
        if before.tracks != after.tracks
            || before.clips != after.clips
            || before.sources != after.sources
        {
            self.history.push(before);
            if self.history.len() > 100 {
                self.history.remove(0);
            }
            self.future.clear();
        }
    }
    pub fn undo_timeline(&mut self, redo: bool, window: &mut Window, cx: &mut Context<Self>) {
        let snapshot = if redo {
            self.future.pop()
        } else {
            self.history.pop()
        };
        if let Some(snapshot) = snapshot {
            let current = Timeline::capture(&self.state);
            if redo {
                self.history.push(current);
            } else {
                self.future.push(current);
            }
            snapshot.restore(&mut self.state);
            self.inline_edit = None;
            self.state.notice = Some(if redo { "Edit restored" } else { "Edit undone" }.into());
            self.refresh_preview(window, cx);
            for (i, slider) in self.sliders.iter().enumerate() {
                slider.update(cx, |slider, cx| {
                    slider.set_value(self.state.controls[i], window, cx)
                });
            }
            cx.notify();
        }
    }
    fn directory(&self) -> PathBuf {
        self.project_path
            .as_ref()
            .and_then(|path| path.parent())
            .or_else(|| {
                self.state
                    .assets
                    .first()
                    .and_then(|asset| asset.path.as_deref())
                    .and_then(Path::parent)
            })
            .unwrap_or(Path::new("."))
            .into()
    }
    pub fn choose_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.exporting || self.importing || self.state.clips.is_empty() {
            self.state.notice = Some(
                "Wait for processing to finish and add media to the timeline before exporting."
                    .into(),
            );
            cx.notify();
            return;
        }
        let choice = cx.prompt_for_new_path(&self.directory(), Some("edited.mp4"));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(mut path))) = choice.await {
                path.set_extension("mp4");
                let _ = this.update(cx, |this, cx| this.start_export(path, cx));
            }
        })
        .detach();
    }
    pub fn start_export(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.exporting {
            return;
        }
        self.exporting = true;
        self.export_path = None;
        self.export_cancel = Arc::new(AtomicBool::new(false));
        self.export_progress = Some(rsx_video_editor::export::Progress {
            percent: 0,
            step: "Preparing export",
        });
        let cancel = self.export_cancel.clone();
        let state = self.state.clone();
        let cache = self.cache.clone();
        cx.spawn(async move |this, cx| {
            let (sender, receiver) = async_channel::bounded(128);
            let output = path.clone();
            let work = cx.background_spawn(async move {
                let result =
                    rsx_video_editor::export::run(&state, &output, cancel, &mut |update| {
                        let _ = sender.send_blocking(update);
                    });
                drop(cache);
                result
            });
            while let Ok(update) = receiver.recv().await {
                if this
                    .update(cx, |this, cx| {
                        this.export_progress = Some(update);
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.exporting = false;
                match result {
                    Ok(()) => {
                        this.export_path = Some(path.clone());
                        this.state.notice = Some(format!("Exported {}", path.display()));
                    }
                    Err(error) => {
                        this.export_progress = None;
                        this.state.notice = Some(error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub fn choose_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.importing {
            self.state.notice = Some("Wait for media processing to finish before saving.".into());
            cx.notify();
            return;
        }
        let choice = cx.prompt_for_new_path(&self.directory(), Some("project.flowcut"));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(mut path))) = choice.await {
                path.set_extension("flowcut");
                let snapshot = this
                    .update(cx, |this, _| {
                        (
                            this.state.clone(),
                            this.composition.clone(),
                            this.cache.clone(),
                        )
                    })
                    .ok();
                if let Some((state, composition, cache)) = snapshot {
                    let destination = path.clone();
                    let result = cx
                        .background_spawn(async move {
                            let result =
                                rsx_video_editor::project::save(&state, &composition, &destination);
                            drop(cache);
                            result
                        })
                        .await;
                    let _ = this.update(cx, |this, cx| {
                        this.state.notice = Some(match result {
                            Ok(()) => {
                                this.project_path = Some(path.clone());
                                format!("Saved {}", path.display())
                            }
                            Err(error) => error,
                        });
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }
    pub fn choose_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.importing || self.graph_rendering {
            self.state.notice = Some(
                "Wait for media processing or rendering to finish before opening another project."
                    .into(),
            );
            cx.notify();
            return;
        }
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open FlowCut project".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = choice.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update_in(cx, |this, window, cx| this.open_project(path, window, cx));
            }
        })
        .detach();
    }
    pub fn open_project(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.importing || self.graph_rendering {
            self.state.notice = Some(
                "Wait for media processing or rendering to finish before opening another project."
                    .into(),
            );
            cx.notify();
            return;
        }
        let Some(cache) = self.cache.clone() else {
            self.state.notice = Some("Cannot open project without a media cache".into());
            cx.notify();
            return;
        };
        self.importing = true;
        self.processing.clear();
        self.import_cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.import_cancel.clone();
        self.state.playing = false;
        self.output_worker.pause();
        self.preview_worker.pause();
        self.state.notice = Some("Opening project and preparing its media…".into());
        cx.spawn_in(window, async move |this, cx| {
            let (sender, receiver) = async_channel::bounded(128);
            let source = path.clone();
            let work = cx.background_spawn(async move {
                rsx_video_editor::project::load(&source, &cache.path, cancel, &mut |update| {
                    let _ = sender.send_blocking(update);
                })
            });
            while let Ok(update) = receiver.recv().await {
                if this
                    .update(cx, |this, cx| {
                        this.update_processing(update);
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
            let result = work.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.importing = false;
                this.finish_processing();
                match result {
                    Ok((state, composition)) => {
                        this.cancel_clip_gesture(window, cx);
                        this.inline_edit = None;
                        this.state = state;
                        this.composition = composition;
                        this.history.clear();
                        this.future.clear();
                        this.graph_history.clear();
                        this.graph_future.clear();
                        this.graph_result = None;
                        if let Some(old) = this.graph_frame.take() {
                            let _ = window.drop_image(old);
                        }
                        this.graph_render_revision = None;
                        this.project_path = Some(path.clone());
                        this.search
                            .update(cx, |input, cx| input.set_value("", window, cx));
                        this.header_search
                            .update(cx, |input, cx| input.set_value("", window, cx));
                        this.state.apply(Action::Screen(crate::state::Screen::Edit));
                        this.state.notice = Some(format!("Opened {}", path.display()));
                        this.refresh_preview(window, cx);
                    }
                    Err(error) => this.state.notice = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
