//! Native macOS GUI for vox configuration (egui + eframe).
//! Launched via `vox setup`. Opens a 680×520 window.

use anyhow::Result;
use eframe::egui;

use crate::backend::{self, SpeakOptions};
use crate::config;
use crate::db;

const VOLUME_PRESETS: &[f32] = &[0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0];
const STT_THRESHOLD_PRESETS: &[f64] = &[0.5, 1.0, 2.0, 3.0, 5.0, 7.5, 10.0];
const STT_SILENCE_PRESETS: &[f64] = &[0.5, 0.8, 1.0, 1.5, 2.0, 3.0, 5.0];

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
    // TTS
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
    // STT
    stt_threshold_idx: usize,
    stt_silence_idx: usize,
    stt_trim_silence: bool,
    stt_auto_enter: bool,
    // UI state
    status: String,
}

impl VoxApp {
    fn load() -> Result<Self> {
        let conn = db::open()?;
        let prefs = db::get_preferences(&conn)?;

        #[cfg(target_os = "macos")]
        let (backends, backend_keys): (Vec<String>, Vec<String>) = vec![
            (
                "say          ★★★  quality  ⚡ 3s".to_string(),
                "say".to_string(),
            ),
            (
                "piper        ★★   quality  ⚡ <1s  [Rust]".to_string(),
                "piper".to_string(),
            ),
            (
                "qwen-native  ★★★★ quality  ⚡ 12s  [Rust+Metal]".to_string(),
                "qwen-native".to_string(),
            ),
            (
                "voxtream     ★★★★★ quality ⚡ 170ms [CUDA]".to_string(),
                "voxtream".to_string(),
            ),
            (
                "qwen         ★★★★ quality  ⚡ 2s   [Python+MLX]".to_string(),
                "qwen".to_string(),
            ),
        ]
        .into_iter()
        .unzip();

        #[cfg(not(target_os = "macos"))]
        let (backends, backend_keys): (Vec<String>, Vec<String>) = vec![
            (
                "piper        ★★   quality  ⚡ <1s  [Rust]".to_string(),
                "piper".to_string(),
            ),
            (
                "qwen-native  ★★★★ quality  ⚡ 3s   [Rust+CUDA]".to_string(),
                "qwen-native".to_string(),
            ),
            (
                "voxtream     ★★★★★ quality ⚡ 170ms [CUDA]".to_string(),
                "voxtream".to_string(),
            ),
        ]
        .into_iter()
        .unzip();

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

        let stt_threshold_idx = prefs
            .stt_threshold
            .and_then(|v| {
                STT_THRESHOLD_PRESETS
                    .iter()
                    .position(|&x| (x - v).abs() < 0.001)
            })
            .unwrap_or(2);
        let stt_silence_idx = prefs
            .stt_silence
            .and_then(|v| {
                STT_SILENCE_PRESETS
                    .iter()
                    .position(|&x| (x - v).abs() < 0.001)
            })
            .unwrap_or(2);
        let stt_trim_silence = prefs.stt_trim_silence.unwrap_or(true);
        let stt_auto_enter = prefs.stt_auto_enter.unwrap_or(false);

        Ok(Self {
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
            stt_threshold_idx,
            stt_silence_idx,
            stt_trim_silence,
            stt_auto_enter,
            status: "Ready. Configure and save.".to_string(),
        })
    }

    fn selected_backend(&self) -> &str {
        &self.backend_keys[self.backend_idx]
    }

    fn selected_voice(&self) -> Option<&str> {
        let v = self.voices.get(self.voice_idx)?.as_str();
        if v.starts_with('(') {
            None
        } else {
            Some(v)
        }
    }

    fn selected_style(&self) -> Option<&str> {
        let s = self.styles[self.style_idx];
        if s == "(default)" {
            None
        } else {
            Some(s)
        }
    }

    fn save(&mut self) {
        match self.do_save() {
            Ok(()) => self.status = "✓ Preferences saved.".to_string(),
            Err(e) => self.status = format!("✗ Save failed: {e}"),
        }
    }

    fn do_save(&self) -> anyhow::Result<()> {
        let conn = db::open()?;
        db::set_preference(&conn, "backend", self.selected_backend())?;
        db::set_preference(&conn, "lang", self.languages[self.lang_idx])?;
        if let Some(v) = self.selected_voice() {
            db::set_preference(&conn, "voice", v)?;
        }
        if let Some(s) = self.selected_style() {
            db::set_preference(&conn, "style", s)?;
        }
        db::set_preference(
            &conn,
            "stt_threshold",
            &STT_THRESHOLD_PRESETS[self.stt_threshold_idx].to_string(),
        )?;
        db::set_preference(
            &conn,
            "stt_silence",
            &STT_SILENCE_PRESETS[self.stt_silence_idx].to_string(),
        )?;
        db::set_preference(
            &conn,
            "stt_trim_silence",
            if self.stt_trim_silence { "1" } else { "0" },
        )?;
        db::set_preference(
            &conn,
            "stt_auto_enter",
            if self.stt_auto_enter { "1" } else { "0" },
        )?;
        Ok(())
    }

    fn test_speak(&mut self) {
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

        self.status = format!("Speaking with {}...", backend);

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
    }
}

fn load_voices(backend_name: &str) -> Vec<String> {
    backend::get_backend(backend_name)
        .and_then(|b| b.list_voices())
        .unwrap_or_else(|_| vec!["(default)".into()])
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

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

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
                                    .selectable_label(self.stt_threshold_idx == i, &label)
                                    .clicked()
                                {
                                    self.stt_threshold_idx = i;
                                }
                            }
                        });
                        ui.end_row();

                        ui.label("Silence:")
                            .on_hover_text("Seconds of silence before stopping recording");
                        ui.horizontal(|ui| {
                            for (i, &v) in STT_SILENCE_PRESETS.iter().enumerate() {
                                let label = format!("{v}s");
                                if ui
                                    .selectable_label(self.stt_silence_idx == i, &label)
                                    .clicked()
                                {
                                    self.stt_silence_idx = i;
                                }
                            }
                        });
                        ui.end_row();

                        ui.label("Trim silence:")
                            .on_hover_text("Remove leading/trailing silence from captured audio");
                        ui.checkbox(&mut self.stt_trim_silence, "");
                        ui.end_row();

                        ui.label("Auto enter:")
                            .on_hover_text("Press Enter automatically after pasting transcript");
                        ui.checkbox(&mut self.stt_auto_enter, "");
                        ui.end_row();
                    });

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
