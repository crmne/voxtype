# `voxtype configure` design

This is the design contract for the configuration TUI. It covers how the
interface looks and behaves, not which settings exist; those live in the
section modules under `src/tui/` and in [CONFIGURATION.md](CONFIGURATION.md).
Change this file together with the code when the interaction changes.

## Promise

Open it, find a setting, change it, know whether it is saved. Every option
voxtype exposes stays reachable from the keyboard on an 80x24 terminal, and
the TUI never leaves you guessing which file it edits, which daemon will
read it, or what is still unsaved.

## Screen

```text
─ voxtype configure ─ ~/.config/voxtype/config.toml ──────── ● daemon 1.1.1 running ─
 ! warning banner, only while the binary cannot run the configured engine
╭ Sections ────────╮╭ Hotkey ───────────────────────────── ● 1 unsaved change ╮
│ Setup            ││   Built-in evdev listener  Off                          │
│ › General        ││ ● Push-to-talk key         SCROLLLOCK                   │
│   Engine         ││   Mode                     Push-to-talk (hold)          │
│ Recording        │╰─────────────────────────────────────────────────────────╯
│   Hotkey       ● │╭ About ──────────────────────────────────────────────────╮
│   ...            ││ explanation of the focused setting                      │
╰──────────────────╯╰─────────────────────────────────────────── PgDn more ───╯
                    ● Hotkey has unsaved changes. s saves them to ~/..., r reverts.
 ↑↓ setting   ←→ change   Enter edit   s save   r revert   Tab sections   ? keys
```

- **Header rule**: the product, the config file being edited (the `--config`
  path when one is given), and the running daemon's version. A daemon older
  than this TUI shows both versions in the warning color.
- **Sections**: grouped by the path a dictation takes: Setup (General,
  Engine), Recording (Hotkey, Audio, VAD), Transcript (Output, Text), Status
  (Notifications, OSD, Waybar), More (Meeting, Advanced). `›` marks the open
  section, `●` a section with unsaved edits. Headings drop out when the
  terminal is too short, never sections.
- **Section**: a form and an About pane. They sit side by side when the
  content area is at least 100 columns wide and stack below that, so labels
  never wrap. The form scrolls to keep the focused row visible and says how
  many rows are hidden. The About pane scrolls with PgUp/PgDn.
- **Status line**: under every section. It shows the last save or revert
  result (`✓`, `✗`, `!`), otherwise whether the section has unsaved edits,
  otherwise that everything is saved and where.
- **Footer**: keys for whatever has focus right now, most important first,
  trimmed from the end on narrow terminals. The unsaved sections are counted
  on the right.

## Saved versus unsaved

Edits live in memory until `s` saves the open section. A row changed since
the last save or load carries `●` in its gutter; the form title counts them;
the sidebar marks the section; the footer counts sections. `r` reverts the
section to the file. Quitting asks only when something is unsaved, names
what, and offers save, discard or keep editing. A save that fails while
quitting keeps the TUI open on the failing section with its error, instead of
quitting with the edit lost.

## Saved versus in use

The daemon, and the OSD it supervises as a child process, read the config
only when they start; neither watches the file. So a save is not in use
until voxtype restarts. While that is true the header says `! R to apply`,
the status line says what R does, and the footer offers `R apply`. R asks
first, saying what it restarts and the exact command
(`systemctl --user restart voxtype`), then reports the result and the
pending state clears. Quitting also restarts voxtype when a save is
pending. Every message names the key or the exact command; none says
"restart X" without saying how.

## Interaction

- One focus at a time: the section list or the open section. The focused
  pane has the accent border. Tab switches, Esc goes back to the list and
  then quits.
- Moving through the list opens sections as you go. Enter moves into one.
- In a form: ↑↓ choose, ←→ change, Enter types a value, Space toggles. Only
  the focused row shows the `◂ ▸` cue.
- While typing a value every key goes to the field, including `q` and Tab.
- `?` shows every key, scrollable on short terminals.
- Below 60x16 the TUI says it needs more room instead of drawing a broken
  screen.

## Visual language

- Colors come from the terminal theme only: default foreground and
  background, the 16 ANSI colors, and attributes. No RGB, no hard-coded white
  or black. Omarchy themes, light or dark, restyle it entirely.
- Muted text uses the dim attribute, not "bright black", which some themes
  set to the background.
- One accent (green) for focus and keys; yellow for unsaved and warnings; red
  for errors; cyan for recommendations. Headings are bold.
- Color is never the only signal: selection is reversed video, unsaved rows
  have `●`, statuses lead with a glyph.
- Values read the same everywhere: `On`/`Off` for switches and `Not set`
  (dimmed) for empty optional values. The config file still stores `true`,
  `false` and absent keys.
- Rounded panes, titles on the border, nothing drawn that does not carry
  information.

## Out of scope

The config system, the set of settings and their storage are unchanged. CLI
flags and config compatibility are unchanged.
