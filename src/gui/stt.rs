use eframe::egui;
use rusqlite::Connection;

use crate::db;

const STT_THRESHOLD_PRESETS: &[f64] = &[0.5, 1.0, 2.0, 3.0, 5.0, 7.5, 10.0];
const STT_SILENCE_PRESETS: &[f64] = &[0.5, 0.8, 1.0, 1.5, 2.0, 3.0, 5.0];

pub(crate) struct SttSettings {
    threshold_idx: usize,
    silence_idx: usize,
    trim_silence: bool,
    auto_enter: bool,
}

impl SttSettings {
    pub(crate) fn load(prefs: &db::Preferences) -> Self {
        let threshold_idx = prefs
            .stt_threshold
            .and_then(|v| {
                STT_THRESHOLD_PRESETS
                    .iter()
                    .position(|&x| (x - v).abs() < 0.001)
            })
            .unwrap_or(2);
        let silence_idx = prefs
            .stt_silence
            .and_then(|v| {
                STT_SILENCE_PRESETS
                    .iter()
                    .position(|&x| (x - v).abs() < 0.001)
            })
            .unwrap_or(2);

        Self {
            threshold_idx,
            silence_idx,
            trim_silence: prefs.stt_trim_silence.unwrap_or(true),
            auto_enter: prefs.stt_auto_enter.unwrap_or(false),
        }
    }

    pub(crate) fn save(&self, conn: &Connection) -> anyhow::Result<()> {
        db::set_preference(
            conn,
            "stt_threshold",
            &STT_THRESHOLD_PRESETS[self.threshold_idx].to_string(),
        )?;
        db::set_preference(
            conn,
            "stt_silence",
            &STT_SILENCE_PRESETS[self.silence_idx].to_string(),
        )?;
        db::set_preference(
            conn,
            "stt_trim_silence",
            if self.trim_silence { "1" } else { "0" },
        )?;
        db::set_preference(
            conn,
            "stt_auto_enter",
            if self.auto_enter { "1" } else { "0" },
        )?;
        Ok(())
    }

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("STT Settings  (hear / always)")
                .strong()
                .size(14.0),
        );
        ui.add_space(6.0);

        egui::Grid::new("stt_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .min_col_width(90.0)
            .show(ui, |ui| {
                ui.label("Threshold:")
                    .on_hover_text("VAD noise threshold in % — lower = more sensitive");
                ui.horizontal(|ui| {
                    for (i, &v) in STT_THRESHOLD_PRESETS.iter().enumerate() {
                        let label = format!("{v}%");
                        if ui
                            .selectable_label(self.threshold_idx == i, &label)
                            .clicked()
                        {
                            self.threshold_idx = i;
                        }
                    }
                });
                ui.end_row();

                ui.label("Silence:")
                    .on_hover_text("Seconds of silence before stopping recording");
                ui.horizontal(|ui| {
                    for (i, &v) in STT_SILENCE_PRESETS.iter().enumerate() {
                        let label = format!("{v}s");
                        if ui.selectable_label(self.silence_idx == i, &label).clicked() {
                            self.silence_idx = i;
                        }
                    }
                });
                ui.end_row();

                ui.label("Trim silence:")
                    .on_hover_text("Remove leading/trailing silence from captured audio");
                ui.checkbox(&mut self.trim_silence, "");
                ui.end_row();

                ui.label("Auto enter:")
                    .on_hover_text("Press Enter automatically after pasting transcript");
                ui.checkbox(&mut self.auto_enter, "");
                ui.end_row();
            });
    }
}
