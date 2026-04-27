use eframe::egui;
use rusqlite::Connection;

use crate::backend::{self, SpeakOptions};
use crate::config;
use crate::db;

const VOLUME_PRESETS: &[f32] = &[0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0];

pub(crate) struct TtsSettings {
    backends: Vec<String>,
    backend_keys: Vec<String>,
    backend_idx: usize,
    voices: Vec<String>,
    voice_idx: usize,
    languages: Vec<&'static str>,
    lang_idx: usize,
    styles: Vec<&'static str>,
    style_idx: usize,
    volume_idx: usize,
}

impl TtsSettings {
    pub(crate) fn load(prefs: &db::Preferences) -> Self {
        let (backends, backend_keys): (Vec<String>, Vec<String>) =
            backend::available_backends().into_iter().unzip();

        let current_backend = prefs.backend.as_deref().unwrap_or(config::DEFAULT_BACKEND);
        let backend_idx = backend_keys
            .iter()
            .position(|k| k == current_backend)
            .unwrap_or(0);

        let languages: Vec<&'static str> = config::SUPPORTED_LANGS.to_vec();
        let lang_idx = prefs
            .lang
            .as_deref()
            .and_then(|l| languages.iter().position(|x| *x == l))
            .unwrap_or(0);

        let styles = vec![
            "(default)",
            "calm",
            "energetic",
            "warm",
            "authoritative",
            "cheerful",
            "serious",
        ];
        let style_idx = prefs
            .style
            .as_deref()
            .and_then(|s| styles.iter().position(|x| *x == s))
            .unwrap_or(0);

        let voices = load_voices(&backend_keys[backend_idx]);
        let voice_idx = prefs
            .voice
            .as_deref()
            .and_then(|v| voices.iter().position(|x| x == v))
            .unwrap_or(0);

        let volume_idx = VOLUME_PRESETS
            .iter()
            .position(|&x| (x - 1.0).abs() < 0.001)
            .unwrap_or(2);

        Self {
            backends,
            backend_keys,
            backend_idx,
            voices,
            voice_idx,
            languages,
            lang_idx,
            styles,
            style_idx,
            volume_idx,
        }
    }

    pub(crate) fn save(&self, conn: &Connection) -> anyhow::Result<()> {
        db::set_preference(conn, "backend", self.selected_backend())?;
        db::set_preference(conn, "lang", self.languages[self.lang_idx])?;
        if let Some(v) = self.selected_voice() {
            db::set_preference(conn, "voice", v)?;
        }
        if let Some(s) = self.selected_style() {
            db::set_preference(conn, "style", s)?;
        }
        Ok(())
    }

    pub(crate) fn test_speak(&self) -> String {
        let backend = self.selected_backend().to_string();
        let voice = self.selected_voice().map(String::from);
        let lang = self.languages[self.lang_idx].to_string();
        let style = self.selected_style().map(String::from);
        let volume = VOLUME_PRESETS[self.volume_idx];

        let text = match lang.as_str() {
            "fr" => "Bonjour, ceci est un test de synthèse vocale.",
            "es" => "Hola, esta es una prueba de síntesis de voz.",
            "de" => "Hallo, dies ist ein Test der Sprachsynthese.",
            "ja" => "こんにちは、これは音声合成のテストです。",
            "zh" => "你好，这是语音合成测试。",
            _ => "Hello, this is a voice synthesis test.",
        }
        .to_string();

        std::thread::spawn(move || {
            let opts = SpeakOptions {
                voice,
                lang: Some(lang),
                style,
                volume,
                ..Default::default()
            };
            if let Ok(b) = backend::get_backend(&backend) {
                let _ = b.speak(&text, &opts);
            }
        });

        format!("Speaking with {}...", self.selected_backend())
    }

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("TTS Settings").strong().size(14.0));
        ui.add_space(6.0);

        egui::Grid::new("tts_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .min_col_width(90.0)
            .show(ui, |ui| {
                ui.label("Backend:");
                egui::ComboBox::from_id_salt("backend")
                    .selected_text(&self.backends[self.backend_idx])
                    .width(340.0)
                    .show_ui(ui, |ui| {
                        for (i, name) in self.backends.iter().enumerate() {
                            if ui
                                .selectable_value(&mut self.backend_idx, i, name)
                                .clicked()
                            {
                                self.voices = load_voices(&self.backend_keys[i]);
                                self.voice_idx = 0;
                            }
                        }
                    });
                ui.end_row();

                ui.label("Voice:");
                let voice_label = self
                    .voices
                    .get(self.voice_idx)
                    .cloned()
                    .unwrap_or_else(|| "(default)".into());
                egui::ComboBox::from_id_salt("voice")
                    .selected_text(&voice_label)
                    .width(340.0)
                    .show_ui(ui, |ui| {
                        for (i, v) in self.voices.iter().enumerate() {
                            ui.selectable_value(&mut self.voice_idx, i, v);
                        }
                    });
                ui.end_row();

                ui.label("Language:");
                egui::ComboBox::from_id_salt("language")
                    .selected_text(self.languages[self.lang_idx])
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        for (i, lang) in self.languages.iter().enumerate() {
                            ui.selectable_value(&mut self.lang_idx, i, *lang);
                        }
                    });
                ui.end_row();

                ui.label("Style:");
                egui::ComboBox::from_id_salt("style")
                    .selected_text(self.styles[self.style_idx])
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        for (i, s) in self.styles.iter().enumerate() {
                            ui.selectable_value(&mut self.style_idx, i, *s);
                        }
                    });
                ui.end_row();

                ui.label("Volume:");
                ui.horizontal(|ui| {
                    for (i, &v) in VOLUME_PRESETS.iter().enumerate() {
                        let label = format!("{v}x");
                        let selected = self.volume_idx == i;
                        if ui.selectable_label(selected, &label).clicked() {
                            self.volume_idx = i;
                        }
                    }
                });
                ui.end_row();
            });
    }

    fn selected_backend(&self) -> &str {
        &self.backend_keys[self.backend_idx]
    }

    fn selected_voice(&self) -> Option<&str> {
        let v = self.voices.get(self.voice_idx)?.as_str();
        if v.starts_with('(') { None } else { Some(v) }
    }

    fn selected_style(&self) -> Option<&str> {
        let s = self.styles[self.style_idx];
        if s == "(default)" { None } else { Some(s) }
    }
}

fn load_voices(backend_name: &str) -> Vec<String> {
    backend::get_backend(backend_name)
        .and_then(|b| b.list_voices())
        .unwrap_or_else(|_| vec!["(default)".into()])
}
