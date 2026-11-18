//! Shared Vizia / `femtovg` UI kit, design tokens, themes, scopes, FX Unit views,
//! and headless PNG preview renderer for the `J-SYN` plugin ecosystem.

pub mod controls;
pub mod fx_units;
pub mod preview;
pub mod scopes;
pub mod shell;
pub mod theme;
pub mod tokens;
pub mod widgets;

pub use shell::{
    build_tooltip_overlay, notify_baseview_resize_macos, register_fonts, ModUiHost, NoopGuiContext,
    TooltipState, UiKitEvent, UiShellState,
};
pub use theme::ThemePalette;
