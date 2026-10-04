//! The window shell's widgets, registered in dependency order. Each file
//! registers its widgets under `mod.widgets.Reco*`.

mod controls;
mod inspector;
mod media_panel;
mod shell;
mod status_bar;
mod top_bar;
mod transport;
mod viewer;

use makepad_widgets::*;

/// Register every shell widget. Call after `makepad_widgets::widgets_mod`
/// and before the app's own script module.
pub fn script_mod(vm: &mut ScriptVm) {
    controls::script_mod(vm);
    top_bar::script_mod(vm);
    media_panel::script_mod(vm);
    viewer::script_mod(vm);
    inspector::script_mod(vm);
    transport::script_mod(vm);
    status_bar::script_mod(vm);
    shell::script_mod(vm);
}
