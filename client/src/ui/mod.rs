mod app_bar;
mod performance;

use bevy::{
    camera::{CameraOutputMode, visibility::RenderLayers},
    prelude::*,
    render::render_resource::BlendState,
};
use bevy_egui::{EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext};

pub fn plugin(app: &mut App) {
    // Register egui once, from a central place.
    // All performance-related UI is owned by `ui/performance`.
    app.add_plugins((EguiPlugin::default(), performance::plugin));
    app.add_systems(Startup, setup);

    // Render panels in the egui pass schedule so the pass state is initialized.
    app.add_systems(EguiPrimaryContextPass, app_bar::render);
}

fn setup(mut commands: Commands, mut egui_global_settings: ResMut<EguiGlobalSettings>) {
    // Disable the automatic creation of a primary context to set it up manually for the camera we need.
    egui_global_settings.auto_create_primary_context = false;
    // Egui camera.
    commands.spawn((
        // The `PrimaryEguiContext` component requires everything needed to render a primary context.
        PrimaryEguiContext,
        Camera2d,
        // Setting RenderLayers to none makes sure we won't render anything apart from the UI.
        RenderLayers::none(),
        Camera {
            order: 1,
            output_mode: CameraOutputMode::Write {
                blend_state: Some(BlendState::ALPHA_BLENDING),
                clear_color: ClearColorConfig::None,
            },
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
    ));
}
