mod symbol_browser_settings;

use gpui::{
    actions, App, AsyncWindowContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    ParentElement, Pixels, Render, Subscription, Task, WeakEntity, Window,
};
use project::Project;
use settings::Settings;
use std::collections::BTreeMap;
use ui::{prelude::*, IconName};
use util::ResultExt;
use workspace::{
    dock::{DockPosition, Panel, PanelEvent},
    Workspace,
};

use self::symbol_browser_settings::SymbolBrowserSettings;

actions!(
    symbol_browser,
    [
        Toggle,
        ToggleFocus,
    ]
);

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, _, _| {
        workspace.register_action(|workspace, _: &ToggleFocus, window, cx| {
            workspace.toggle_panel_focus::<SymbolBrowserPanel>(window, cx);
        });
        workspace.register_action(|workspace, _: &Toggle, window, cx| {
            if !workspace.toggle_panel_focus::<SymbolBrowserPanel>(window, cx) {
                workspace.close_panel::<SymbolBrowserPanel>(window, cx);
            }
        });
    })
    .detach();
}

#[derive(Debug, Clone)]
struct SymbolEntry {
    name: String,
    #[allow(dead_code)]
    kind: lsp::SymbolKind,
}

#[derive(Debug, Clone)]
struct SymbolGroup {
    kind_name: String,
    entries: Vec<SymbolEntry>,
}

fn kind_name(kind: lsp::SymbolKind) -> &'static str {
    use lsp::SymbolKind;
    match kind {
        SymbolKind::FUNCTION => "Functions",
        SymbolKind::METHOD => "Methods",
        SymbolKind::CLASS => "Classes",
        SymbolKind::STRUCT => "Structs",
        SymbolKind::INTERFACE => "Interfaces",
        SymbolKind::ENUM => "Enums",
        SymbolKind::CONSTANT => "Constants",
        SymbolKind::VARIABLE => "Variables",
        SymbolKind::MODULE => "Modules",
        SymbolKind::NAMESPACE => "Namespaces",
        SymbolKind::PROPERTY => "Properties",
        SymbolKind::CONSTRUCTOR => "Constructors",
        SymbolKind::TYPE_PARAMETER => "Type Parameters",
        _ => "Other",
    }
}

fn group_symbols(symbols: Vec<project::Symbol>) -> Vec<SymbolGroup> {
    let mut map: BTreeMap<String, Vec<SymbolEntry>> = BTreeMap::new();
    for sym in symbols {
        let key = kind_name(sym.kind).to_string();
        map.entry(key).or_default().push(SymbolEntry {
            name: sym.name,
            kind: sym.kind,
        });
    }
    let mut groups: Vec<SymbolGroup> = map
        .into_iter()
        .map(|(kind_name, entries)| SymbolGroup { kind_name, entries })
        .collect();
    groups.sort_by(|a, b| a.kind_name.cmp(&b.kind_name));
    groups
}

pub struct SymbolBrowserPanel {
    #[allow(dead_code)] workspace: WeakEntity<Workspace>,
    #[allow(dead_code)] project: Entity<Project>,
    focus_handle: FocusHandle,
    active: bool,
    groups: Vec<SymbolGroup>,
    _fetch_task: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<PanelEvent> for SymbolBrowserPanel {}

impl SymbolBrowserPanel {
    pub async fn load(
        workspace: WeakEntity<Workspace>,
        mut cx: AsyncWindowContext,
    ) -> anyhow::Result<Entity<Self>> {
        workspace.update_in(&mut cx, |workspace, window, cx| {
            let panel = Self::new(workspace, window, cx);
            panel.update(cx, |_, cx| cx.notify());
            panel
        })
    }

    fn new(
        workspace: &mut Workspace,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Entity<Self> {
        let project = workspace.project().clone();
        let workspace_handle = cx.entity().downgrade();
        cx.new(|cx| {
            let focus_handle = cx.focus_handle();
            let mut this = Self {
                workspace: workspace_handle,
                project: project.clone(),
                focus_handle,
                active: false,
                groups: Vec::new(),
                _fetch_task: Task::ready(()),
                _subscriptions: vec![],
            };
            this.fetch_symbols(project, window, cx);
            this
        })
    }

    fn fetch_symbols(&mut self, project: Entity<Project>, window: &mut Window, cx: &mut Context<Self>) {
        let symbols_task = project.update(cx, |project, cx| project.symbols("", cx));
        self._fetch_task = cx.spawn_in(window, async move |this, cx| {
            let symbols = symbols_task.await.log_err().unwrap_or_default();
            this.update_in(cx, |this, _window, cx| {
                this.groups = group_symbols(symbols);
                cx.notify();
            }).ok();
        });
    }
}

const SYMBOL_BROWSER_KEY: &str = "SymbolBrowserPanel";

impl Panel for SymbolBrowserPanel {
    fn persistent_name() -> &'static str { "Symbol Browser" }
    fn panel_key() -> &'static str { SYMBOL_BROWSER_KEY }
    fn position(&self, _window: &Window, cx: &App) -> DockPosition {
        match SymbolBrowserSettings::get_global(cx).dock {
            settings::DockSide::Left => DockPosition::Left,
            settings::DockSide::Right => DockPosition::Right,
        }
    }
    fn position_is_valid(&self, position: DockPosition) -> bool {
        matches!(position, DockPosition::Left | DockPosition::Right)
    }
    fn set_position(&mut self, _pos: DockPosition, _w: &mut Window, _cx: &mut Context<Self>) {}
    fn default_size(&self, _w: &Window, cx: &App) -> Pixels { SymbolBrowserSettings::get_global(cx).default_width }
    fn icon(&self, _w: &Window, cx: &App) -> Option<IconName> { SymbolBrowserSettings::get_global(cx).button.then_some(IconName::Code) }
    fn icon_tooltip(&self, _w: &Window, _cx: &App) -> Option<&'static str> { Some("Symbol Browser") }
    fn toggle_action(&self) -> Box<dyn gpui::Action> { Box::new(ToggleFocus) }
    fn starts_open(&self, _w: &Window, _cx: &App) -> bool { self.active }
    fn set_active(&mut self, active: bool, _w: &mut Window, _cx: &mut Context<Self>) { self.active = active; _cx.notify(); }
    fn activation_priority(&self) -> u32 { 0 }
}

impl Focusable for SymbolBrowserPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for SymbolBrowserPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.groups.is_empty() {
            return v_flex()
                .size_full()
                .bg(cx.theme().colors().panel_background)
                .child(div().flex().flex_1().justify_center().items_center()
                    .text_color(cx.theme().colors().text_muted)
                    .child(Label::new("Loading symbols...")))
                .into_any_element();
        }
        v_flex()
            .size_full()
            .bg(cx.theme().colors().panel_background)
            .child(div().flex().items_center().px_2().py_1().border_b_1()
                .border_color(cx.theme().colors().border)
                .child(Label::new(format!("{} groups", self.groups.len()))))
            .child(div().flex_1().children(self.groups.iter().map(|group| {
                v_flex()
                    .child(div().px_2().py_1().child(Label::new(format!("{} ({})", group.kind_name, group.entries.len()))))
                    .children(group.entries.iter().map(|entry| {
                        div().pl_4().pr_2().py_0p5().child(Label::new(&entry.name))
                    })).into_any_element()
            }))).into_any_element()
    }
}
