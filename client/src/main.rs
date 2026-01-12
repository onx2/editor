mod flycam;
mod infinite_grid;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, infinite_grid::plugin, flycam::plugin))
        .run();
}
