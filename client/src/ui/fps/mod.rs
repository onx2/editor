use bevy::{
    app::{App, PreUpdate},
    ecs::{
        resource::Resource,
        system::{Res, ResMut},
    },
    time::Time,
};

const FPS_HISTORY_LEN: usize = 250;

pub fn plugin(app: &mut App) {
    app.init_resource::<FpsDebug>();
    app.add_systems(PreUpdate, tick);
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
    // Grab values up-front to avoid borrowing `fps_debug` mutably + immutably in the same expression.
    let new_dt = time.delta_secs();
    let prev_dt = fps_debug.curr_delta_secs;

    // Take a single mutable borrow of the resource for the rest of this function,
    // then work through locals to avoid E0499/E0502.
    let fps = fps_debug.as_mut();

    fps.frame_counter += 1;

    fps.last_delta_secs = prev_dt;
    fps.curr_delta_secs = new_dt;

    // Ring-buffer update using locals (no multiple borrows of `fps` fields in one expression).
    let head = fps.history_head;
    fps.frame_times_secs[head] = new_dt;
    fps.history_head = (head + 1) % FPS_HISTORY_LEN;
    if fps.history_len < FPS_HISTORY_LEN {
        fps.history_len += 1;
    }

    // Snapshot length once, then compute the rolling average.
    let history_len = fps.history_len;

    // Avoid borrowing a slice of `fps.frame_times_secs` (which can extend the borrow)
    // by summing manually over indices.
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
