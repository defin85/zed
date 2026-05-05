use gpui::Pixels;
use settings::{RegisterSetting, Settings};

#[derive(Debug, Clone, Copy, PartialEq, RegisterSetting)]
pub struct SymbolBrowserSettings {
    pub button: bool,
    pub default_width: Pixels,
    pub dock: settings::DockSide,
}

impl Settings for SymbolBrowserSettings {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        let panel = content.symbol_browser.as_ref().unwrap();
        Self {
            button: panel.button.unwrap(),
            default_width: panel.default_width.map(gpui::px).unwrap(),
            dock: panel.dock.unwrap(),
        }
    }
}
