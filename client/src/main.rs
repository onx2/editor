mod flycam;
mod fps_overlay;
mod infinite_grid;
mod window;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            window::plugin,
            infinite_grid::plugin,
            flycam::plugin,
            fps_overlay::plugin,
        ))
        .run();
}
