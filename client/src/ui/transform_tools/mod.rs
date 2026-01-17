//! Transform tools UI (Move / Rotate / Scale) similar to Unreal's viewport toolbar.
//!
//! Unreal calls this set of tools the "Transform Gizmos" / "Transform Tools" (W/E/R hotkeys).
//! Internally it's often referred to as the current "widget mode" (translate/rotate/scale).
//!
//! This module provides:
//! - `TransformToolMode` enum (Translate/Rotate/Scale)
//! - `TransformTool` resource: `{ selected_tool, is_active }`
//! - an egui toolbar renderer suitable for placing in the top app bar
//! - W/E/R hotkeys to switch mode (disabled while a drag interaction is active)
//!
//! Tool locking:
//! - While a transform drag interaction is active, switching tools is disabled.
//! - The interaction system should set `TransformTool.is_active = true` on `DragStart`
//!   and set it back to `false` on `DragEnd` (after saving).

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::flycam::FlyCamActive;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<TransformTool>();

    // Hotkeys are handled in Update so it works regardless of egui pass scheduling.
    app.add_systems(Update, handle_hotkeys);
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

/// Global editor state for transform tools.
///
/// `selected_tool` is the currently selected tool.
/// `is_active` means "a drag interaction is currently using the selected tool"
/// and tool switching must be disabled until the interaction ends.
///
/// The interaction system (object dragging) should:
/// - set `is_active = true` on `DragStart`
/// - set `is_active = false` on `DragEnd` (after saving)
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransformTool {
    pub selected_tool: TransformToolMode,
    pub is_active: bool,
}

impl Default for TransformTool {
    fn default() -> Self {
        Self {
            selected_tool: TransformToolMode::Translate,
            is_active: false,
        }
    }
}

/// Render a compact, single-select button group (toggle group) for the transform tools.
/// Call this from your top app bar UI.
///
/// While `TransformTool.is_active == true`, buttons are disabled.
pub fn render_toolbar(ui: &mut egui::Ui, tool: &mut TransformTool) {
    let disabled = tool.is_active;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;

        tool_button(ui, tool, TransformToolMode::Translate, disabled);
        tool_button(ui, tool, TransformToolMode::Rotate, disabled);
        tool_button(ui, tool, TransformToolMode::Scale, disabled);
    });
}

fn tool_button(
    ui: &mut egui::Ui,
    tool: &mut TransformTool,
    mode: TransformToolMode,
    disabled: bool,
) {
    let selected = tool.selected_tool == mode;

    let text = format!("{} ({})", mode.label(), mode.hotkey());

    let resp = ui.add_enabled(!disabled, egui::Button::new(text).selected(selected));
    if resp.clicked() {
        tool.selected_tool = mode;
    }
}

/// Handle W/E/R hotkeys to switch the selected transform tool.
///
/// Disabled while:
/// - flycam is active
/// - egui is interacting
/// - a transform drag interaction is active (`TransformTool.is_active == true`)
fn handle_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    flycam_active: Res<FlyCamActive>,
    mut tool: ResMut<TransformTool>,
    mut contexts: EguiContexts,
) {
    // Disable tool switching while a transform interaction is active.
    if tool.is_active {
        return;
    }

    // Gate editor tool hotkeys while flycam is active to avoid conflicts.
    if flycam_active.0 {
        return;
    }

    // Also gate hotkeys while egui wants keyboard input (typing) or pointer input (dragging/clicking UI).
    if let Ok(ctx) = contexts.ctx_mut() {
        if ctx.wants_keyboard_input() || ctx.wants_pointer_input() {
            return;
        }
    }

    if keys.just_pressed(KeyCode::KeyW) {
        tool.selected_tool = TransformToolMode::Translate;
    } else if keys.just_pressed(KeyCode::KeyE) {
        tool.selected_tool = TransformToolMode::Rotate;
    } else if keys.just_pressed(KeyCode::KeyR) {
        tool.selected_tool = TransformToolMode::Scale;
    }
}

/// Optional standalone panel renderer (not currently used).
#[allow(dead_code)]
fn render_panel(mut contexts: EguiContexts, mut tool: ResMut<TransformTool>) {
    let ctx = contexts.ctx_mut().expect("to get primary egui context");

    egui::TopBottomPanel::top("transform_tools_panel")
        .resizable(false)
        .exact_height(32.0)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                render_toolbar(ui, &mut tool);
                ui.add_space(ui.available_width());
            });
        });
}
