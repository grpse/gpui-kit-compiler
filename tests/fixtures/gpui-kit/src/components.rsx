// Examples adapted from GPUI Kit 0.7.0 documentation code (Apache-2.0).
// Every component page has a source URL below. Checked on 2026-10-04.
use gpui_kit::{prelude::*, *};
use gpui_kit::component::{self as c, ActiveTheme as _, Sizable as _,
    button::ButtonVariants as _, scroll::ScrollableElement as _,
    menu::ContextMenuExt as _, FocusTrapElement as _, WindowExt as _,
    group_box::GroupBoxVariants as _};

gpui_kit::actions!(compatibility, [Copy, Paste]);

/// https://gpui-kit.com/component/accordion/
#[gpui]
pub fn accordion() -> impl IntoElement {
    <c::accordion::Accordion args={"sections"} item={|item| <div ctor={item} title="Section">Content</div>} />
}

/// https://gpui-kit.com/component/alert/
#[gpui]
pub fn alert() -> impl IntoElement {
    <c::alert::Alert args={("notice", "Saved")} />
}

/// https://gpui-kit.com/component/attachment/
#[gpui]
pub fn attachment() -> impl IntoElement {
    <c::attachment::Attachment
    media={<c::attachment::AttachmentMedia><c::Icon args={c::IconName::FileText} /></c::attachment::AttachmentMedia>}
    content={<c::attachment::AttachmentContent title={<c::attachment::AttachmentTitle args={"report.pdf"} />} />}
    actions={<c::attachment::AttachmentActions><c::button::Button args={"remove"} ghost label="Remove" /></c::attachment::AttachmentActions>} />
}

/// https://gpui-kit.com/component/avatar/
#[gpui]
pub fn avatar() -> impl IntoElement {
    <c::avatar::Avatar name="John Doe" src="https://example.com/avatar.jpg" />
}

/// https://gpui-kit.com/component/badge/
#[gpui]
pub fn badge() -> impl IntoElement {
    <c::badge::Badge count={3}><c::Icon args={c::IconName::Bell} /></c::badge::Badge>
}

/// https://gpui-kit.com/component/bubble/
#[gpui]
pub fn bubble() -> impl IntoElement {
    <c::bubble::Bubble alignment={c::message::MessageAlignment::Start}>Hello</c::bubble::Bubble>
}

/// https://gpui-kit.com/component/button/
#[gpui]
pub fn button() -> impl IntoElement {
    <c::button::Button args={"save"} primary small label="Save changes" on-click={|_, _, _| { println!("Saved"); }} />
}

/// https://gpui-kit.com/component/checkbox/
#[gpui]
pub fn checkbox() -> impl IntoElement {
    <c::checkbox::Checkbox args={"terms"} label="Accept terms" checked={false} on-change={|checked, _, _| { println!("{checked}"); }} />
}

/// https://gpui-kit.com/component/clipboard/
#[gpui]
pub fn clipboard() -> impl IntoElement {
    <c::clipboard::Clipboard args={"copy"} value="Text to copy" on-copied={|value, window, cx| window.push_notification(value.clone(), cx)} />
}

/// https://gpui-kit.com/component/collapsible/
#[gpui]
pub fn collapsible() -> impl IntoElement {
    <c::collapsible::Collapsible open={true} content={<div>Expanded content</div>}>Summary</c::collapsible::Collapsible>
}

/// https://gpui-kit.com/component/description-list/
#[gpui]
pub fn description_list() -> impl IntoElement {
    <c::description_list::DescriptionList item:args={("Name", "GPUI Kit", 1)} item:args={("License", "Apache-2.0", 1)} />
}

/// https://gpui-kit.com/component/dropdown_button/
#[gpui]
pub fn dropdown_button() -> impl IntoElement {
    <c::button::DropdownButton args={"dropdown"} button={<c::button::Button args={"trigger"} label="Options" />}
    dropdown-menu={|menu, _, _| <div ctor={menu} menu:args={("Copy", Box::new(Copy))} separator menu:args={("Paste", Box::new(Paste))} />} />
}

