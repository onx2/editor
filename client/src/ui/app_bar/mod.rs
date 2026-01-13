mod file_menu;

use bevy::{app::AppExit, ecs::message::MessageWriter};
use bevy_egui::{EguiContexts, egui};

pub(super) fn render(mut contexts: EguiContexts, exit: MessageWriter<AppExit>) {
    let ctx = contexts.ctx_mut().expect("to get primary egui context");

    egui::TopBottomPanel::top("top_app_bar")
        .resizable(false)
        .show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                file_menu::render(ui, exit);

                // Fill the rest of the bar so it visually spans the full width.
                ui.add_space(ui.available_width());
            });

            // Prevent the panel from collapsing to minimal height in some layouts.
            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
        });
}
