mod flycam;
mod fps_overlay;
mod infinite_grid;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            infinite_grid::plugin,
            flycam::plugin,
            fps_overlay::plugin,
        ))
        .run();
}
