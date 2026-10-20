//! General settings screen: install info, daemon status, variant matrix.

use crate::setup::binary::{Acceleration, EngineFamily, InstallKind, Variant};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

use super::app::{Action, App, COLS, ROWS};
use super::common::{self, FeedbackLevel};
use super::theme;

/// Width of the label column in the Status pane.
const FACT_LABEL_WIDTH: usize = 9;

/// Width of the engine-family column in the variant matrix.
const FAMILY_COLUMN_WIDTH: usize = 9;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let (level, message) = status_message(app);
    let status_rows = match level {
        Some(level) => common::status_height(area.width, false, false, Some((level, &message))),
        None => 1,
    };
    let [body, status] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(status_rows)])
        .areas(area);
    render_status_line(f, status, level, &message);

    let facts = fact_lines(app, body.width.saturating_sub(4) as usize);
    let [info, rest] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(facts.len() as u16 + 2),
            Constraint::Min(0),
        ])
        .areas(body);
    let block = common::pane("Status", false);
    let inner = block.inner(info);
    f.render_widget(block, info);
    f.render_widget(Paragraph::new(facts), inner);

    let (matrix_area, about_area) = if rest.width >= common::SIDE_BY_SIDE_MIN_WIDTH {
        let [m, a] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(rest);
        (m, a)
    } else {
        let [m, a] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(matrix_height(app, rest.width)),
                Constraint::Min(0),
            ])
            .areas(rest);
        (m, a)
    };
    render_matrix(f, matrix_area, app);
    if about_area.height >= 3 {
        render_hint(f, about_area, app);
    }
}

/// What the line under General says: the outcome of the last switch,
/// download or daemon action, then a pending restart, then nothing to save.
fn status_message(app: &App) -> (Option<FeedbackLevel>, String) {
    if let Some((level, message)) = &app.notice {
        (Some(*level), message.clone())
    } else if let Some(outcome) = &app.last_switch {
        let level = if outcome.success {
            FeedbackLevel::Ok
        } else {
            FeedbackLevel::Err
        };
        (Some(level), outcome.message.clone())
    } else if app.restart_needed {
        (
            Some(FeedbackLevel::Warn),
            "The new binary isn't running yet: R restarts voxtype with it".to_string(),
        )
    } else if app.apply_pending.get() {
        (
            Some(FeedbackLevel::Warn),
            "Saved settings aren't in use yet: R restarts voxtype and its OSD".to_string(),
        )
    } else {
        (
            None,
            "Changes here apply at once; nothing to save.".to_string(),
        )
    }
}

fn render_status_line(f: &mut Frame, area: Rect, level: Option<FeedbackLevel>, message: &str) {
    match level {
        Some(level) => common::render_status_line(f, area, false, false, Some((level, message))),
        None => f.render_widget(
            Paragraph::new(Span::styled(
                common::truncate(&message, area.width.saturating_sub(2) as usize),
                theme::muted(),
            )),
            Rect {
                x: area.x + 2,
                width: area.width.saturating_sub(2),
                ..area
            },
        ),
    }
}

fn yes_no_mark(present: bool) -> Span<'static> {
    if present {
        Span::styled("✓", theme::ok())
    } else {
        Span::styled("✗", theme::muted())
    }
}

