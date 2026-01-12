mod floor_wireframe;
mod flycam;

use bevy::prelude::*;

fn main() {
    App::new()
        // Make it obvious the camera is rendering even if the grid is faint.
        // .insert_resource(ClearColor(Color::srgb(0.08, 0.09, 0.11)))
        // Only run the floor grid shader + the camera controller.
        .add_plugins((DefaultPlugins, floor_wireframe::plugin, flycam::plugin))
        .run();
}
