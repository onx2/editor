mod flycam;
mod infinite_grid;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, infinite_grid::plugin, flycam::plugin))
        .add_systems(Startup, spawn_infinite_grid)
        .run();
}

fn spawn_infinite_grid(mut commands: Commands) {
    commands.spawn(infinite_grid::InfiniteGridBundle::default());
}
