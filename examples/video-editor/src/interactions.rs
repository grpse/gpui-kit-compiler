//! Timeline and library context menus. All edits remain project-local.
use crate::{
    editor::{Editor, InlineEdit},
    state::{Action, Screen},
};
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::{prelude::*, *};

type Edit = Box<dyn Fn(&mut Editor, &mut Window, &mut Context<Editor>)>;
fn item(
    owner: &Entity<Editor>,
    label: impl Into<SharedString>,
    disabled: bool,
    edit: Edit,
) -> PopupMenuItem {
    let owner = owner.clone();
    PopupMenuItem::new(label)
        .disabled(disabled)
        .on_click(move |_, window, cx| {
            owner.update(cx, |editor, cx| edit(editor, window, cx));
        })
}
fn timeline_commands(
    mut menu: PopupMenu,
    owner: &Entity<Editor>,
    cx: &Context<PopupMenu>,
) -> PopupMenu {
    menu = menu.separator();
    for (label, command) in [("Add marker", "Marker"), ("Undo", "Undo"), ("Redo", "Redo")] {
        menu = menu.item(item(
            owner,
            label,
            false,
            Box::new(move |editor, window, cx| editor.dispatch(Action::Tool(command), window, cx)),
        ));
    }
    menu.item(item(
        owner,
        "Paste at playhead",
        !owner.read(cx).state.can_paste(),
        Box::new(|editor, window, cx| editor.dispatch(Action::PasteClips, window, cx)),
    ))
}

/// Empty timeline space operates on the selected clip, if one exists.
pub fn timeline_menu(
    menu: PopupMenu,
    owner: Entity<Editor>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let state = &owner.read(cx).state;
    let selected = state
        .clips
        .get(state.selected_clip)
        .map(|_| state.selected_clip);
    let menu = if let Some(index) = selected {
        clip_menu(menu, owner.clone(), index, window, cx)
    } else {
        timeline_commands(menu, &owner, cx)
    };
    menu.separator().item(item(
        &owner,
        "Add track",
        false,
        Box::new(|editor, window, cx| editor.dispatch(Action::AddTrack, window, cx)),
    ))
}

