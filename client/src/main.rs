mod flycam;
mod infinite_grid;
mod ui;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            ui::plugin,
            infinite_grid::plugin,
            flycam::plugin,
        ))
        .run();
}
