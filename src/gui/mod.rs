//! Native macOS GUI for vox configuration (egui + eframe).
//! Launched via `vox setup`. Opens a 680×520 window.

mod stt;
mod tts;

use anyhow::Result;
use eframe::egui;

use crate::db;

pub fn run() -> Result<()> {
    let app = VoxApp::load()?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("vox — Voice Configuration")
            .with_inner_size([680.0, 560.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native("vox setup", options, Box::new(|_cc| Ok(Box::new(app))))
        .map_err(|e| anyhow::anyhow!("GUI error: {e}"))
}

struct VoxApp {
    tts: tts::TtsSettings,
    stt: stt::SttSettings,
    status: String,
}

impl VoxApp {
    fn load() -> Result<Self> {
        let conn = db::open()?;
        let prefs = db::get_preferences(&conn)?;

        Ok(Self {
            tts: tts::TtsSettings::load(&prefs),
            stt: stt::SttSettings::load(&prefs),
            status: "Ready. Configure and save.".to_string(),
        })
    }

    fn save(&mut self) {
        match self.do_save() {
            Ok(()) => self.status = "✓ Preferences saved.".to_string(),
            Err(e) => self.status = format!("✗ Save failed: {e}"),
        }
    }

    fn do_save(&self) -> anyhow::Result<()> {
        let conn = db::open()?;
        self.tts.save(&conn)?;
        self.stt.save(&conn)?;
        Ok(())
    }

    fn test_speak(&mut self) {
        self.status = self.tts.test_speak();
    }
}

impl eframe::App for VoxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            ui.heading("🔊 vox — Voice Configuration");
            ui.add_space(8.0);
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(8.0);

                self.tts.ui(ui);

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                self.stt.ui(ui);

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("▶ Test Voice").clicked() {
                        self.test_speak();
                    }
                    ui.add_space(8.0);
                    if ui.button("💾 Save").clicked() {
                        self.save();
                    }
                    ui.add_space(8.0);
                    if ui.button("✕ Close").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.add_space(8.0);

                let color = if self.status.starts_with('✓') {
                    egui::Color32::from_rgb(80, 200, 120)
                } else if self.status.starts_with('✗') {
                    egui::Color32::from_rgb(220, 80, 80)
                } else {
                    egui::Color32::GRAY
                };
                ui.label(egui::RichText::new(&self.status).color(color).size(12.0));
                ui.add_space(8.0);
            });
        });
    }
}