/// https://gpui-kit.com/component/empty/
#[gpui]
pub fn empty() -> impl IntoElement {
    <c::empty::Empty header={<c::empty::EmptyHeader title={<c::empty::EmptyTitle>No projects</c::empty::EmptyTitle>} />}
    content={<c::empty::EmptyContent><c::button::Button args={"create"} label="Create project" /></c::empty::EmptyContent>} />
}

/// https://gpui-kit.com/component/form/
#[gpui]
pub fn form() -> impl IntoElement {
    <c::form::Form columns={2} footer={<c::button::Button args={"submit"} label="Save" />}><c::form::Field label="Name"><div>Field content</div></c::form::Field></c::form::Form>
}

/// https://gpui-kit.com/component/group-box/
#[gpui]
pub fn group_box() -> impl IntoElement {
    <c::group_box::GroupBox fill title="Preferences" footer="Changes apply to this device."><c::switch::Switch args={"privacy"} checked={false} /></c::group_box::GroupBox>
}

/// https://gpui-kit.com/component/hover-card/
#[gpui]
pub fn hover_card() -> impl IntoElement {
    <c::hover_card::HoverCard args={"profile"} trigger={<div cursor-pointer>@user</div>}><c::avatar::Avatar name="User" /></c::hover_card::HoverCard>
}

/// https://gpui-kit.com/component/icon/
#[gpui]
pub fn icon() -> impl IntoElement {
    <c::Icon args={c::IconName::Heart} size-4 />
}

/// https://gpui-kit.com/component/image/
#[gpui]
pub fn image() -> impl IntoElement {
    <img args={"images/cover.png"} w={px(320.)} h={px(180.)} object-fit={ObjectFit::Cover} with-fallback={|| <div>Image unavailable</div>.into_any_element()} />
}

/// https://gpui-kit.com/component/kbd/
#[gpui]
pub fn kbd() -> impl IntoElement {
    <c::kbd::Kbd args={Keystroke::parse("cmd-shift-p").unwrap()} />
}

/// https://gpui-kit.com/component/label/
#[gpui]
pub fn label() -> impl IntoElement {
    <c::label::Label args={"This is a label"} />
}

/// https://gpui-kit.com/component/marker/
#[gpui]
pub fn marker() -> impl IntoElement {
    <c::marker::Marker icon={<c::marker::MarkerIcon><c::Icon args={c::IconName::Check} /></c::marker::MarkerIcon>} content={<c::marker::MarkerContent text="Online" />} />
}

/// https://gpui-kit.com/component/menu/
#[gpui]
pub fn menu() -> impl IntoElement {
    <div id="context-menu" context-menu={|menu, _, _| <div ctor={menu} menu:args={("Copy", Box::new(Copy))} menu:args={("Paste", Box::new(Paste))} separator />}>Right click</div>
}

/// https://gpui-kit.com/component/message/
#[gpui]
pub fn message() -> impl IntoElement {
    <c::message::Message content={<c::message::MessageContent bubble={<c::bubble::Bubble>Hello</c::bubble::Bubble>} />} />
}

/// https://gpui-kit.com/component/notification/
#[gpui]
pub fn notification() -> c::notification::Notification {
    <c::notification::Notification message="Changes saved" autohide={true} />
}

/// https://gpui-kit.com/component/pagination/
#[gpui]
pub fn pagination() -> impl IntoElement {
    <c::pagination::Pagination args={"pages"} current-page={5} total-pages={10} on-click={|page, _, _| println!("Page {page}")} />
}

/// https://gpui-kit.com/component/popover/
#[gpui]
pub fn popover() -> impl IntoElement {
    <c::popover::Popover args={"popover"} trigger={<c::button::Button args={"open"} label="Open" outline />}>Popover content</c::popover::Popover>
}

/// https://gpui-kit.com/component/progress/
#[gpui]
pub fn progress() -> impl IntoElement {
    <c::progress::Progress args={"progress"} value={75.0} />
}

/// https://gpui-kit.com/component/radio/
#[gpui]
pub fn radio() -> impl IntoElement {
    <c::radio::RadioGroup args={"choice"}><c::radio::Radio args={"one"} label="One" /><c::radio::Radio args={"two"} label="Two" /></c::radio::RadioGroup>
}

