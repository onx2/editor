use bevy::ecs::system::ResMut;
use bevy_egui::egui::Ui;

use crate::infinite_grid::InfiniteGridEnabled;
use crate::ui::asset_browser::AssetBrowserUiState;
use crate::ui::performance::PerformanceUiState;

pub(super) fn render(
    ui: &mut Ui,
    mut perf_ui: ResMut<PerformanceUiState>,
    mut asset_browser_ui: ResMut<AssetBrowserUiState>,
    mut grid_enabled: ResMut<InfiniteGridEnabled>,
) {
    ui.menu_button("View", |ui| {
        if ui.button("Performance").clicked() {
            perf_ui.visible = !perf_ui.visible;
            ui.close();
        }

        if ui.button("Asset Browser").clicked() {
            asset_browser_ui.visible = !asset_browser_ui.visible;
            ui.close();
        }

        if ui.button("Grid").clicked() {
            grid_enabled.0 = !grid_enabled.0;
            ui.close();
        }
    });
}
