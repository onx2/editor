use bevy::{
    app::{App, Startup, Update},
    camera::{Camera3d, Exposure},
    color::Color,
    ecs::{
        component::Component,
        message::MessageReader,
        query::With,
        resource::Resource,
        system::{Commands, Res, ResMut, Single},
    },
    input::{
        ButtonInput,
        keyboard::KeyCode,
        mouse::{MouseButton, MouseMotion, MouseScrollUnit, MouseWheel},
    },
    math::{Quat, Vec2, Vec3},
    pbr::{AtmosphereMode, AtmosphereSettings, DistanceFog, FogFalloff},
    prelude::IntoScheduleConfigs,
    time::Time,
    transform::components::Transform,
    utils::default,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use bevy_egui::EguiContexts;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<FlyCamSettings>();
    app.init_resource::<FlyCamActive>();

    app.add_systems(Startup, spawn_camera);

    // Compute flycam active state once per frame so other systems (editor hotkeys, gizmos, etc.)
    // can gate behavior consistently without duplicating input logic.
    app.add_systems(Update, update_flycam_active);

    app.add_systems(
        Update,
        (
            flycam_toggle_capture,
            flycam_look.run_if(flycam_is_active),
            flycam_move.run_if(flycam_is_active),
            flycam_pan.run_if(flycam_pan_is_active),
            flycam_scroll_zoom,
        ),
    );
}

#[derive(Component)]
pub struct FlyCam;

#[derive(Resource)]
pub struct FlyCamSettings {
    /// Fly movement speed in meters/second (while RMB is held).
    pub fly_speed: f32,
    /// Mouse sensitivity in radians per pixel.
    pub mouse_sensitivity: f32,
    /// Pan speed in meters per pixel of mouse movement (while MMB is held).
    pub pan_sensitivity: f32,
    /// Scroll zoom speed ratio (dolly forward/back) expressed as a multiple of `fly_speed`.
    ///
    /// This applies to both mouse wheels and trackpads, but we interpret their units differently:
    /// - `MouseScrollUnit::Line` (typical mouse wheel): treated as discrete ticks (no `dt` scaling)
    /// - `MouseScrollUnit::Pixel` (typical trackpad): treated as continuous input (scaled by `dt`)
    pub scroll_zoom_speed_ratio: f32,
    /// Trackpad scroll scaling for `MouseScrollUnit::Pixel`.
    ///
    /// This is effectively "pixels of scroll per second" converted into a normalized amount.
    /// Tune to taste; larger = faster trackpad zoom.
    pub trackpad_pixels_per_scroll: f32,
    /// Pitch clamp to avoid gimbal flips.
    pub max_pitch_radians: f32,
}

impl Default for FlyCamSettings {
    fn default() -> Self {
        Self {
            fly_speed: 12.0,
            mouse_sensitivity: 0.0025,
            pan_sensitivity: 0.02,
            // Keep zoom aligned with fly speed.
            // Tune this to taste: 0.1..0.35 is a reasonable range.
            scroll_zoom_speed_ratio: 0.25,
            // Higher values make trackpad zoom faster.
            trackpad_pixels_per_scroll: 1024.0,
            max_pitch_radians: 1.54, // ~88 degrees
        }
    }
}

const CAMERA_OFFSET_GLOBAL: Vec3 = Vec3::new(0.0, 25.0, -10.0);

/// Global "is flycam active" state for the current frame.
/// Treat this as the single source of truth for gating editor hotkeys and interactions.
///
/// Unreal-style behavior: flycam is "captured" on RMB just_pressed (only if not over UI),
/// and released on RMB just_released.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlyCamActive(pub bool);

fn update_flycam_active(
    buttons: Res<ButtonInput<MouseButton>>,
    mut active: ResMut<FlyCamActive>,
    mut contexts: EguiContexts,
) {
    // Is the pointer currently over any egui UI (even hover)?
    let pointer_over_egui = contexts
        .ctx_mut()
        .map(|ctx| ctx.is_pointer_over_area())
        .unwrap_or(false);

    // Only begin flycam if the RMB press started outside UI.
    if buttons.just_pressed(MouseButton::Right) {
        active.0 = !pointer_over_egui;
    }

    // Always stop flycam when RMB is released.
    if buttons.just_released(MouseButton::Right) {
        active.0 = false;
    }

    // Safety: if RMB isn't held (e.g. focus loss), flycam can't be active.
    if !buttons.pressed(MouseButton::Right) {
        active.0 = false;
    }
}

fn spawn_camera(mut commands: Commands) {
    // World camera
    commands.spawn((
        FlyCam,
        Exposure { ev100: 15.0 },
        bevy::core_pipeline::tonemapping::Tonemapping::TonyMcMapface,
        Camera3d::default(),
        Transform::from_translation(CAMERA_OFFSET_GLOBAL).looking_at(Vec3::ZERO, Vec3::Y),
        DistanceFog {
            color: Color::srgba(0.35, 0.48, 0.66, 1.0),
            directional_light_color: Color::srgba(1.0, 0.95, 0.85, 0.5),
            directional_light_exponent: 30.0,
            falloff: FogFalloff::from_visibility_colors(
                1000.0, // Fog distance
                Color::srgb(0.35, 0.5, 0.66),
                Color::srgb(0.8, 0.8, 0.7),
            ),
        },
        AtmosphereSettings {
            rendering_method: AtmosphereMode::Raymarched,
            ..default()
        },
    ));
}

