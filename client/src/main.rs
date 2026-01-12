mod flycam;
mod infinite_grid;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, infinite_grid::plugin, flycam::plugin))
        .add_systems(Startup, spawn_grid_scale_overlay)
        .add_systems(Update, update_grid_scale_overlay)
        .run();
}

#[derive(Component)]
struct GridScaleOverlay;

fn spawn_grid_scale_overlay(mut commands: Commands) {
    commands.spawn((
        GridScaleOverlay,
        Text::new("Grid: ?"),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(0.92, 0.92, 0.92)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            bottom: Val::Px(12.0),
            ..default()
        },
    ));
}

fn update_grid_scale_overlay(
    mut q_text: Query<&mut Text, With<GridScaleOverlay>>,
    q_cam: Query<&GlobalTransform, With<Camera3d>>,
) {
    let Ok(mut text) = q_text.single_mut() else {
        return;
    };
    let Ok(cam_gt) = q_cam.single() else {
        return;
    };

    // Plane is y=0 in this editor.
    let h = cam_gt.translation().y.abs();

    // These thresholds should match the shader logic in `assets/shaders/infinite_grid.wgsl`.
    // Active pairs:
    // - 1cm ↔ 1m:   smoothstep(0.5, 8.0, h) then biased
    // - 1m  ↔ 10m:  smoothstep(5.0, 90.0, h) then biased
    // - 10m ↔ 100m: smoothstep(40.0, 600.0, h) then biased
    //
    // For the UI, we show the dominant scale. The shader biases crossfades so the
    // "dominant" scale tends to remain dominant longer; we approximate dominance
    // by using a higher cutoff than 50/50.
    fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
        let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    // Shader bias: t' = t^2
    fn bias_75_25(t: f32) -> f32 {
        t * t
    }

    // Dominance cutoff: show the "next" scale only when it is >= ~75%.
    let dominance_cutoff = 0.75;

    let scale_label = if h < 8.0 {
        let t = bias_75_25(smoothstep(0.5, 8.0, h));
        if t >= dominance_cutoff {
            "1m"
        } else {
            "1cm"
        }
    } else if h < 90.0 {
        let t = bias_75_25(smoothstep(5.0, 90.0, h));
        if t >= dominance_cutoff {
            "10m"
        } else {
            "1m"
        }
    } else {
        let t = bias_75_25(smoothstep(40.0, 600.0, h));
        if t >= dominance_cutoff {
            "100m"
        } else {
            "10m"
        }
    };

    text.0 = format!("Grid: {}", scale_label);
}
