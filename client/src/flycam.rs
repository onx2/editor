use bevy::{
    input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel},
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<FlyCamSettings>();
    app.add_systems(Startup, spawn_camera);
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
    /// If true, you must hold RMB to look/move (and we will lock/hide the cursor); if false, always active.
    pub require_rmb: bool,
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
            // Reasonable default for macOS trackpads / pixel-scrolling devices.
            // Higher values make trackpad zoom faster.
            trackpad_pixels_per_scroll: 1024.0,
            max_pitch_radians: 1.54, // ~88 degrees
            require_rmb: true,
        }
    }
}

const CAMERA_OFFSET_GLOBAL: Vec3 = Vec3::new(0.0, 25.0, -10.0);

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        FlyCam,
        Camera3d::default(),
        Transform::from_translation(CAMERA_OFFSET_GLOBAL).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn flycam_is_active(buttons: Res<ButtonInput<MouseButton>>, settings: Res<FlyCamSettings>) -> bool {
    if !settings.require_rmb {
        return true;
    }
    buttons.pressed(MouseButton::Right)
}

fn flycam_toggle_capture(
    buttons: Res<ButtonInput<MouseButton>>,
    settings: Res<FlyCamSettings>,
    mut cursor_q: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if !settings.require_rmb {
        return;
    }

    // In Bevy 0.17, cursor state is stored on the `CursorOptions` component (required by `Window`).
    // Grab/release cursor when RMB is pressed/released.
    let Ok(mut cursor) = cursor_q.single_mut() else {
        return;
    };

    if buttons.just_pressed(MouseButton::Right) {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }

    if buttons.just_released(MouseButton::Right) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

fn flycam_look(
    mut motion_evr: MessageReader<MouseMotion>,
    settings: Res<FlyCamSettings>,
    mut q: Query<&mut Transform, With<FlyCam>>,
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

    for mut transform in q.iter_mut() {
        // Apply yaw around global up.
        transform.rotate(Quat::from_axis_angle(Vec3::Y, yaw_delta));

        // Apply pitch around camera local right axis, clamped.
        // Compute current pitch by projecting forward onto XZ plane.
        let forward = transform.forward();
        let current_pitch = forward.y.asin();

        let target_pitch = (current_pitch + pitch_delta)
            .clamp(-settings.max_pitch_radians, settings.max_pitch_radians);

        let clamped_delta = target_pitch - current_pitch;

        // Right axis in world space
        let right = transform.right();
        transform.rotate(Quat::from_axis_angle(*right, clamped_delta));
    }
}

fn flycam_move(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<FlyCamSettings>,
    mut q: Query<&mut Transform, With<FlyCam>>,
) {
    let dt = time.delta_secs();

    let mut input = Vec3::ZERO;

    // Planar movement (WASD) - only active while RMB is held (Unreal-style).
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

    // Vertical movement: E up, Q down (Unreal-style).
    if keys.pressed(KeyCode::KeyE) {
        input.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyQ) {
        input.y -= 1.0;
    }

    if input == Vec3::ZERO {
        return;
    }

    let speed = settings.fly_speed;

    for mut transform in q.iter_mut() {
        // Move relative to camera orientation:
        // - X: right
        // - Z: forward (Bevy's `forward()` points in the direction the camera faces)
        let mut wish_dir = Vec3::ZERO;

        let right = *transform.right();
        let forward = *transform.forward();

        wish_dir += right * input.x;
        wish_dir += Vec3::Y * input.y;
        wish_dir += forward * input.z;

        // Normalize to keep diagonal speed consistent
        let wish_dir = wish_dir.normalize_or_zero();

        transform.translation += wish_dir * speed * dt;
    }
}

fn flycam_pan_is_active(buttons: Res<ButtonInput<MouseButton>>) -> bool {
    buttons.pressed(MouseButton::Middle)
}

fn flycam_pan(
    mut motion_evr: MessageReader<MouseMotion>,
    settings: Res<FlyCamSettings>,
    mut q: Query<&mut Transform, With<FlyCam>>,
) {
    // Accumulate mouse delta for the frame.
    let mut delta = Vec2::ZERO;
    for ev in motion_evr.read() {
        delta += ev.delta;
    }
    if delta == Vec2::ZERO {
        return;
    }

    // Unreal-style pan: drag moves camera left/right/up/down in view space.
    // We map:
    // - drag right => move right
    // - drag up => move up
    let pan_right = delta.x * settings.pan_sensitivity;
    let pan_up = -delta.y * settings.pan_sensitivity;

    for mut transform in q.iter_mut() {
        let right = *transform.right();
        transform.translation += right * pan_right;
        transform.translation += Vec3::Y * pan_up;
    }
}

fn flycam_scroll_zoom(
    time: Res<Time>,
    mut wheel_evr: MessageReader<MouseWheel>,
    settings: Res<FlyCamSettings>,
    mut q: Query<&mut Transform, With<FlyCam>>,
) {
    // Handle mouse wheel vs trackpad separately for consistent feel:
    // - Wheels typically send `Line` units as discrete ticks.
    // - Trackpads typically send `Pixel` units (continuous).
    //
    // NOTE: We intentionally do NOT scale pixel scroll by `dt`. Trackpads already provide
    // high-frequency deltas, and `dt` scaling tends to make zoom feel unresponsive.
    let _dt = time.delta_secs();

    let mut amount = 0.0f32;

    for ev in wheel_evr.read() {
        match ev.unit {
            MouseScrollUnit::Line => {
                // Discrete "ticks": treat `ev.y` as steps.
                amount += ev.y * settings.fly_speed * settings.scroll_zoom_speed_ratio;
            }
            MouseScrollUnit::Pixel => {
                // Continuous pixel scroll (trackpads): normalize by a tunable constant.
                //
                // IMPORTANT:
                // Trackpads often already encode "time" in the frequency/size of pixel scroll deltas.
                // Multiplying by `dt` can make trackpad zoom feel extremely slow and also makes
                // `trackpad_pixels_per_scroll` feel like it "does nothing".
                let normalized = ev.y / settings.trackpad_pixels_per_scroll.max(1.0);
                amount += normalized * settings.fly_speed * settings.scroll_zoom_speed_ratio;
            }
        }
    }

    if amount == 0.0 {
        return;
    }

    for mut transform in q.iter_mut() {
        let forward = *transform.forward();
        transform.translation += forward * amount;
    }
}
