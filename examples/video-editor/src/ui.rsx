use gpui_kit::{prelude::*, *};
use crate::{editor::Editor,state::Action,generated::primitives::*};

/// The application entry point. Native state can be adopted independently of RSX views.
pub fn entry(window:&mut Window,cx:&mut App)->Entity<Editor> {
    cx.new(|cx|Editor::new(window,cx))
}

#[gpui]
pub fn editor_view(editor: &Editor, window: &mut Window, cx: &mut Context<Editor>) -> impl IntoElement {
    let editing = editor.state.screen.has_timeline();
    let workspace = if editing {
        editor.edit_workspace.area.clone()
    } else {
        editor.library_workspace.area.clone()
    };
    <div relative flex flex-col size-full overflow-hidden bg={rgb(BG)} text-color={rgb(TEXT)}>
        {crate::generated::chrome::topbar(editor,cx)}
        <div relative flex-1 min-h-0 size-full>
            {if editor.state.fullscreen {
                let width = f32::from(window.bounds().size.width);
                let height = (f32::from(window.bounds().size.height) - 58.).max(100.);
                crate::generated::player::player(editor,width, height, cx).into_any_element()
            } else {
                workspace.into_any_element()
            }}
        </div>
        {if let Some(notice) = editor.state.notice.clone() {
            <div absolute bottom={px(34.)} right={px(20.)} flex items-center gap-3 p-3 bg={rgb(RAISED)} border-1 border-color={rgb(PURPLE)} rounded-lg shadow-lg>
                <div text-size={px(12.)}>{notice}</div>
                {tool("dismiss", "Dismiss", Some(gpui_kit::assets::IconName::X), Action::Dismiss, false, cx)}
            </div>.into_any_element()
        } else {
            div().into_any_element()
        }}
    </div>
}
