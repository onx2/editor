use bevy::{app::App, ecs::resource::Resource, ecs::system::Res};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

#[derive(Resource)]
pub struct AssetBrowserUiState {
    pub visible: bool,
}

impl Default for AssetBrowserUiState {
    fn default() -> Self {
        Self { visible: true }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<AssetBrowserUiState>();
    // Render panels in the egui pass schedule so the pass state is initialized.
    app.add_systems(EguiPrimaryContextPass, render);
}

fn render(mut contexts: EguiContexts, ui_state: Res<AssetBrowserUiState>) {
    if !ui_state.visible {
        return;
    }

    let ctx = contexts.ctx_mut().expect("to get primary egui context");

    egui::TopBottomPanel::bottom("bottom_asset_browser")
        .resizable(true)
        .default_height(220.0)
        .min_height(64.0)
        .max_height(480.0)
        .show(ctx, |ui| {
            let header_h = 28.0;
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), header_h),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.heading("Asset Browser");
                },
            );

            ui.separator();

            let content_h = ui.available_height();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .min_scrolled_height(content_h)
                .show(ui, |ui| {
                    // Placeholder content; replace with your asset list/grid later
                    ui.label("Assets will go here...");
                });
        });
}