/// The Status pane: daemon, binary, variants and hardware as label/value
/// facts, plus the missing-model warning when there is one.
fn fact_lines(app: &App, width: usize) -> Vec<Line<'static>> {
    let inv = &app.inventory;
    let value_room = width.saturating_sub(FACT_LABEL_WIDTH + 2);
    let install_kind = match inv.install_kind {
        InstallKind::Package => "package",
        InstallKind::Source => "source build",
    };

    let daemon = if app.daemon_running {
        vec![Span::styled("● ", theme::ok()), Span::raw("running")]
    } else {
        vec![
            Span::styled("○ ", theme::muted()),
            Span::raw("stopped  "),
            Span::styled("D starts it", theme::muted()),
        ]
    };

    let binary = format!("{} ({})", inv.binary_path.display(), install_kind);
    let active = inv
        .active_variant
        .map(|v| format!("{} · {}", v.display(), v.binary_name()))
        .unwrap_or_else(|| "unknown (symlink missing or unrecognized)".to_string());
    let rec = &inv.recommendation;
    let best = format!("{} · {}", rec.whisper.display(), rec.onnx.display());

    let mut lines = vec![
        common::fact_line("Daemon", FACT_LABEL_WIDTH, daemon),
        common::fact_line(
            "Binary",
            FACT_LABEL_WIDTH,
            vec![Span::raw(common::truncate_middle(&binary, value_room))],
        ),
        common::fact_line(
            "Active",
            FACT_LABEL_WIDTH,
            vec![Span::raw(common::truncate(&active, value_room))],
        ),
        common::fact_line(
            "Best fit",
            FACT_LABEL_WIDTH,
            vec![
                Span::styled("★ ", theme::info()),
                Span::styled(
                    common::truncate(&best, value_room.saturating_sub(2)),
                    theme::info(),
                ),
            ],
        ),
        common::fact_line(
            "Hardware",
            FACT_LABEL_WIDTH,
            vec![
                Span::raw("AVX2 "),
                yes_no_mark(inv.cpu.avx2),
                Span::raw("  AVX-512 "),
                yes_no_mark(inv.cpu.avx512),
                Span::raw("  NVIDIA "),
                yes_no_mark(inv.gpus.nvidia),
                Span::raw("  AMD "),
                yes_no_mark(inv.gpus.amd),
            ],
        ),
    ];

    if let Some(missing) = &app.missing_model {
        let text = format!(
            "{} model {} not downloaded. M downloads it",
            missing.engine, missing.model
        );
        lines.push(common::fact_line(
            "Model",
            FACT_LABEL_WIDTH,
            vec![
                Span::styled("! ", theme::warn().add_modifier(Modifier::BOLD)),
                Span::styled(
                    common::truncate(&text, value_room.saturating_sub(2)),
                    theme::warn(),
                ),
            ],
        ));
    }
    lines
}

/// Width of each acceleration column for a matrix pane `inner` cells wide.
fn column_width(inner: usize) -> usize {
    (inner.saturating_sub(FAMILY_COLUMN_WIDTH) / COLS.len()).clamp(6, 11)
}

/// Rows the matrix pane needs: header, one row per family, a blank line,
/// and the legend wrapped to the pane.
fn matrix_height(app: &App, width: u16) -> u16 {
    if app.inventory.install_kind == InstallKind::Source {
        return 8;
    }
    let inner = width.saturating_sub(4).max(1) as usize;
    let legend = legend_lines(inner).len() as u16;
    1 + ROWS.len() as u16 + 1 + legend + 2
}

const LEGEND: &[&str] = &[
    "★ recommended",
    "● active",
    "✓ ready",
    "⚠ CPU/GPU mismatch",
    "· not installed",
    "— not built",
];

/// The legend wrapped between entries, never inside one.
fn legend_lines(width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for item in LEGEND {
        match lines.last_mut() {
            Some(line) if common::str_width(line) + 2 + common::str_width(item) <= width => {
                line.push_str("  ");
                line.push_str(item);
            }
            _ => lines.push(item.to_string()),
        }
    }
    lines
}

