mod config;
mod flycam;
mod infinite_grid;
mod module_bindings;
mod spacetimedb;
mod ui;
mod world_object;

use bevy::asset::AssetPlugin;
use bevy::color::palettes::css::ALICE_BLUE;
use bevy::prelude::*;

use crate::config::ClientRuntimeConfig;

fn main() {
    // To make `.env` work reliably, load it from the client crate directory.
    let client_crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dotenv_path = client_crate_dir.join(".env");
    let _ = dotenvy::from_path(dotenv_path);

    let config = ClientRuntimeConfig::from_env();

    let default_plugins = DefaultPlugins.build().set(AssetPlugin {
        file_path: config.asset_root_for_bevy(),
        ..default()
    });

    let mut app = App::new();
    app.insert_resource(config);
    app.add_plugins((
        spacetimedb::plugin,
        world_object::plugin,
        default_plugins,
        MeshPickingPlugin,
        ui::plugin,
        infinite_grid::plugin,
        flycam::plugin,
    ));
    app.add_systems(Startup, setup_sun);
    app.insert_resource(AmbientLight {
        color: ALICE_BLUE.into(),
        brightness: 2_000.,
        ..AmbientLight::default()
    });
    app.run();
}

fn setup_sun(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 80_000.0,
            shadows_enabled: true,
            ..default()
        },
        // Orientation: Looking down from the sky
        Transform::from_xyz(0.0, 10.0, 0.0).looking_at(Vec3::new(1.0, -1.0, 1.0), Vec3::Y),
    ));
}