/// https://gpui-kit.com/component/rating/
#[gpui]
pub fn rating() -> impl IntoElement {
    <c::rating::Rating args={"rating"} value={3} max={5} on-click={|value, _, _| println!("Rating {value}")} />
}

/// https://gpui-kit.com/component/scrollable/
#[gpui]
pub fn scrollable() -> impl IntoElement {
    <div id="scroll" size-full overflow-scrollbar>Scrollable content</div>
}

/// https://gpui-kit.com/component/shimmer/
#[gpui]
pub fn shimmer() -> impl IntoElement {
    <c::shimmer::ShimmerText args={"Thinking…"} />
}

/// https://gpui-kit.com/component/sidebar/
#[gpui]
pub fn sidebar() -> impl IntoElement {
    <c::sidebar::Sidebar args={"sidebar"} header={<c::sidebar::SidebarHeader>Application</c::sidebar::SidebarHeader>}
    footer={<c::sidebar::SidebarFooter>Profile</c::sidebar::SidebarFooter>}>
    <c::sidebar::SidebarGroup args={"Navigation"}><c::sidebar::SidebarMenu>
        <c::sidebar::SidebarMenuItem args={"Dashboard"} icon={c::IconName::LayoutDashboard} on-click={|_, _, _| println!("Dashboard")} />
    </c::sidebar::SidebarMenu></c::sidebar::SidebarGroup>
</c::sidebar::Sidebar>
}

/// https://gpui-kit.com/component/skeleton/
#[gpui]
pub fn skeleton() -> impl IntoElement {
    <c::skeleton::Skeleton />
}

/// https://gpui-kit.com/component/spinner/
#[gpui]
pub fn spinner() -> impl IntoElement {
    <c::spinner::Spinner />
}

/// https://gpui-kit.com/component/status-bar/
#[gpui]
pub fn status_bar() -> impl IntoElement {
    <c::status_bar::StatusBar left={"Ready"} right={"UTF-8"}>README.md</c::status_bar::StatusBar>
}

/// https://gpui-kit.com/component/stepper/
#[gpui]
pub fn stepper() -> impl IntoElement {
    <c::stepper::Stepper args={"steps"} selected-index={0} items={[<c::stepper::StepperItem>Step 1</c::stepper::StepperItem>, <c::stepper::StepperItem>Step 2</c::stepper::StepperItem>]} />
}

/// https://gpui-kit.com/component/switch/
#[gpui]
pub fn switch() -> impl IntoElement {
    <c::switch::Switch args={"enabled"} checked={false} on-change={|checked, _, _| println!("Enabled {checked}")} />
}

/// https://gpui-kit.com/component/table/
#[gpui]
pub fn table() -> impl IntoElement {
    <c::table::Table><c::table::TableHeader><c::table::TableRow><c::table::TableHead>Name</c::table::TableHead></c::table::TableRow></c::table::TableHeader>
    <c::table::TableBody><c::table::TableRow><c::table::TableCell>John</c::table::TableCell></c::table::TableRow></c::table::TableBody>
    <c::table::TableCaption>Invoices</c::table::TableCaption></c::table::Table>
}

/// https://gpui-kit.com/component/tabs/
#[gpui]
pub fn tabs() -> impl IntoElement {
    <c::tab::TabBar args={"tabs"} selected-index={0}><c::tab::Tab label="Account" /><c::tab::Tab label="Settings" /></c::tab::TabBar>
}

/// https://gpui-kit.com/component/tag/
#[gpui]
pub fn tag() -> impl IntoElement {
    <c::tag::Tag::primary>Primary</c::tag::Tag::primary>
}

/// https://gpui-kit.com/component/text-view/
#[gpui]
pub fn text_view() -> impl IntoElement {
    <c::text::markdown args={"# Hello\n\nThis is **Markdown**."} scrollable={true} />
}

/// https://gpui-kit.com/component/title-bar/
#[gpui]
pub fn title_bar() -> impl IntoElement {
    <c::TitleBar><div>My application</div></c::TitleBar>
}

