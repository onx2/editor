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

#[derive(Resource, Default)]
struct DragMoveState {
    /// World-space offset between the object's origin and the cursor-projected hit point at drag start.
    offset: Option<Vec3>,
    /// The view-plane used for "free move" translation:
    /// - passes through the object's position at drag start
    /// - has normal = camera forward at drag start
    plane_origin: Option<Vec3>,
    plane_normal: Option<Vec3>,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<DragMoveState>();
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
    objects: Query<&Transform>,
    tool: ResMut<TransformTool>,
    flycam_active: Res<FlyCamActive>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::flycam::FlyCam>>,
    mut move_state: ResMut<DragMoveState>,
) {
    // Never begin a transform interaction while flycam is active.
    if flycam_active.0 {
        return;
    }

    // Lock tool switching for the duration of the drag gesture.
    // We don't allow changing selected tool while active, so `selected_tool` is effectively the locked tool.
    let mut tool = tool;
    tool.is_active = true;

    // Reset move state each drag start.
    move_state.offset = None;
    move_state.plane_origin = None;
    move_state.plane_normal = None;

    if tool.selected_tool != TransformToolMode::Translate {
        return;
    }

    let Ok(object_tf) = objects.get(drag.entity) else {
        return;
    };

    // Use the primary flycam camera.
    let Ok((cam, cam_gt)) = camera.single() else {
        return;
    };

    // Project cursor to a world ray.
    // In Bevy 0.17, this returns `Result<Ray3d, ViewportConversionError>`.
    let Ok(ray) = cam.viewport_to_world(cam_gt, drag.pointer_location.position) else {
        return;
    };

    // View-plane free move:
    // Plane passes through the object and faces the camera.
    let plane_origin = object_tf.translation;
    let plane_normal = cam_gt.forward().as_vec3();

    // If the ray is nearly parallel to the plane, bail.
    let denom = ray.direction.dot(plane_normal);
    if denom.abs() < 1e-6 {
        return;
    }

    let t = (plane_origin - ray.origin).dot(plane_normal) / denom;
    if t <= 0.0 {
        return;
    }

    let hit = ray.origin + ray.direction * t;

    // Store offset so we don't snap the object origin onto the cursor at drag start.
    move_state.offset = Some(object_tf.translation - hit);
    move_state.plane_origin = Some(plane_origin);
    move_state.plane_normal = Some(plane_normal);
}

fn on_drag_transform(
    drag: On<Pointer<Drag>>,
    mut objects: Query<&mut Transform>,
    tool: Res<TransformTool>,
    flycam_active: Res<FlyCamActive>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::flycam::FlyCam>>,
    move_state: ResMut<DragMoveState>,
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
            // View-plane free move (Unreal-like):
            // - Cast a ray from the camera through the cursor.
            // - Intersect with a camera-facing plane captured at DragStart (through the object).
            // - Place the object at the hit point + an offset captured at DragStart.
            let Ok((cam, cam_gt)) = camera.single() else {
                return;
            };

            // In Bevy 0.17, this returns `Result<Ray3d, ViewportConversionError>`.
            let Ok(ray) = cam.viewport_to_world(cam_gt, drag.pointer_location.position) else {
                return;
            };

            let plane_origin = move_state.plane_origin.unwrap_or(transform.translation);
            let plane_normal = move_state
                .plane_normal
                .unwrap_or_else(|| cam_gt.forward().as_vec3());

            let denom = ray.direction.dot(plane_normal);
            if denom.abs() < 1e-6 {
                return;
            }

            let t = (plane_origin - ray.origin).dot(plane_normal) / denom;
            if t <= 0.0 {
                return;
            }

            let hit = ray.origin + ray.direction * t;

            // If we somehow missed DragStart offset, fall back to snapping the origin to cursor.
            let offset = move_state.offset.unwrap_or(Vec3::ZERO);
            transform.translation = hit + offset;
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
    mut move_state: ResMut<DragMoveState>,
) {
    // If flycam is active, we shouldn't have been manipulating; ensure we unlock.
    let mut tool = tool;
    if flycam_active.0 {
        tool.is_active = false;
        move_state.offset = None;
        move_state.plane_origin = None;
        move_state.plane_normal = None;
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
    move_state.offset = None;
    move_state.plane_origin = None;
    move_state.plane_normal = None;
}