fn render_matrix(f: &mut Frame, area: Rect, app: &App) {
    let block = common::pane("Binary variant", app.content_focused());
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.inventory.install_kind == InstallKind::Source {
        let features = if app.inventory.compiled_features.is_empty() {
            "none".to_string()
        } else {
            app.inventory.compiled_features.join(", ")
        };
        let para = Paragraph::new(vec![
            Line::from(Span::styled("Source build", theme::heading())),
            Line::from(
                "Variant switching applies only to package installs \
                 (/usr/lib/voxtype/voxtype-*). To use another engine, rebuild \
                 with the matching Cargo features.",
            ),
            Line::from(vec![
                Span::styled("Compiled features: ", theme::muted()),
                Span::raw(features),
            ]),
        ])
        .wrap(Wrap { trim: true });
        f.render_widget(para, inner);
        return;
    }

    let col_w = column_width(inner.width as usize);
    let long_cells = col_w >= 10;
    let mut lines = Vec::new();

    let mut header = vec![Span::raw(" ".repeat(FAMILY_COLUMN_WIDTH))];
    for col in COLS {
        header.push(Span::styled(
            format!(
                "{:<w$}",
                common::truncate(accel_label(*col), col_w - 1),
                w = col_w
            ),
            theme::muted(),
        ));
    }
    lines.push(Line::from(header));

    for (r, family) in ROWS.iter().enumerate() {
        let mut spans = vec![Span::styled(
            format!("{:<w$}", family_label(*family), w = FAMILY_COLUMN_WIDTH),
            theme::heading(),
        )];
        for (c, _accel) in COLS.iter().enumerate() {
            let cell = common::truncate(&render_cell(app, r, c, long_cells), col_w - 1);
            let is_cursor = app.cursor == (r, c);
            let text = format!("{:<w$}", cell, w = col_w - 1);
            if is_cursor {
                spans.push(Span::styled(text, theme::selected()));
            } else {
                spans.push(Span::raw(text));
            }
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::from(""));
    for legend in legend_lines(inner.width as usize) {
        lines.push(Line::from(Span::styled(legend, theme::muted())));
    }

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_cell(app: &App, row: usize, col: usize, long: bool) -> String {
    let Some(variant) = app.variant_at(row, col) else {
        return "—".to_string();
    };

    let status = app.inventory.variants.iter().find(|s| s.variant == variant);

    let glyph = match status {
        Some(s) if s.active => {
            if long {
                "● active"
            } else {
                "●"
            }
        }
        Some(s) if !s.installed => "·",
        Some(s) if !s.runs_on_this_cpu => "⚠ CPU",
        Some(s) if !s.gpu_available => "⚠ GPU",
        Some(_) => "✓",
        None => "·",
    };

    if is_recommended(variant, &app.inventory.recommendation) {
        format!("★ {}", glyph)
    } else {
        glyph.to_string()
    }
}

fn is_recommended(v: Variant, r: &crate::setup::binary::Recommendation) -> bool {
    v == r.whisper || v == r.onnx
}

fn render_hint(f: &mut Frame, area: Rect, app: &App) {
    if app.inventory.install_kind == InstallKind::Source {
        let block = common::pane("About", false);
        f.render_widget(block, area);
        return;
    }

    let (r, c) = app.cursor;
    let lines: Vec<Line> = match app.variant_at(r, c) {
        Some(variant) => variant_hint_lines(variant, app),
        None => na_hint_lines(r, c),
    };
    common::render_guidance(f, area, app, lines);
}

fn variant_hint_lines<'a>(variant: Variant, app: &App) -> Vec<Line<'a>> {
    let hint = variant_hint(variant);
    let status = app.inventory.variants.iter().find(|s| s.variant == variant);

    let status_line = match status {
        Some(s) if s.active => Line::from(Span::styled("● Currently active", theme::ok())),
        Some(s) if !s.installed => Line::from(Span::styled(
            "· Not installed on this system",
            theme::muted(),
        )),
        Some(s) if !s.runs_on_this_cpu => Line::from(Span::styled(
            "⚠ Won't run: CPU lacks required instructions",
            theme::warn(),
        )),
        Some(s) if !s.gpu_available => Line::from(Span::styled(
            "⚠ Won't accelerate: required GPU not detected",
            theme::warn(),
        )),
        Some(_) => Line::from(Span::styled("✓ Ready to switch (Enter)", theme::info())),
        None => Line::from(""),
    };

    let rec = &app.inventory.recommendation;
    let mut lines: Vec<Line> = Vec::new();
    if variant == rec.whisper {
        lines.push(Line::from(Span::styled(
            "★ Recommended for Whisper on this hardware",
            theme::heading(),
        )));
        lines.push(Line::from(Span::styled(
            rec.whisper_reason.to_string(),
            theme::info(),
        )));
        lines.push(Line::from(""));
    }
    if variant == rec.onnx {
        lines.push(Line::from(Span::styled(
            "★ Recommended for ONNX engines on this hardware",
            theme::heading(),
        )));
        lines.push(Line::from(Span::styled(
            rec.onnx_reason.to_string(),
            theme::info(),
        )));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        hint.headline.to_string(),
        theme::heading(),
    )));
    lines.push(Line::from(""));
    for paragraph in hint.body {
        lines.push(Line::from(paragraph.to_string()));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(vec![
        Span::styled("Models:    ", theme::muted()),
        Span::raw(hint.models.to_string()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Speed:     ", theme::muted()),
        Span::raw(hint.speed.to_string()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Hardware:  ", theme::muted()),
        Span::raw(hint.hardware.to_string()),
    ]));

    // Only show concrete model picks on the recommended cells, where the user
    // is most likely to act on them. On non-recommended cells the static
    // `models:` line above is enough.
    if variant == rec.whisper || variant == rec.onnx {
        let models = recommended_models(variant);
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Recommended models",
            theme::heading(),
        )));
        lines.push(Line::from(vec![
            Span::styled("  English:   ", theme::muted()),
            Span::raw(models.english.to_string()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  European:  ", theme::muted()),
            Span::raw(models.european.to_string()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Asian:     ", theme::muted()),
            Span::raw(models.asian.to_string()),
        ]));
        if let Some(note) = models.note {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(note.to_string(), theme::muted())));
        }
    }

    lines.push(Line::from(""));
    lines.push(status_line);
    lines
}

