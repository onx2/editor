use bevy::prelude::*;
use bevy_egui::EguiPlugin;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(EguiPlugin::default());
}
