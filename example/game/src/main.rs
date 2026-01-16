mod module_bindings;
mod spacetime;
mod world_object;
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, spacetime::plugin, world_object::plugin))
        .run();
}