fn na_hint_lines<'a>(row: usize, col: usize) -> Vec<Line<'a>> {
    let family = ROWS.get(row).copied();
    let accel = COLS.get(col).copied();
    let suggestion = match (family, accel) {
        (Some(EngineFamily::Whisper), Some(Acceleration::Cuda)) => {
            "For NVIDIA GPU acceleration with Whisper, use Vulkan — it covers \
             NVIDIA, AMD, and Intel GPUs in a single binary."
        }
        (Some(EngineFamily::Whisper), Some(Acceleration::Migraphx)) => {
            "For AMD GPU acceleration with Whisper, use Vulkan — voxtype's \
             whisper.cpp build uses Vulkan instead of ROCm."
        }
        (Some(EngineFamily::Onnx), Some(Acceleration::Vulkan)) => {
            "ONNX Runtime does not ship a Vulkan execution provider. Use \
             ONNX (CUDA) for NVIDIA, ONNX (MIGraphX) for AMD, or ONNX (AVX2/AVX-512) \
             for CPU."
        }
        _ => "This combination is not built.",
    };
    vec![
        Line::from(Span::styled(
            "Not applicable",
            theme::muted().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(suggestion.to_string()),
    ]
}

struct VariantHint {
    headline: &'static str,
    body: &'static [&'static str],
    models: &'static str,
    speed: &'static str,
    hardware: &'static str,
}

/// Recommended models for a given variant, broken out by language family.
/// Shown in the hint pane only when the cursor is on a *recommended* cell —
/// the goal is to give users a jumping-off point ("if I switch to this
/// binary, what should I download?") rather than a complete model catalog.
struct ModelRecommendations {
    english: &'static str,
    european: &'static str,
    asian: &'static str,
    /// Optional advice tied to the acceleration tier (e.g. prefer int8 on AVX2).
    note: Option<&'static str>,
}

fn recommended_models(v: Variant) -> ModelRecommendations {
    match v {
        // ---- Whisper family ----
        Variant::WhisperBaseline => ModelRecommendations {
            english: "base.en  (tiny.en if base is too slow)",
            european: "base  (small if you can tolerate the wait)",
            asian: "small  (CJK needs the larger model, and it will be slow here)",
            note: Some(
                "No AVX2, so this is the slowest CPU path voxtype has. Sizes \
                 above small are not practical.",
            ),
        },
        Variant::WhisperAvx2 | Variant::WhisperNative => ModelRecommendations {
            english: "small.en  (or base.en for low-power CPUs)",
            european: "small  (covers FR, DE, IT, ES, NL, PL, PT and more)",
            asian: "medium  (CJK accuracy improves a lot at medium+)",
            note: Some(
                "AVX2-only CPU: large-v3 will run but isn't realtime. Stick to \
                 small/medium unless you have a GPU.",
            ),
        },
        Variant::WhisperAvx512 => ModelRecommendations {
            english: "large-v3-turbo  (fast and accurate)",
            european: "large-v3-turbo  (strong on most EU languages)",
            asian: "large-v3  (better CJK than turbo for the same size)",
            note: Some(
                "AVX-512 makes large-v3-turbo practical on CPU; large-v3 is \
                 slower but more accurate on non-English.",
            ),
        },
        Variant::WhisperVulkan => ModelRecommendations {
            english: "large-v3-turbo",
            european: "large-v3-turbo  (FR, DE, IT, ES, NL, PL, PT, etc.)",
            asian: "large-v3  (CJK; turbo is slightly weaker on Asian languages)",
            note: Some(
                "GPU acceleration removes the size penalty; pick whichever \
                 model gives you the accuracy you need.",
            ),
        },

        // ---- ONNX family ----
        Variant::OnnxAvx2 | Variant::OnnxNative => ModelRecommendations {
            english: "parakeet-tdt-0.6b-v3-int8  (quantized; ~50% faster on CPU)",
            european: "dolphin-base  (multi-language CTC, dictation-tuned)",
            asian: "sensevoice-small  (zh, en, ja, ko, yue in one model)",
            note: Some(
                "On AVX2-only CPUs the int8 Parakeet variant is the practical \
                 default. Omnilingual is also viable but heavier; Cohere is \
                 the heaviest at ~3 GB but ranks #1 on the Open ASR \
                 Leaderboard.",
            ),
        },
        Variant::OnnxAvx512 => ModelRecommendations {
            english: "parakeet-tdt-0.6b-v3  (top of the Open ASR Leaderboard)",
            european: "omnilingual-300m  (1600 languages, including all of EU)",
            asian: "sensevoice-small  (zh, en, ja, ko, yue)",
            note: Some(
                "AVX-512 lets you run full-precision Parakeet at real-time \
                 speed without a GPU.",
            ),
        },
        Variant::OnnxCuda12 | Variant::OnnxCuda13 | Variant::OnnxCuda => ModelRecommendations {
            english: "parakeet-tdt-0.6b-v3",
            european: "omnilingual-300m  (1600 languages)",
            asian: "sensevoice-small  (zh/en/ja/ko/yue) or paraformer-zh for Chinese-only",
            note: Some(
                "CUDA inference is so fast on Parakeet that English dictation \
                 is effectively instantaneous; use the largest model that fits \
                 your VRAM.",
            ),
        },
        Variant::OnnxMigraphx => ModelRecommendations {
            english: "parakeet-tdt-0.6b-v3",
            european: "omnilingual-300m  (1600 languages)",
            asian: "sensevoice-small  (zh, en, ja, ko, yue)",
            note: Some(
                "MIGraphX execution provider is new and may not register on all \
                 driver versions; if you see ORT registration errors, fall back \
                 to ONNX (AVX-512) on CPU.",
            ),
        },
    }
}

fn variant_hint(v: Variant) -> VariantHint {
    match v {
        Variant::WhisperBaseline => VariantHint {
            headline: "Whisper on pre-AVX2 CPUs",
            body: &[
                "The floor build, for x86-64-v2 (Nehalem 2008 and newer). \
                 Every other x86_64 binary needs AVX2 and will SIGILL on \
                 these machines. Slower than the AVX2 build, which is the \
                 trade for running at all.",
            ],
            models: "tiny, base, small (larger models are impractical at this speed)",
            speed: "Slower than AVX2; tiny/base are the usable sizes",
            hardware: "Any x86-64 CPU without AVX2 (pre-Haswell, e.g. Ivy Bridge)",
        },
        Variant::WhisperAvx2 => VariantHint {
            headline: "Whisper on AVX2 CPUs",
            body: &[
                "Baseline Whisper build. Runs on any x86-64 CPU since ~2013 \
                 (Haswell/Excavator and newer). Pick this if your CPU lacks \
                 AVX-512 and you don't have a GPU worth using.",
            ],
            models: "tiny, base, small, medium, large-v3, large-v3-turbo (and .en variants)",
            speed: "Real-time on small/base; large-v3 is slow without a GPU",
            hardware: "Any x86-64 CPU with AVX2",
        },
        Variant::WhisperAvx512 => VariantHint {
            headline: "Whisper on AVX-512 CPUs",
            body: &[
                "Fastest CPU-only Whisper. Roughly 1.5-2x throughput over the \
                 AVX2 build on supported chips. Use this if you don't have a \
                 capable GPU but do have a recent Intel or AMD CPU.",
            ],
            models: "Same as AVX2; large-v3-turbo becomes practical for live use",
            speed: "Best CPU performance; ~1.5-2x AVX2",
            hardware: "Intel Tiger/Ice Lake+, AMD Zen 4+",
        },
        Variant::WhisperVulkan => VariantHint {
            headline: "Whisper with Vulkan GPU",
            body: &[
                "Vendor-agnostic GPU acceleration via Vulkan compute shaders. \
                 Works on NVIDIA, AMD, and Intel GPUs (including integrated \
                 graphics that support Vulkan).",
                "Best general-purpose pick for desktops and gaming laptops.",
            ],
            models: "All Whisper models; large-v3-turbo runs comfortably",
            speed: "5-10x CPU on a discrete GPU; falls back to CPU if Vulkan unavailable",
            hardware: "Any Vulkan 1.2 GPU; ~2 GB VRAM for large-v3",
        },
        Variant::WhisperNative => VariantHint {
            headline: "Whisper (source build)",
            body: &[
                "A locally compiled Whisper binary with whatever Cargo features \
                 you enabled. Reported when no specific tier suffix matches.",
            ],
            models: "Whatever your build supports",
            speed: "Depends on build flags (RUSTFLAGS, GPU features)",
            hardware: "Whatever you compiled for",
        },
        Variant::OnnxAvx2 => VariantHint {
            headline: "ONNX engines on AVX2 CPUs",
            body: &[
                "CPU inference for the ONNX Runtime engine family: Parakeet, \
                 Moonshine, SenseVoice, Paraformer, Dolphin, Omnilingual, and \
                 Cohere Transcribe.",
                "Pick this when you don't have a GPU but want a faster, more \
                 accurate alternative to Whisper.",
            ],
            models: "parakeet-tdt-0.6b-v3, moonshine-base/tiny, sense-voice-small, paraformer-zh, dolphin-base, omnilingual",
            speed: "Parakeet is ~2-3x faster than Whisper-large at higher accuracy",
            hardware: "Any x86-64 CPU with AVX2",
        },
        Variant::OnnxAvx512 => VariantHint {
            headline: "ONNX engines on AVX-512 CPUs",
            body: &[
                "Same engine set as ONNX (AVX2), built against a newer toolchain \
                 that takes advantage of AVX-512 where ONNX Runtime can use it.",
            ],
            models: "Same as ONNX (AVX2)",
            speed: "Modest gain over AVX2 build; ORT does runtime SIMD dispatch",
            hardware: "Intel Tiger/Ice Lake+, AMD Zen 4+",
        },
        Variant::OnnxCuda12 | Variant::OnnxCuda13 | Variant::OnnxCuda => VariantHint {
            headline: "ONNX engines on NVIDIA CUDA",
            body: &[
                "GPU inference via the CUDA execution provider. Best choice for \
                 anyone with a recent NVIDIA card running Parakeet or another \
                 ONNX engine.",
                "voxtype ships separate cu12 and cu13 binaries; pick the one \
                 matching your installed CUDA runtime. The unversioned variant \
                 is for source builds and pre-0.7.0 installs.",
                "Note: this binary bundles an ONNX Runtime built with AVX-512, \
                 so the CPU also needs AVX-512 to load it cleanly.",
            ],
            models: "Same as ONNX (AVX2)",
            speed: "10-20x AVX2 on capable GPUs; Parakeet faster than real-time even at large sizes",
            hardware: "NVIDIA GPU + matching CUDA 12.x or 13.x driver; AVX-512 CPU",
        },
        Variant::OnnxMigraphx => VariantHint {
            headline: "ONNX engines on AMD MIGraphX",
            body: &[
                "GPU inference for AMD GPUs via the MIGraphX execution provider \
                 (replaces the ROCm EP that was retired in voxtype 0.7.0).",
                "Note: this binary bundles an ONNX Runtime built with AVX-512, \
                 so the CPU also needs AVX-512 to load it cleanly. MIGraphX \
                 support is new — if the provider fails to register on your \
                 driver/card combo, fall back to ONNX (AVX-512) on CPU or \
                 switch the engine to Whisper (Vulkan).",
            ],
            models: "Same as ONNX (AVX2)",
            speed: "Comparable to CUDA on similarly-tier GPUs",
            hardware: "AMD GPU with MIGraphX-capable driver; AVX-512 CPU",
        },
        Variant::OnnxNative => VariantHint {
            headline: "ONNX engines (source build)",
            body: &[
                "Locally compiled ONNX engine binary with whatever Cargo \
                 features you enabled. Reported when no specific tier suffix \
                 matches.",
            ],
            models: "Whatever your build supports",
            speed: "Depends on build flags",
            hardware: "Whatever you compiled for",
        },
    }
}

fn family_label(f: EngineFamily) -> &'static str {
    match f {
        EngineFamily::Whisper => "Whisper",
        EngineFamily::Onnx => "ONNX",
    }
}