pub fn clip_menu(
    menu: PopupMenu,
    owner: Entity<Editor>,
    index: usize,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    owner.update(cx, |editor, cx| {
        editor.dispatch(Action::TargetClip(index), window, cx)
    });
    let state = &owner.read(cx).state;
    let Some(clip) = state.clips.get(index) else {
        return menu;
    };
    let locked = state.linked_locked(index);
    let split_disabled =
        locked || state.position <= clip.start || state.position >= clip.start + clip.length;
    let reveal_disabled = state.assets[clip.asset].path.is_none();
    let mut menu = menu
        .item(item(
            &owner,
            "Rename clip…",
            locked,
            Box::new(move |editor, window, cx| {
                editor.begin_inline(InlineEdit::Clip(index), window, cx)
            }),
        ))
        .item(item(
            &owner,
            "Edit timing…",
            locked,
            Box::new(move |editor, window, cx| {
                editor.begin_inline(InlineEdit::Timing(index), window, cx)
            }),
        ))
        .separator()
        .item(item(
            &owner,
            "Split at playhead",
            split_disabled,
            Box::new(move |editor, window, cx| {
                editor.dispatch(Action::TargetClip(index), window, cx);
                editor.dispatch(Action::Tool("Split"), window, cx);
            }),
        ))
        .item(item(
            &owner,
            "Cut",
            locked,
            Box::new(|editor, window, cx| editor.dispatch(Action::CutClips, window, cx)),
        ))
        .item(item(
            &owner,
            "Copy",
            false,
            Box::new(|editor, window, cx| editor.dispatch(Action::CopyClips, window, cx)),
        ))
        .item(item(
            &owner,
            "Duplicate linked clips",
            locked,
            Box::new(move |editor, window, cx| {
                editor.dispatch(Action::DuplicateClip(index), window, cx)
            }),
        ))
        .item(item(
            &owner,
            "Delete linked clips",
            locked,
            Box::new(move |editor, window, cx| {
                editor.dispatch(Action::TargetClip(index), window, cx);
                editor.dispatch(Action::Tool("Delete"), window, cx);
            }),
        ))
        .separator();
    for command in ["Ripple", "Speed", "Crop", "Audio", "Fade"] {
        menu = menu.item(item(
            &owner,
            command,
            locked,
            Box::new(move |editor, window, cx| {
                editor.dispatch(Action::TargetClip(index), window, cx);
                editor.dispatch(Action::Tool(command), window, cx);
            }),
        ));
    }
    let menu = menu.separator().item(item(
        &owner,
        "Show source in Finder",
        reveal_disabled,
        Box::new(move |editor, window, cx| {
            editor.dispatch(Action::TargetClip(index), window, cx);
            editor.dispatch(Action::ShowInFinder, window, cx);
        }),
    ));
    timeline_commands(menu, &owner, cx)
}
pub fn track_menu(
    menu: PopupMenu,
    owner: Entity<Editor>,
    index: usize,
    _window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let state = &owner.read(cx).state;
    let Some(track) = state.tracks.get(index) else {
        return menu;
    };
    let last = state.tracks.len() - 1;
    let toggle = if track.audio {
        Action::TrackMuted(index)
    } else {
        Action::TrackVisible(index)
    };
    let toggle_label = if track.audio {
        if track.muted {
            "Unmute track"
        } else {
            "Mute track"
        }
    } else if track.visible {
        "Hide video track"
    } else {
        "Show video track"
    };
    let lock_label = if track.locked {
        "Unlock track"
    } else {
        "Lock track"
    };
    menu.item(item(
        &owner,
        "Rename track…",
        false,
        Box::new(move |editor, window, cx| {
            editor.begin_inline(InlineEdit::Track(index), window, cx)
        }),
    ))
    .item(item(
        &owner,
        toggle_label,
        false,
        Box::new(move |editor, window, cx| editor.dispatch(toggle.clone(), window, cx)),
    ))
    .item(item(
        &owner,
        lock_label,
        false,
        Box::new(move |editor, window, cx| editor.dispatch(Action::TrackLocked(index), window, cx)),
    ))
    .separator()
    .item(item(
        &owner,
        "Move track up",
        index == 0,
        Box::new(move |editor, window, cx| {
            editor.dispatch(
                Action::MoveTrack {
                    from: index,
                    to: index.saturating_sub(1),
                },
                window,
                cx,
            )
        }),
    ))
    .item(item(
        &owner,
        "Move track down",
        index == last,
        Box::new(move |editor, window, cx| {
            editor.dispatch(
                Action::MoveTrack {
                    from: index,
                    to: index + 1,
                },
                window,
                cx,
            )
        }),
    ))
    .item(item(
        &owner,
        "Move track to top",
        index == 0,
        Box::new(move |editor, window, cx| {
            editor.dispatch(Action::MoveTrack { from: index, to: 0 }, window, cx)
        }),
    ))
    .item(item(
        &owner,
        "Move track to bottom",
        index == last,
        Box::new(move |editor, window, cx| {
            editor.dispatch(
                Action::MoveTrack {
                    from: index,
                    to: last,
                },
                window,
                cx,
            )
        }),
    ))
    .separator()
    .item(item(
        &owner,
        "Delete track",
        track.locked,
        Box::new(move |editor, window, cx| editor.dispatch(Action::DeleteTrack(index), window, cx)),
    ))
}
pub fn media_menu(
    menu: PopupMenu,
    owner: Entity<Editor>,
    id: usize,
    _window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let Some(asset) = owner.read(cx).state.assets.get(id) else {
        return menu;
    };
    let unreadable = asset.metadata_error.is_some();
    let no_path = asset.path.is_none();
    let favorite = asset.favorite;
    menu.item(item(
        &owner,
        "Add to timeline",
        unreadable,
        Box::new(move |editor, window, cx| {
            editor.dispatch(Action::TargetAsset(id), window, cx);
            editor.dispatch(Action::AddToTimeline, window, cx);
        }),
    ))
    .item(item(
        &owner,
        "Preview source",
        unreadable,
        Box::new(move |editor, window, cx| {
            editor.dispatch(Action::Screen(Screen::Library), window, cx);
            editor.dispatch(Action::Select(id), window, cx);
        }),
    ))
    .item(item(
        &owner,
        if favorite {
            "Remove from favorites"
        } else {
            "Add to favorites"
        },
        false,
        Box::new(move |editor, window, cx| {
            editor.dispatch(Action::FavoriteAsset(id), window, cx);
        }),
    ))
    .separator()
    .item(item(
        &owner,
        "Show in Finder",
        no_path,
        Box::new(move |editor, window, cx| {
            editor.dispatch(Action::ShowAssetInFinder(id), window, cx);
        }),
    ))
}

