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

// Flame graph tuning.
const FLAME_GRAPH_HEIGHT_PX: f32 = 60.0;
const FLAME_GRAPH_BAR_GAP_PX: f32 = 1.0;

// Fixed flame-graph vertical scale (0..MAX_MS).
const FLAME_GRAPH_MAX_MS: f32 = 16.6667;

fn ms_to_fps(ms: f64) -> f64 {
    if ms > 0.0 { 1000.0 / ms } else { 0.0 }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<FpsDebug>();
    app.add_plugins(FrameTimeDiagnosticsPlugin::default());
    app.add_systems(PreUpdate, tick);
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
    frame_time_ms_min: Option<f64>,
    frame_time_ms_max: Option<f64>,
}

fn render(
    mut contexts: EguiContexts,
    diagnostics: Res<DiagnosticsStore>,
    time: Res<Time>,
    fps_debug: Res<FpsDebug>,
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
        .max_width(300.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("FPS:");
                match (cache.fps, cache.frame_time_ms) {
                    (Some(fps), Some(ms)) => {
                        ui.monospace(format!("{fps:.2} ({ms:.2} ms)"));
                    }
                    (Some(fps), None) => {
                        ui.monospace(format!("{fps:.2}"));
                    }
                    (None, Some(ms)) => {
                        ui.monospace(format!("(warming up) ({ms:.2} ms)"));
                    }
                    (None, None) => {
                        ui.monospace("(warming up)");
                    }
                };

                ui.add_space(ui.available_width());

                let mut history_min_ms: Option<f64> = None;
                let mut history_max_ms: Option<f64> = None;

                let n = fps_debug.history_len.min(FPS_HISTORY_LEN);
                if n > 0 {
                    let start = (fps_debug.history_head + FPS_HISTORY_LEN - n) % FPS_HISTORY_LEN;
                    for i in 0..n {
                        let idx = (start + i) % FPS_HISTORY_LEN;
                        let dt_ms = (fps_debug.frame_times_secs[idx] as f64) * 1000.0;

                        history_min_ms = Some(match history_min_ms {
                            Some(v) => v.min(dt_ms),
                            None => dt_ms,
                        });
                        history_max_ms = Some(match history_max_ms {
                            Some(v) => v.max(dt_ms),
                            None => dt_ms,
                        });
                    }
                }

                // Worst (max ms) => min FPS, best (min ms) => max FPS.
                let min_fps = history_max_ms.map(ms_to_fps);
                let max_fps = history_min_ms.map(ms_to_fps);

                ui.label("min:");
                match min_fps {
                    Some(v) => {
                        ui.monospace(format!("{v:.2}"));
                    }
                    None => {
                        ui.monospace("(warming up)");
                    }
                };

                ui.add_space(12.0);

                ui.label("max:");
                match max_fps {
                    Some(v) => {
                        ui.monospace(format!("{v:.2}"));
                    }
                    None => {
                        ui.monospace("(warming up)");
                    }
                };
            });

            ui.separator();

            // Flame graph of the last N frame times (ms), driven by our `FpsDebug` ring buffer.
            let graph_width = ui.available_width().min(480.0);
            let (rect, _response) = ui.allocate_exact_size(
                egui::vec2(graph_width, FLAME_GRAPH_HEIGHT_PX),
                egui::Sense::hover(),
            );

            let painter = ui.painter();

            // Background.
            painter.rect_filled(rect, 2.0, egui::Color32::from_gray(18));
            painter.rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
                egui::StrokeKind::Inside,
            );

            // Draw bars oldest -> newest, left -> right.
            let n = fps_debug.history_len.min(FPS_HISTORY_LEN);
            if n > 0 {
                let bar_w = (rect.width() / n as f32).max(1.0);
                let gap = FLAME_GRAPH_BAR_GAP_PX.min(bar_w - 1.0).max(0.0);

                // Oldest index in the ring buffer.
                let start = (fps_debug.history_head + FPS_HISTORY_LEN - n) % FPS_HISTORY_LEN;

                let scale_min_ms: f32 = 0.0;
                let scale_max_ms: f32 = FLAME_GRAPH_MAX_MS;
                let scale_range_ms: f32 = (scale_max_ms - scale_min_ms).max(0.0001);

                for i in 0..n {
                    let idx = (start + i) % FPS_HISTORY_LEN;
                    let dt_ms = fps_debug.frame_times_secs[idx] * 1000.0;

                    // Normalize to graph height using the fixed range.
                    let t = ((dt_ms - scale_min_ms) / scale_range_ms).clamp(0.0, 1.0);
                    let h = t * rect.height();

                    let x0 = rect.left() + i as f32 * bar_w;
                    let x1 = (x0 + bar_w - gap).min(rect.right());
                    let y1 = rect.bottom();
                    let y0 = (y1 - h).max(rect.top());

                    // Color: green -> yellow -> red based on normalized height.
                    let color = if t < 0.5 {
                        // green to yellow
                        let k = (t / 0.5) as f32;
                        egui::Color32::from_rgb((0.0 + k * 255.0) as u8, 255, 0)
                    } else {
                        // yellow to red
                        let k = ((t - 0.5) / 0.5) as f32;
                        egui::Color32::from_rgb(255, (255.0 - k * 255.0) as u8, 0)
                    };

                    let bar = egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1));
                    painter.rect_filled(bar, 0.0, color);
                }

                // Reference lines: 60fps (16.67ms) and 30fps (33.33ms), mapped into the same scale.
                let line_color = egui::Color32::from_gray(90);
                for &ms in &[16.67_f32, 33.33_f32] {
                    let t = ((ms - scale_min_ms) / scale_range_ms).clamp(0.0, 1.0);
                    let y = rect.bottom() - t * rect.height();
                    painter.line_segment(
                        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                        egui::Stroke::new(1.0, line_color),
                    );
                }
            }

            ui.separator();
            ui.small(format!(
                "Refresh: every {} ms",
                PERF_WINDOW_REFRESH_INTERVAL.as_millis()
            ));
        });
}