fn flycam_is_active(active: Res<FlyCamActive>) -> bool {
    active.0
}

fn flycam_toggle_capture(
    buttons: Res<ButtonInput<MouseButton>>,
    flycam_active: Res<FlyCamActive>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    // Capture/hide cursor only when flycam is actually active.
    // This prevents RMB on top of egui UI from locking/hiding the cursor.
    if buttons.just_pressed(MouseButton::Right) && flycam_active.0 {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }

    // Always release on RMB up (safe even if we never captured).
    if buttons.just_released(MouseButton::Right) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

fn flycam_look(
    mut motion_evr: MessageReader<MouseMotion>,
    settings: Res<FlyCamSettings>,
    mut flycam_transform: Single<&mut Transform, With<FlyCam>>,
) {
    // Accumulate mouse delta for the frame.
    let mut delta = Vec2::ZERO;
    for ev in motion_evr.read() {
        delta += ev.delta;
    }
    if delta == Vec2::ZERO {
        return;
    }

    // Note: typical editor convention is:
    // - mouse right => yaw right
    // - mouse up => pitch up (invert Y as needed)
    let yaw_delta = -delta.x * settings.mouse_sensitivity;
    let pitch_delta = -delta.y * settings.mouse_sensitivity;

    // Apply yaw around global up.
    flycam_transform.rotate(Quat::from_axis_angle(Vec3::Y, yaw_delta));

    // Apply pitch around camera local right axis, clamped.
    // Compute current pitch by projecting forward onto XZ plane.
    let forward = flycam_transform.forward();
    let current_pitch = forward.y.asin();

    let target_pitch = (current_pitch + pitch_delta)
        .clamp(-settings.max_pitch_radians, settings.max_pitch_radians);

    let clamped_delta = target_pitch - current_pitch;

    // Right axis in world space
    let right = flycam_transform.right();
    flycam_transform.rotate(Quat::from_axis_angle(*right, clamped_delta));
}

fn flycam_move(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<FlyCamSettings>,
    mut flycam_transform: Single<&mut Transform, With<FlyCam>>,
) {
    let mut input = Vec3::ZERO;

    // Planar movement
    if keys.pressed(KeyCode::KeyW) {
        input.z += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        input.z -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        input.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        input.x += 1.0;
    }

    // Vertical movement
    if keys.pressed(KeyCode::KeyE) {
        input.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyQ) {
        input.y -= 1.0;
    }

    if input == Vec3::ZERO {
        return;
    }

    let dt = time.delta_secs();
    let speed = settings.fly_speed;

    // Move relative to camera orientation
    let mut desired_dir = Vec3::ZERO;

    let right = *flycam_transform.right();
    let forward = *flycam_transform.forward();

    desired_dir += right * input.x;
    desired_dir += Vec3::Y * input.y;
    desired_dir += forward * input.z;

    // Keep diagonal speed consistent
    let desired_dir = desired_dir.normalize_or_zero();

    flycam_transform.translation += desired_dir * speed * dt;
}

fn flycam_pan_is_active(buttons: Res<ButtonInput<MouseButton>>) -> bool {
    buttons.pressed(MouseButton::Middle)
}

fn flycam_pan(
    mut motion_evr: MessageReader<MouseMotion>,
    settings: Res<FlyCamSettings>,
    mut flycam_transform: Single<&mut Transform, With<FlyCam>>,
) {
    // Accumulate mouse delta for the frame.
    let mut delta = Vec2::ZERO;
    for ev in motion_evr.read() {
        delta += ev.delta;
    }
    if delta == Vec2::ZERO {
        return;
    }

    // Drag moves camera left/right/up/down in view space:
    // - drag right => move right
    // - drag up => move up
    let pan_right = delta.x * settings.pan_sensitivity;
    let pan_up = -delta.y * settings.pan_sensitivity;
    let right = *flycam_transform.right();
    flycam_transform.translation += right * pan_right;
    flycam_transform.translation += Vec3::Y * pan_up;
}

fn flycam_scroll_zoom(
    mut wheel_evr: MessageReader<MouseWheel>,
    settings: Res<FlyCamSettings>,
    mut flycam_transform: Single<&mut Transform, With<FlyCam>>,
) {
    let forward = *flycam_transform.forward();
    for ev in wheel_evr.read() {
        let amount = match ev.unit {
            // Mouse scroll wheel's discrete "ticks": treat `ev.y` as steps
            MouseScrollUnit::Line => ev.y * settings.fly_speed * settings.scroll_zoom_speed_ratio,
            // Handle continous scrolling (trackpad, smooth wheel scroll), tuned using a damping constant
            MouseScrollUnit::Pixel => {
                let normalized = ev.y / settings.trackpad_pixels_per_scroll.max(1.0);
                normalized * settings.fly_speed * settings.scroll_zoom_speed_ratio
            }
        };

        flycam_transform.translation += forward * amount;
    }
}
