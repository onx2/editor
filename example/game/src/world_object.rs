use crate::{
    module_bindings::{Player, WorldObject},
    spacetime::SpacetimeDB,
};
use bevy::prelude::*;
use bevy_spacetimedb::ReadInsertMessage;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, setup);
    app.add_systems(Update, on_insert);
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    println!("Setting up world object");
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::new(0.0, 25.0, -10.0)).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    commands.spawn((
        Transform::from_xyz(1.0, 1.0, 1.0),
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(1., 1., 1.)))),
        MeshMaterial3d(materials.add(StandardMaterial::default())),
    ));
}

fn on_insert(
    mut commands: Commands,
    mut inserted: ReadInsertMessage<WorldObject>,
    asset_server: Res<AssetServer>,
    stdb: SpacetimeDB,
) {
    for msg in inserted.read() {
        let row = msg.row.clone();
        println!("world_object inserted with ID: {}", row.id);

        // let translation = Vec3::new(row.translation.x, row.translation.y, row.translation.z);
        // let rotation = Quat::from_xyzw(
        //     row.rotation.x,
        //     row.rotation.y,
        //     row.rotation.z,
        //     row.rotation.w,
        // );
        // let scale = Vec3::new(row.scale.x, row.scale.y, row.scale.z);

        // let transform = Transform {
        //     translation,
        //     rotation,
        //     scale,
        // };

        // // If an asset path is provided, try to load it as a glTF scene (common for .gltf/.glb).
        // // The stored path should be relative to Bevy's asset root, e.g. "FlightHelmet.gltf".
        // if let Some(asset_path) = row.asset_path.clone().filter(|p| !p.trim().is_empty()) {
        //     // For glTF files, a good default is loading the default scene.
        //     // Bevy supports the "#Scene0" suffix for glTF scenes.
        //     let scene_handle: Handle<Scene> = asset_server.load(format!("{asset_path}#Scene0"));

        //     commands.spawn((
        //         SceneRoot(scene_handle),
        //         transform,
        //         Name::new(format!("WorldObject({})", row.id)),
        //     ));

        //     println!("spawned world object {} from asset {}", row.id, asset_path);
        // }
        continue;
    }
}
