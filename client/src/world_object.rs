use crate::{
    module_bindings::{
        AssetKind, CollisionShape, WorldObject, insert_object, move_object, rotate_object,
    },
    spacetimedb::SpacetimeDB,
};
use bevy::prelude::*;
use bevy_spacetimedb::ReadInsertMessage;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, (on_insert, spawn_alien_on_key0));
}

#[derive(Component)]
pub struct ObjectId(pub u64);

fn on_insert(
    mut commands: Commands,
    mut inserted: ReadInsertMessage<WorldObject>,
    asset_server: Res<AssetServer>,
) {
    for msg in inserted.read() {
        let row = msg.row.clone();
        println!("Inserting WorldObject with ID: {}", row.id);

        let translation = Vec3::new(row.translation.x, row.translation.y, row.translation.z);
        let rotation = Quat::from_xyzw(
            row.rotation.x,
            row.rotation.y,
            row.rotation.z,
            row.rotation.w,
        );
        let scale = Vec3::new(row.scale.x, row.scale.y, row.scale.z);

        let transform = Transform {
            translation,
            rotation,
            scale,
        };

        // If an asset path is provided, try to load it as a glTF scene (common for .gltf/.glb).
        // The stored path should be relative to Bevy's asset root, e.g. "FlightHelmet.gltf".
        match row.asset {
            AssetKind::Path(path) => {
                // For glTF files, a good default is loading the default scene.
                // Bevy supports the "#Scene0" suffix for glTF scenes.
                let scene_handle: Handle<Scene> = asset_server.load(format!("{path}#Scene0"));

                commands
                    .spawn((SceneRoot(scene_handle), transform, ObjectId(row.id)))
                    .observe(on_drag_rotate)
                    .observe(on_drag_end);
            }
            _ => {
                todo!("implement primitive shapes")
            }
        }

        continue;
    }
}

fn spawn_alien_on_key0(keys: Res<ButtonInput<KeyCode>>, stdb: SpacetimeDB) {
    if !keys.just_pressed(KeyCode::Digit0) {
        return;
    }

    // Note: this assumes your server-side reducer/table will accept client-provided IDs.
    // If the server assigns IDs, change `id` to whatever convention your module expects.
    let object = WorldObject {
        id: 0,
        asset: AssetKind::Path("alien.glb".to_string()),
        translation: crate::module_bindings::Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        rotation: crate::module_bindings::Quat {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        },
        scale: crate::module_bindings::Vec3 {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        },
        // Start simple: no collision.
        collision_shape: CollisionShape::None,
    };

    let _ = stdb.reducers().insert_object(object);
}

fn on_drag_rotate(drag: On<Pointer<Drag>>, mut objects: Query<(&mut Transform, &ObjectId)>) {
    let Ok((mut transform, _id)) = objects.get_mut(drag.entity) else {
        return;
    };

    // Provided by your drag event
    let delta: Vec2 = drag.delta;

    // Tune to taste: radians per pixel.
    // Start around 0.01–0.02.
    let sensitivity = 0.01;

    // Turntable:
    // - horizontal drag => yaw about global up
    // - vertical drag => pitch about object's local right
    //
    // NOTE: If your world is Z-up (like Unreal), change Vec3::Y to Vec3::Z.
    let yaw = -delta.x * sensitivity;
    let pitch = -delta.y * sensitivity;

    let q_yaw = Quat::from_axis_angle(Vec3::Y, yaw);
    let q_pitch = Quat::from_axis_angle(transform.right().into(), pitch);

    // Apply incremental rotation
    transform.rotation = (q_yaw * q_pitch) * transform.rotation;
}

fn on_drag_end(
    drag: On<Pointer<DragEnd>>,
    objects: Query<(&Transform, &ObjectId)>,
    stdb: SpacetimeDB,
) {
    if let Ok((transform, id)) = objects.get(drag.entity) {
        let _ = stdb
            .reducers()
            .rotate_object(id.0, transform.rotation.into());
        let _ = stdb
            .reducers()
            .move_object(id.0, transform.translation.into());
    }
}
