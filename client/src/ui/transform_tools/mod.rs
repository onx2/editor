//! Transform tools UI (Move / Rotate / Scale) similar to Unreal's viewport toolbar.
//!
//! Unreal calls this set of tools the "Transform Gizmos" / "Transform Tools" (W/E/R hotkeys).
//! Internally it's often referred to as the current "widget mode" (translate/rotate/scale).
//!
//! This module provides:
//! - `TransformToolMode` enum (Translate/Rotate/Scale)
//! - `ActiveTransformTool` resource (current mode)
//! - an egui toolbar renderer suitable for placing in the top app bar
//! - W/E/R hotkeys to switch mode
//!
//! Integration notes (wiring this into your existing UI):
//! - Add this module under `client/src/ui/mod.rs` (e.g. `mod transform_tools;` and add its plugin).
//! - Call `transform_tools::render_toolbar(ui, active_tool)` from `ui/app_bar/mod.rs` where you want it.
//! - Use `Res<ActiveTransformTool>` from gameplay/interaction systems to decide which drag behavior to apply.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::flycam::FlyCamActive;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ActiveTransformTool>();

    // Hotkeys are handled in Update so it works regardless of egui pass scheduling.
    app.add_systems(Update, handle_hotkeys);

    // Optional: if you want this module to render its own panel, you can enable the system below.
    // For now, we expose `render_toolbar(...)` so the app bar can host it.
    //
    // app.add_systems(EguiPrimaryContextPass, render_panel);
}

/// Equivalent to Unreal's widget mode (Translate/Rotate/Scale).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransformToolMode {
    Translate,
    Rotate,
    Scale,
}

impl TransformToolMode {
    pub fn label(self) -> &'static str {
        match self {
            TransformToolMode::Translate => "Move",
            TransformToolMode::Rotate => "Rotate",
            TransformToolMode::Scale => "Scale",
        }
    }

    /// Unreal-style hotkeys (W/E/R).
    pub fn hotkey(self) -> &'static str {
        match self {
            TransformToolMode::Translate => "W",
            TransformToolMode::Rotate => "E",
            TransformToolMode::Scale => "R",
        }
    }
}

/// Global editor state: which transform tool is currently active.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveTransformTool {
    pub mode: TransformToolMode,
}

impl Default for ActiveTransformTool {
    fn default() -> Self {
        Self {
            mode: TransformToolMode::Translate,
        }
    }
}

/// Render a compact, single-select button group (toggle group) for the transform tools.
/// Call this from your top app bar UI.
pub fn render_toolbar(ui: &mut egui::Ui, active: &mut ActiveTransformTool) {
    // This uses `selectable_label` which behaves like a toggle, and we enforce exclusivity by
    // setting `active.mode` when clicked.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;

        tool_button(ui, active, TransformToolMode::Translate);
        tool_button(ui, active, TransformToolMode::Rotate);
        tool_button(ui, active, TransformToolMode::Scale);
    });
}

fn tool_button(ui: &mut egui::Ui, active: &mut ActiveTransformTool, mode: TransformToolMode) {
    let selected = active.mode == mode;

    // Text-only for now; later you can swap to icons.
    // We include the hotkey hint to match the "editor muscle memory" vibe.
    let text = format!("{} ({})", mode.label(), mode.hotkey());

    if ui.selectable_label(selected, text).clicked() {
        active.mode = mode;
    }
}

/// Handle W/E/R hotkeys to switch the active transform tool.
///
/// This matches Unreal defaults:
/// - W = Translate
/// - E = Rotate
/// - R = Scale
fn handle_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    flycam_active: Res<FlyCamActive>,
    mut active: ResMut<ActiveTransformTool>,
    mut contexts: EguiContexts,
) {
    // Gate editor tool hotkeys while flycam is active to avoid conflicts.
    // Single source of truth lives in `flycam::FlyCamActive`.
    if flycam_active.0 {
        return;
    }

    // Also gate hotkeys while egui wants keyboard input (typing in text fields, etc.)
    // or pointer input (dragging sliders, clicking UI).
    //
    // Note: `EguiContexts::ctx_mut()` returns a `Result`, not an `Option`.
    // If there's no primary context for some reason, we just skip this gate.
    if let Ok(ctx) = contexts.ctx_mut() {
        if ctx.wants_keyboard_input() || ctx.wants_pointer_input() {
            return;
        }
    }

    if keys.just_pressed(KeyCode::KeyW) {
        active.mode = TransformToolMode::Translate;
    } else if keys.just_pressed(KeyCode::KeyE) {
        active.mode = TransformToolMode::Rotate;
    } else if keys.just_pressed(KeyCode::KeyR) {
        active.mode = TransformToolMode::Scale;
    }
}

/// Optional standalone panel renderer (not currently used).
/// Kept here if you decide you want an always-visible toolbar without editing the existing app bar.
#[allow(dead_code)]
fn render_panel(mut contexts: EguiContexts, mut active: ResMut<ActiveTransformTool>) {
    let ctx = contexts.ctx_mut().expect("to get primary egui context");

    egui::TopBottomPanel::top("transform_tools_panel")
        .resizable(false)
        .exact_height(32.0)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                render_toolbar(ui, &mut active);
                ui.add_space(ui.available_width());
            });
        });
}
