mod file_menu;
mod view_menu;

use bevy::{app::App, app::AppExit, ecs::message::MessageWriter, ecs::system::ResMut};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use crate::ui::asset_browser::AssetBrowserUiState;
use crate::ui::performance::PerformanceUiState;
use crate::ui::transform_tools::ActiveTransformTool;

pub(super) fn plugin(app: &mut App) {
    // Render panels in the egui pass schedule so the pass state is initialized.
    app.add_systems(EguiPrimaryContextPass, render);
}

fn render(
    mut contexts: EguiContexts,
    exit: MessageWriter<AppExit>,
    perf_ui: ResMut<PerformanceUiState>,
    asset_browser_ui: ResMut<AssetBrowserUiState>,
    grid_enabled: ResMut<crate::infinite_grid::InfiniteGridEnabled>,
    mut active_tool: ResMut<ActiveTransformTool>,
) {
    let ctx = contexts.ctx_mut().expect("to get primary egui context");

    egui::TopBottomPanel::top("top_app_bar")
        .resizable(false)
        .exact_height(32.0)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    file_menu::render(ui, exit);
                    view_menu::render(ui, perf_ui, asset_browser_ui, grid_enabled);

                    ui.separator();
                    crate::ui::transform_tools::render_toolbar(ui, &mut active_tool);

                    // Fill the rest of the bar so it visually spans the full width.
                    ui.add_space(ui.available_width());
                });
            });

            // Prevent the panel from collapsing to minimal height in some layouts.
            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
        });
}