/// https://gpui-kit.com/component/toggle/
#[gpui]
pub fn toggle() -> impl IntoElement {
    <c::button::ToggleGroup args={"toggle-group"}><c::button::Toggle args={"one"} label="One" checked={false} /></c::button::ToggleGroup>
}

/// https://gpui-kit.com/component/toolbar/
#[gpui]
pub fn toolbar() -> impl IntoElement {
    <c::toolbar::Toolbar args={"toolbar"} content={<c::separator::Separator::vertical />}><c::button::Button args={"new"} label="New" /></c::toolbar::Toolbar>
}

/// https://gpui-kit.com/component/tooltip/
#[gpui]
pub fn tooltip() -> impl IntoElement {
    <div id="tooltip" tooltip={|window, cx| <c::tooltip::Tooltip args={"Helpful hint"} build:args={(window, cx)} />}>Hover me</div>
}

/// https://gpui-kit.com/component/alert-dialog/
#[gpui]
pub fn alert_dialog(cx: &mut App) -> impl IntoElement {
    <c::dialog::AlertDialog args={cx}
        trigger={<c::button::Button args={"confirm"} label="Confirm" outline />}
        content={|content, _, _| <div ctor={content}>
            <c::dialog::DialogHeader><c::dialog::DialogTitle>Are you sure?</c::dialog::DialogTitle></c::dialog::DialogHeader>
            <c::dialog::DialogFooter><c::dialog::DialogClose><c::button::Button args={"cancel"} label="Cancel" /></c::dialog::DialogClose></c::dialog::DialogFooter>
        </div>} />
}

/// https://gpui-kit.com/component/dialog/
#[gpui]
pub fn dialog(cx: &mut App) -> impl IntoElement {
    <c::dialog::Dialog args={cx}
        trigger={<c::button::Button args={"dialog-trigger"} label="Open dialog" />}
        content={|content, _, _| <div ctor={content}>
            <c::dialog::DialogHeader><c::dialog::DialogTitle>Welcome</c::dialog::DialogTitle></c::dialog::DialogHeader>
            <c::dialog::DialogDescription>Dialog content</c::dialog::DialogDescription>
        </div>} />
}

/// https://gpui-kit.com/component/calendar/
#[gpui]
pub fn calendar(state: &Entity<c::calendar::CalendarState>) -> impl IntoElement {
    <c::calendar::Calendar args={state} />
}

/// https://gpui-kit.com/component/carousel/
#[gpui]
pub fn carousel(state: &Entity<c::carousel::CarouselState>) -> impl IntoElement {
    <c::carousel::Carousel args={("carousel", state)}>
        <c::carousel::CarouselContent args={state}>
            <c::carousel::CarouselItem args={("item", 0, state)}>Project</c::carousel::CarouselItem>
        </c::carousel::CarouselContent>
        <c::carousel::CarouselPrevious args={state} />
        <c::carousel::CarouselNext args={state} />
    </c::carousel::Carousel>
}

#[derive(Clone)]
pub struct DataPoint { pub x: String, pub y: f64 }

/// https://gpui-kit.com/component/chart/
#[gpui]
pub fn chart(data: Vec<DataPoint>) -> impl IntoElement {
    <c::chart::LineChart args={data} x={|d| d.x.clone()} y={|d| d.y} />
}

/// https://gpui-kit.com/component/color-picker/
#[gpui]
pub fn color_picker(state: &Entity<c::color_picker::ColorPickerState>) -> impl IntoElement {
    <c::color_picker::ColorPicker args={state} />
}

/// https://gpui-kit.com/component/combobox/
#[gpui]
pub fn combobox(state: &Entity<c::combobox::ComboboxState<c::searchable_list::SearchableVec<&'static str>>>) -> impl IntoElement {
    <c::combobox::Combobox args={state} placeholder="Select framework…" search-placeholder="Search…" w-full />
}

/// https://gpui-kit.com/component/command/
#[gpui]
pub fn command(state: &Entity<c::command::CommandState>) -> impl IntoElement {
    <c::command::Command args={state} placeholder="Search commands…"
        group={<c::command::CommandGroup label="Suggestions"
            item={<c::command::CommandItem label="Calendar" icon={c::IconName::Calendar} />}
            item={<c::command::CommandItem label="Copy" action={Box::new(Copy)} />} />}
        on-confirm={|index, window, cx| window.push_notification(format!("Selected {index}"), cx)} />
}

