use bevy::{
    app::{App, PreUpdate},
    diagnostic::{Diagnostic, DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    ecs::{
        resource::Resource,
        system::{Local, Res, ResMut},
    },
    time::Time,
};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use core::time::Duration;

const FPS_HISTORY_LEN: usize = 250;
const PERF_WINDOW_REFRESH_INTERVAL: Duration = Duration::from_millis(100);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<FpsDebug>();
    app.add_systems(PreUpdate, tick);

    // - Bevy diagnostics (same source used by bevy_dev_tools fps_overlay)
    app.add_plugins(FrameTimeDiagnosticsPlugin::default());

    // UI:
    app.add_systems(EguiPrimaryContextPass, render);
}

#[derive(Resource)]
pub struct FpsDebug {
    /// Previous frame delta time (seconds).
    pub last_delta_secs: f32,
    /// Current frame delta time (seconds).
    pub curr_delta_secs: f32,
    /// Average FPS over the rolling window (`frame_times_secs`).
    pub avg_frame_rate: f32,

    /// Fixed-size ring buffer storing the last N frame times (seconds).
    /// Slots beyond `history_len` are valid but not yet "filled" with real history.
    pub frame_times_secs: [f32; FPS_HISTORY_LEN],
    /// Next write position in the ring buffer.
    pub history_head: usize,
    /// Number of valid samples collected so far (<= FPS_HISTORY_LEN).
    pub history_len: usize,

    /// Internal: how many ticks have occurred (used to print periodically).
    pub frame_counter: u64,
}

impl Default for FpsDebug {
    fn default() -> Self {
        Self {
            last_delta_secs: 0.0,
            curr_delta_secs: 0.0,
            avg_frame_rate: 0.0,
            frame_times_secs: [0.0; FPS_HISTORY_LEN],
            history_head: 0,
            history_len: 0,
            frame_counter: 0,
        }
    }
}

pub fn tick(time: Res<Time>, mut fps_debug: ResMut<FpsDebug>) {
    let new_dt = time.delta_secs();
    let prev_dt = fps_debug.curr_delta_secs;

    let fps = fps_debug.as_mut();

    fps.frame_counter += 1;

    fps.last_delta_secs = prev_dt;
    fps.curr_delta_secs = new_dt;

    let head = fps.history_head;
    fps.frame_times_secs[head] = new_dt;
    fps.history_head = (head + 1) % FPS_HISTORY_LEN;
    if fps.history_len < FPS_HISTORY_LEN {
        fps.history_len += 1;
    }

    let history_len = fps.history_len;

    let mut sum: f32 = 0.0;
    for i in 0..history_len {
        sum += fps.frame_times_secs[i];
    }

    fps.avg_frame_rate = if history_len > 0 && sum > 0.0 {
        let avg_dt = sum / (history_len as f32);
        1.0 / avg_dt
    } else {
        0.0
    };
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

fn render(
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
                    .min_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
            });

        cache.frame_time_ms_max = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .and_then(|diag| {
                diag.measurements()
                    .map(|m| m.value)
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
            });
    }

    egui::Window::new("Performance")
        .resizable(false)
        .show(ctx, |ui| {
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
