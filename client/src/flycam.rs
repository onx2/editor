use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_camera);
    app.add_systems(Update, (on_fly_up, on_fly_down, on_move, on_look));
}

const CAMERA_OFFSET_GLOBAL: Vec3 = Vec3::new(0.0, 25.0, -10.0);
// const CAMERA_DECAY_RATE: f32 = 24.0;
fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(CAMERA_OFFSET_GLOBAL).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn on_fly_up() {
    // Shift key to ascend (+Y)
}
fn on_fly_down() {
    // Shift key to descend (-Y)
}
fn on_move() {
    // WASD to control planar movement (X/Z)
}
fn on_look() {
    // rotate camera using RMB held and mouse move
}