fn accel_label(a: Acceleration) -> &'static str {
    match a {
        Acceleration::Baseline => "baseline",
        Acceleration::Avx2 => "AVX2",
        Acceleration::Avx512 => "AVX-512",
        Acceleration::Vulkan => "Vulkan",
        Acceleration::Cuda => "CUDA",
        Acceleration::Migraphx => "MIGraphX",
        Acceleration::Native => "native",
    }
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('r') => {
            app.refresh_inventory();
            Action::None
        }
        // D predates the global R; it restarts at once, without asking.
        KeyCode::Char('D') => {
            app.last_switch = None;
            app.restart_daemon();
            Action::None
        }
        // Fetch the configured engine's missing model, the same download
        // the Engine section runs after a save.
        KeyCode::Char('M') => match &app.missing_model {
            Some(missing) => Action::DownloadModel {
                engine: missing.engine.clone(),
                model: missing.model.clone(),
            },
            None => Action::None,
        },
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_cursor(-1, 0);
            Action::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_cursor(1, 0);
            Action::None
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.move_cursor(0, -1);
            Action::None
        }
        KeyCode::Right | KeyCode::Char('l') => {
            app.move_cursor(0, 1);
            Action::None
        }
        KeyCode::Enter => match selected_actionable_variant(app) {
            Some(v) => Action::SwitchVariant(v),
            None => Action::None,
        },
        _ => Action::None,
    }
}

/// Variant under the cursor, but only if switching to it makes sense:
/// - exists in the matrix
/// - is installed
/// - runs on this CPU
/// - has a compatible GPU (if required)
/// - is not already active
fn selected_actionable_variant(app: &App) -> Option<Variant> {
    if app.inventory.install_kind == InstallKind::Source {
        return None;
    }
    let (r, c) = app.cursor;
    let v = app.variant_at(r, c)?;
    let status = app.inventory.variants.iter().find(|s| s.variant == v)?;
    if status.active || !status.installed || !status.runs_on_this_cpu || !status.gpu_available {
        return None;
    }
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legend_wraps_between_entries_never_inside_one() {
        for width in [20, 40, 56, 80, 200] {
            let lines = legend_lines(width);
            assert_eq!(lines.join("  "), LEGEND.join("  "), "width {width}");
            for line in &lines {
                assert!(
                    common::str_width(line) <= width || LEGEND.contains(&line.as_str()),
                    "{line:?} overflows {width}"
                );
            }
        }
    }
}
