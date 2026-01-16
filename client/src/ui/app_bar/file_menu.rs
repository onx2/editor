use bevy::{app::AppExit, ecs::message::MessageWriter};
use bevy_egui::egui::Ui;

pub(super) fn render(ui: &mut Ui, mut exit: MessageWriter<AppExit>) {
    ui.menu_button("File", |ui| {
        if ui.button("New project").clicked() {
            ui.close();
        }
        if ui.button("Open project").clicked() {
            ui.close();
        }
        if ui.button("Export collision data").clicked() {
            ui.close();
        }

        ui.separator();

        if ui.button("Quit").clicked() {
            ui.close();
            exit.write(AppExit::Success);
        }
    });
}
