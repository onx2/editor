mod config;
mod flycam;
mod infinite_grid;
mod module_bindings;
mod spacetimedb;
mod ui;

use bevy::asset::AssetPlugin;
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

    App::new()
        .insert_resource(config)
        .add_plugins((
            spacetimedb::plugin,
            default_plugins,
            ui::plugin,
            infinite_grid::plugin,
            flycam::plugin,
        ))
        .run();
}
