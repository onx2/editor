mod app_bar;
mod fps;

use bevy::{
    camera::{CameraOutputMode, visibility::RenderLayers},
    diagnostic::{Diagnostic, DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::render_resource::BlendState,
};
use bevy_egui::{
    EguiContexts, EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext, egui,
};
use core::time::Duration;

const PERF_WINDOW_REFRESH_INTERVAL: Duration = Duration::from_millis(100);

pub fn plugin(app: &mut App) {
    // Provide Bevy frame diagnostics (FPS, frame time, etc.) for the egui window.
    // This is the same underlying data source used by `bevy_dev_tools::fps_overlay`.
    app.add_plugins(FrameTimeDiagnosticsPlugin::default());

    // Register egui once, from a central place.
    app.add_plugins((EguiPlugin::default(), fps::plugin));
    app.add_systems(Startup, setup);

    // Render panels in the egui pass schedule so the pass state is initialized.
    // Add them separately to avoid tuple config type mismatches.
    app.add_systems(EguiPrimaryContextPass, app_bar::render);
    app.add_systems(EguiPrimaryContextPass, fps_window);
}

fn setup(mut commands: Commands, mut egui_global_settings: ResMut<EguiGlobalSettings>) {
    // Disable the automatic creation of a primary context to set it up manually for the camera we need.
    egui_global_settings.auto_create_primary_context = false;
    // Egui camera.
    commands.spawn((
        // The `PrimaryEguiContext` component requires everything needed to render a primary context.
        PrimaryEguiContext,
        Camera2d,
        // Setting RenderLayers to none makes sure we won't render anything apart from the UI.
        RenderLayers::none(),
        Camera {
            order: 1,
            output_mode: CameraOutputMode::Write {
                blend_state: Some(BlendState::ALPHA_BLENDING),
                clear_color: ClearColorConfig::None,
            },
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
    ));
}

#[derive(Default)]
struct PerfWindowCache {
    next_refresh_in: Duration,
    fps: Option<f64>,
    frame_time_ms: Option<f64>,
    frame_time_ms_avg: Option<f64>,
    frame_time_ms_min: Option<f64>,
    frame_time_ms_max: Option<f64>,
}

fn fps_window(
    mut contexts: EguiContexts,
    diagnostics: Res<DiagnosticsStore>,
    time: Res<Time>,
    mut cache: Local<PerfWindowCache>,
) {
    let ctx = contexts.ctx_mut().expect("to get primary egui context");

    cache.next_refresh_in = cache.next_refresh_in.saturating_sub(time.delta());

    if cache.next_refresh_in == Duration::ZERO {
        cache.next_refresh_in = PERF_WINDOW_REFRESH_INTERVAL;

        cache.fps = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(Diagnostic::smoothed);

        cache.frame_time_ms = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .and_then(Diagnostic::smoothed);

        cache.frame_time_ms_avg = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .and_then(Diagnostic::average);

        cache.frame_time_ms_min = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .and_then(|diag| {
                diag.measurements()
                    .map(|m| m.value)
                    .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            });

        cache.frame_time_ms_max = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .and_then(|diag| {
                diag.measurements()
                    .map(|m| m.value)
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            });
    }

    egui::Window::new("Performance")
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("Frame stats");

            ui.horizontal(|ui| {
                ui.label("FPS (smoothed):");
                match cache.fps {
                    Some(fps) => ui.monospace(format!("{fps:.2}")),
                    None => ui.monospace("(warming up)"),
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Frame time (smoothed):");
                match cache.frame_time_ms {
                    Some(ms) => ui.monospace(format!("{ms:.3} ms")),
                    None => ui.monospace("(warming up)"),
                }
            });

            ui.horizontal(|ui| {
                ui.label("Frame time (avg):");
                match cache.frame_time_ms_avg {
                    Some(ms) => ui.monospace(format!("{ms:.3} ms")),
                    None => ui.monospace("(warming up)"),
                }
            });

            ui.horizontal(|ui| {
                ui.label("Frame time (min):");
                match cache.frame_time_ms_min {
                    Some(ms) => ui.monospace(format!("{ms:.3} ms")),
                    None => ui.monospace("(warming up)"),
                }
            });

            ui.horizontal(|ui| {
                ui.label("Frame time (max):");
                match cache.frame_time_ms_max {
                    Some(ms) => ui.monospace(format!("{ms:.3} ms")),
                    None => ui.monospace("(warming up)"),
                }
            });

            ui.separator();
            ui.small(format!(
                "Refresh: every {} ms",
                PERF_WINDOW_REFRESH_INTERVAL.as_millis()
            ));
        });
}