/// https://gpui-kit.com/component/data-table/
#[gpui]
pub fn data_table<D: c::table::TableDelegate + 'static>(state: &Entity<c::table::TableState<D>>) -> impl IntoElement {
    <c::table::DataTable args={state} />
}

/// https://gpui-kit.com/component/date-picker/
#[gpui]
pub fn date_picker(state: &Entity<c::date_picker::DatePickerState>) -> impl IntoElement {
    <c::date_picker::DatePicker args={state} />
}

/// https://gpui-kit.com/component/dock/
#[gpui]
pub fn dock(left: c::dock::DockLayout, right: c::dock::DockLayout) -> c::dock::DockLayout {
    <c::dock::DockLayout::h_split child:args={(left, Some(px(240.)))} child:args={(right, None)} />
}

/// https://gpui-kit.com/component/editor/
#[gpui]
pub fn editor(state: &Entity<c::input::EditorState>) -> impl IntoElement {
    <c::input::Editor args={state} />
}

/// https://gpui-kit.com/component/focus-trap/
#[gpui]
pub fn focus_trap(handle: &FocusHandle) -> impl IntoElement {
    <div child={<c::button::Button args={"first"} label="First" />}
        child={<c::button::Button args={"second"} label="Second" />}
        focus-trap:args={("trap", handle)} />
}

/// https://gpui-kit.com/component/input-group/
#[gpui]
pub fn input_group(state: &Entity<c::input::InputState>) -> impl IntoElement {
    <c::input::InputGroup args={"website"}
        input={<c::input::InputGroupInput args={state} aria-label="Website" />}
        addon={<c::input::InputGroupAddon args={"protocol"}><c::input::InputGroupText>https://</c::input::InputGroupText></c::input::InputGroupAddon>}
        addon={<c::input::InputGroupAddon args={"action"}><c::input::InputGroupButton args={"go"} label="Go" /></c::input::InputGroupAddon>} />
}

/// https://gpui-kit.com/component/input/
#[gpui]
pub fn input(state: &Entity<c::input::InputState>) -> impl IntoElement {
    <c::input::Input args={state} />
}

/// https://gpui-kit.com/component/list/
#[gpui]
pub fn list<D: c::list::ListDelegate + 'static>(state: &Entity<c::list::ListState<D>>) -> impl IntoElement {
    <c::list::List args={state} />
}

/// https://gpui-kit.com/component/message-scroller/
#[gpui]
pub fn message_scroller(state: Entity<c::message_scroller::MessageScrollerState>) -> impl IntoElement {
    <c::message_scroller::MessageScroller args={("conversation", state, |index, _, _| <div id={("message", index)}>Message {index.to_string()}</div>)} w-full h-96 />
}

/// https://gpui-kit.com/component/number-input/
#[gpui]
pub fn number_input(state: &Entity<c::input::InputState>) -> impl IntoElement {
    <c::input::NumberInput args={state} />
}

/// https://gpui-kit.com/component/otp-input/
#[gpui]
pub fn otp_input(state: &Entity<c::input::OtpState>) -> impl IntoElement {
    <c::input::OtpInput args={state} />
}

/// https://gpui-kit.com/component/plot/
#[gpui]
pub fn plot() -> c::plot::scale::ScaleLinear<f64> {
    <c::plot::scale::ScaleLinear args={(vec![0., 100.], [0., 500.])} />
}

