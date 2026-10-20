//! The set of configuration sections shown in the sidebar.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    General,
    Engine,
    Hotkey,
    Audio,
    Output,
    Text,
    Vad,
    Meeting,
    Notifications,
    Osd,
    Waybar,
    Advanced,
}

impl Section {
    /// Sidebar order: grouped by the path a dictation takes, from setup
    /// through recording to the transcript and the status around it.
    pub const ALL: &'static [Section] = &[
        Section::General,
        Section::Engine,
        Section::Hotkey,
        Section::Audio,
        Section::Vad,
        Section::Output,
        Section::Text,
        Section::Notifications,
        Section::Osd,
        Section::Waybar,
        Section::Meeting,
        Section::Advanced,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::Engine => "Engine",
            Section::Hotkey => "Hotkey",
            Section::Audio => "Audio",
            Section::Output => "Output",
            Section::Text => "Text",
            Section::Vad => "VAD",
            Section::Meeting => "Meeting",
            Section::Notifications => "Notifications",
            Section::Osd => "OSD",
            Section::Waybar => "Waybar",
            Section::Advanced => "Advanced",
        }
    }

    /// Sidebar group heading this section sits under.
    pub const fn group(self) -> &'static str {
        match self {
            Section::General | Section::Engine => "Setup",
            Section::Hotkey | Section::Audio | Section::Vad => "Recording",
            Section::Output | Section::Text => "Transcript",
            Section::Notifications | Section::Osd | Section::Waybar => "Status",
            Section::Meeting | Section::Advanced => "More",
        }
    }

    /// One-line description of what the section covers, shown in the help
    /// overlay.
    pub const fn summary(self) -> &'static str {
        match self {
            Section::General => "Engine, variant binary, daemon status",
            Section::Engine => "Engine + model + per-engine tuning",
            Section::Hotkey => "Push-to-talk key, mode, modifier, cancel key",
            Section::Audio => "Input device, max duration, feedback, MPRIS",
            Section::Output => "Mode, driver order, post-processing, profiles",
            Section::Text => "Spoken punctuation, replacements",
            Section::Vad => "Silero VAD, energy thresholds, eager processing",
            Section::Meeting => "Meeting mode: audio source, diarization, summary",
            Section::Notifications => "Desktop notifications and expire times",
            Section::Osd => "Floating waveform OSD: frontend, position, size, opacity",
            Section::Waybar => "Status integration: icon theme, overrides",
            Section::Advanced => "GPU isolation, flash attention, on-demand loading",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sidebar draws a heading whenever the group changes, so a group
    /// split across two runs would show its heading twice.
    #[test]
    fn each_group_is_one_contiguous_run() {
        let mut seen: Vec<&str> = Vec::new();
        for section in Section::ALL {
            let group = section.group();
            if seen.last() != Some(&group) {
                assert!(!seen.contains(&group), "{group} appears twice");
                seen.push(group);
            }
        }
    }
}
