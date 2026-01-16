use bevy::{app::App, ecs::resource::Resource, ecs::system::Res};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use crate::config::ClientRuntimeConfig;

fn list_asset_files(asset_root: &str) -> Result<Vec<String>, String> {
    fn walk_dir(
        dir: &std::path::Path,
        base: &std::path::Path,
        out: &mut Vec<String>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walk_dir(&path, base, out)?;
            } else if path.is_file() {
                // Prefer relative paths so the UI is stable across machines.
                let rel = path.strip_prefix(base).unwrap_or(&path);
                out.push(rel.to_string_lossy().to_string());
            }
        }
        Ok(())
    }

    let base = std::path::PathBuf::from(asset_root);
    if !base.exists() {
        return Err(format!("Asset root does not exist: {}", asset_root));
    }
    if !base.is_dir() {
        return Err(format!("Asset root is not a directory: {}", asset_root));
    }

    let mut files = Vec::new();
    walk_dir(&base, &base, &mut files).map_err(|e| format!("Failed to read assets: {e}"))?;
    files.sort();
    Ok(files)
}

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

fn render(
    mut contexts: EguiContexts,
    ui_state: Res<AssetBrowserUiState>,
    config: Option<Res<ClientRuntimeConfig>>,
) {
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
                    let asset_root = config
                        .as_ref()
                        .map(|c| c.asset_root_for_listing())
                        .unwrap_or_else(|| "assets".to_string());

                    ui.horizontal(|ui| {
                        ui.label("Asset root:");
                        ui.monospace(&asset_root);
                    });

                    ui.add_space(6.0);

                    match list_asset_files(&asset_root) {
                        Ok(files) => {
                            ui.label(format!("{} file(s)", files.len()));
                            ui.add_space(6.0);

                            for name in files {
                                ui.label(name);
                            }
                        }
                        Err(err) => {
                            ui.colored_label(egui::Color32::RED, err);
                            ui.add_space(6.0);
                            ui.label("Set EDITOR_ASSET_PATH to a valid directory, or ensure ./assets exists.");
                        }
                    }
                });
        });
}
