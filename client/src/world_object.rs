use crate::{
    flycam::FlyCamActive,
    module_bindings::{
        AssetKind, CollisionShape, WorldObject, insert_object, move_object, rotate_object,
        scale_object,
    },
    spacetimedb::SpacetimeDB,
    ui::transform_tools::{TransformTool, TransformToolMode},
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
                    .observe(on_drag_start)
                    .observe(on_drag_transform)
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

fn on_drag_start(
    drag: On<Pointer<DragStart>>,
    tool: ResMut<TransformTool>,
    flycam_active: Res<FlyCamActive>,
) {
    // Never begin a transform interaction while flycam is active.
    if flycam_active.0 {
        return;
    }

    // Lock tool switching for the duration of the drag gesture.
    // We don't allow changing selected tool while active, so `selected_tool` is effectively the locked tool.
    let mut tool = tool;
    tool.is_active = true;

    // Note: we intentionally do not read/modify the entity here.
    // DragStart is only used to lock the tool mode.
    let _ = drag.entity;
}

fn on_drag_transform(
    drag: On<Pointer<Drag>>,
    mut objects: Query<&mut Transform>,
    tool: Res<TransformTool>,
    flycam_active: Res<FlyCamActive>,
) {
    // Never manipulate objects while flycam is active.
    if flycam_active.0 {
        return;
    }

    // Only apply transforms while an interaction is active.
    // (Drag events should normally only arrive while dragging, but this keeps the state model tight.)
    if !tool.is_active {
        return;
    }

    let Ok(mut transform) = objects.get_mut(drag.entity) else {
        return;
    };

    let mode = tool.selected_tool;

    // Provided by your drag event
    let delta: Vec2 = drag.delta;

    match mode {
        TransformToolMode::Rotate => {
            // Tune to taste: radians per pixel.
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

            transform.rotation = (q_yaw * q_pitch) * transform.rotation;
        }
        TransformToolMode::Translate => {
            // Simple screen-space -> world XZ plane mapping:
            // - drag right => +X
            // - drag up => -Z
            //
            // This is a placeholder until you implement camera-relative plane projection.
            let sensitivity = 0.01; // world units per pixel
            transform.translation.x += delta.x * sensitivity;
            transform.translation.z += -delta.y * sensitivity;
        }
        TransformToolMode::Scale => {
            // Simple uniform scale:
            // - drag right/up increases, left/down decreases
            let sensitivity = 0.01; // scale delta per pixel
            let ds = (delta.x - delta.y) * sensitivity;

            let mut new_scale = transform.scale + Vec3::splat(ds);
            // Prevent negative/zero scale
            new_scale = new_scale.max(Vec3::splat(0.001));
            transform.scale = new_scale;
        }
    }
}

fn on_drag_end(
    drag: On<Pointer<DragEnd>>,
    objects: Query<(&Transform, &ObjectId)>,
    stdb: SpacetimeDB,
    tool: ResMut<TransformTool>,
    flycam_active: Res<FlyCamActive>,
) {
    // If flycam is active, we shouldn't have been manipulating; ensure we unlock.
    let mut tool = tool;
    if flycam_active.0 {
        tool.is_active = false;
        return;
    }

    if let Ok((transform, id)) = objects.get(drag.entity) {
        // Save only what matches the selected/active tool.
        // Since tool switching is disabled while `is_active == true`,
        // `selected_tool` is effectively the locked tool for this gesture.
        match tool.selected_tool {
            TransformToolMode::Rotate => {
                let _ = stdb
                    .reducers()
                    .rotate_object(id.0, transform.rotation.into());
            }
            TransformToolMode::Translate => {
                let _ = stdb
                    .reducers()
                    .move_object(id.0, transform.translation.into());
            }
            TransformToolMode::Scale => {
                let _ = stdb.reducers().scale_object(id.0, transform.scale.into());
            }
        }
    }

    // Unlock tool switching after we've saved.
    tool.is_active = false;
}