/// https://gpui-kit.com/component/questionnaire/
#[gpui]
pub fn questionnaire(state: &Entity<c::questionnaire::QuestionnaireState>) -> impl IntoElement {
    <c::questionnaire::Questionnaire args={state}>
        <c::questionnaire::QuestionnaireProgress args={state} />
        <c::questionnaire::QuestionnaireItem args={(state, "direction")}>
            <c::questionnaire::QuestionnaireTitle args={(state, "direction")} />
            <c::questionnaire::QuestionnaireDescription args={(state, "direction")} />
            <c::questionnaire::QuestionnaireChoices args={(state, "direction")}>
                <c::questionnaire::QuestionnaireChoice args={(state, "direction", "focused")} />
            </c::questionnaire::QuestionnaireChoices>
            <c::questionnaire::QuestionnaireError args={(state, "direction")} />
        </c::questionnaire::QuestionnaireItem>
        <c::questionnaire::QuestionnaireActions args={state}>
            <c::questionnaire::QuestionnairePrevious args={state} />
            <c::questionnaire::QuestionnaireSkip args={state} />
            <c::questionnaire::QuestionnaireNext args={state} />
            <c::questionnaire::QuestionnaireSubmit args={state} />
        </c::questionnaire::QuestionnaireActions>
    </c::questionnaire::Questionnaire>
}

/// https://gpui-kit.com/component/resizable/
#[gpui]
pub fn resizable() -> impl IntoElement {
    <c::resizable::h_resizable args={"layout"}>
        <c::resizable::resizable_panel size={px(200.)}>Left</c::resizable::resizable_panel>
        <c::resizable::resizable_panel>Right</c::resizable::resizable_panel>
    </c::resizable::h_resizable>
}

/// https://gpui-kit.com/component/root/
#[gpui]
pub fn root<V: Render>(view: Entity<V>, window: &mut Window, cx: &mut Context<gpui_kit::base::Root>) -> gpui_kit::base::Root {
    <gpui_kit::base::Root args={(view, window, cx)} />
}

/// https://gpui-kit.com/component/select/
#[gpui]
pub fn select(state: &Entity<c::select::SelectState<Vec<&'static str>>>) -> impl IntoElement {
    <c::select::Select args={state} placeholder="Select language…" small />
}

/// https://gpui-kit.com/component/settings/
#[gpui]
pub fn settings() -> impl IntoElement {
    <c::setting::Settings args={"settings"} pages={vec![
        <c::setting::SettingPage args={"General"} group={
            <c::setting::SettingGroup title="Basic options" item={
                <c::setting::SettingItem args={("Enable feature", <c::setting::SettingField::switch args={(|_: &App| true, |_: bool, _: &mut App| {})} />)} />
            } />
        } />
    ]} />
}

/// https://gpui-kit.com/component/sheet/
#[gpui]
pub fn sheet(window: &mut Window, cx: &mut App) {
    window.open_sheet(cx, |sheet, _, _| <div ctor={sheet} title="Navigation">Sheet content</div>);
}

/// https://gpui-kit.com/component/slider/
#[gpui]
pub fn slider(state: &Entity<c::slider::SliderState>) -> impl IntoElement {
    <c::slider::Slider args={state} />
}

/// https://gpui-kit.com/component/textarea/
#[gpui]
pub fn textarea(state: &Entity<c::input::TextareaState>) -> impl IntoElement {
    <c::input::Textarea args={state} />
}

/// https://gpui-kit.com/component/theme/
#[gpui]
pub fn theme(cx: &App) -> impl IntoElement {
    <div bg={cx.theme().background} text-color={cx.theme().foreground}>Themed content</div>
}

/// https://gpui-kit.com/component/time-field/
#[gpui]
pub fn time_field(state: &Entity<c::time_field::TimeFieldState>) -> impl IntoElement {
    <c::time_field::TimeField args={state} />
}

/// https://gpui-kit.com/component/tree/
#[gpui]
pub fn tree(state: &Entity<c::tree::TreeState>) -> impl IntoElement {
    <c::tree::tree args={(state, |ix, entry, selected, _, _| <c::list::ListItem args={ix} selected={selected}>{entry.item().label.clone()}</c::list::ListItem>)} />
}

pub struct VirtualListView;
impl Render for VirtualListView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement { div() }
}

/// https://gpui-kit.com/component/virtual-list/
#[gpui]
pub fn virtual_list(view: Entity<VirtualListView>, sizes: std::rc::Rc<Vec<Size<Pixels>>>) -> impl IntoElement {
    <c::v_virtual_list args={(view, "items", sizes, |_, range, _, _| range.map(|ix| <div h={px(30.)} w-full>{format!("Item {ix}")}</div>.into_any_element()).collect())} />
}