#[derive(Clone)]
pub struct TrackDrag {
    pub index: usize,
    pub revision: u64,
    pub owner: EntityId,
    pub name: String,
}
pub struct TrackDragPreview {
    pub name: String,
}
impl Render for TrackDragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        crate::generated::tracks::drag_preview(self.name.clone())
    }
}

pub fn node_menu(
    menu: PopupMenu,
    owner: Entity<Editor>,
    id: u64,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    owner.update(cx, |editor, cx| editor.select_node(id, window, cx));
    menu.item(item(
        &owner,
        "Duplicate node",
        false,
        Box::new(move |editor, window, cx| editor.duplicate_node(id, window, cx)),
    ))
    .item(item(
        &owner,
        "Frame upstream branch",
        false,
        Box::new(move |editor, window, cx| editor.frame_branch(id, window, cx)),
    ))
    .item(item(
        &owner,
        "Mute / unmute",
        false,
        Box::new(move |editor, _, cx| editor.mute_node(id, cx)),
    ))
    .item(item(
        &owner,
        "Disconnect inputs",
        false,
        Box::new(move |editor, _, cx| {
            let count = editor
                .composition
                .node(id)
                .map(|n| n.inputs.len())
                .unwrap_or(0);
            editor.remember_graph();
            for port in 0..count {
                editor.composition.disconnect(id, port);
            }
            cx.notify();
        }),
    ))
    .separator()
    .item(item(
        &owner,
        "Remove node",
        false,
        Box::new(move |editor, _, cx| editor.delete_node(id, cx)),
    ))
}
pub fn graph_input_menu(menu: PopupMenu, owner: Entity<Editor>, id: u64, port: usize) -> PopupMenu {
    menu.item(item(
        &owner,
        "Disconnect input",
        false,
        Box::new(move |editor, _, cx| {
            editor.remember_graph();
            editor.composition.disconnect(id, port);
            cx.notify();
        }),
    ))
}
pub fn graph_menu(
    mut menu: PopupMenu,
    owner: Entity<Editor>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    use rsx_video_editor::composition::{Category, Operation};
    for category in Category::ALL {
        let operations: Vec<_> = Operation::palette()
            .into_iter()
            .filter(|op| op.category() == category)
            .collect();
        if operations.is_empty() {
            continue;
        }
        let owner = owner.clone();
        menu = menu.submenu(category.label(), window, cx, move |mut menu, _, _| {
            for operation in &operations {
                let operation = operation.clone();
                menu = menu.item(item(
                    &owner,
                    operation.label(),
                    false,
                    Box::new(move |editor, window, cx| {
                        editor.add_node(operation.clone(), window, cx)
                    }),
                ));
            }
            menu
        });
    }
    menu.separator().item(item(
        &owner,
        "Arrange nodes",
        false,
        Box::new(|editor, _, cx| editor.arrange_graph(cx)),
    ))
}
pub fn graph_enum_menu(
    mut menu: PopupMenu,
    owner: Entity<Editor>,
    index: usize,
    choices: Vec<rsx_video_editor::blender_catalog::Choice>,
) -> PopupMenu {
    for choice in choices {
        menu = menu.item(item(
            &owner,
            choice.label,
            false,
            Box::new(move |editor, window, cx| {
                editor.set_graph_parameter(index, choice.value.clone(), window, cx)
            }),
        ));
    }
    menu
}
