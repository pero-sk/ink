use std::fs;
use std::path::PathBuf;

use crossterm::style::Color;

use crate::terminal::Theme;

/// Loads ~/.inkrc (or $INK_CONFIG if set) and applies any recognized
/// settings on top of Theme::default(). No file, or an empty file, just
/// means "use the defaults" -- not an error. Bad lines don't stop parsing
/// or fall back to a blank theme; they're collected and returned as a
/// single warning message so the rest of the file still applies.
pub fn load_theme() -> (Theme, Option<String>) {
    let mut theme = Theme::default();

    let Some(path) = config_path() else {
        return (theme, None);
    };
    let Ok(content) = fs::read_to_string(&path) else {
        return (theme, None);
    };

    let mut errors = Vec::new();

    for (i, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            errors.push(format!("line {}: expected 'key = value'", i + 1));
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        match parse_color(value) {
            Some(color) => apply(&mut theme, key, color, i + 1, &mut errors),
            None => errors.push(format!("line {}: unknown color '{value}'", i + 1)),
        }
    }

    let warning = if errors.is_empty() {
        None
    } else {
        Some(format!("~/.inkrc: {}", errors.join("; ")))
    };
    (theme, warning)
}

fn apply(theme: &mut Theme, key: &str, color: Color, line: usize, errors: &mut Vec<String>) {
    match key {
        "text" => theme.text = color,
        "tilde" => theme.tilde = color,
        "buffer_bar_background" => theme.buffer_bar_background = color,
        "buffer_active_background" => theme.buffer_active_background = color,
        "buffer_active_foreground" => theme.buffer_active_foreground = color,
        "buffer_inactive_foreground" => theme.buffer_inactive_foreground = color,
        "status_background" => theme.status_background = color,
        "status_foreground" => theme.status_foreground = color,
        "command_prefix" => theme.command_prefix = color,
        "command_text" => theme.command_text = color,
        "warning_background" => theme.warning_background = color,
        "warning_foreground" => theme.warning_foreground = color,
        other => errors.push(format!("line {line}: unknown setting '{other}'")),
    }
}

/// `#RRGGBB` hex, or a named ANSI color (case-insensitive).
fn parse_color(s: &str) -> Option<Color> {
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        return Some(Color::Rgb { r, g, b });
    }

    Some(match s.to_ascii_lowercase().as_str() {
        "black" => Color::Black,
        "darkgrey" | "darkgray" => Color::DarkGrey,
        "red" => Color::Red,
        "darkred" => Color::DarkRed,
        "green" => Color::Green,
        "darkgreen" => Color::DarkGreen,
        "yellow" => Color::Yellow,
        "darkyellow" => Color::DarkYellow,
        "blue" => Color::Blue,
        "darkblue" => Color::DarkBlue,
        "magenta" => Color::Magenta,
        "darkmagenta" => Color::DarkMagenta,
        "cyan" => Color::Cyan,
        "darkcyan" => Color::DarkCyan,
        "white" => Color::White,
        "grey" | "gray" => Color::Grey,
        _ => return None,
    })
}

pub fn config_path() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("INK_CONFIG") {
        return Some(PathBuf::from(custom));
    }
    // HOME is set on Linux/macOS (and on Windows under Git Bash/MSYS/WSL),
    // but plain cmd.exe/PowerShell use USERPROFILE instead.
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    Some(PathBuf::from(home).join(".inkrc"))
}

/// Same path as a plain String, empty if it can't be resolved -- handed
/// to plugins as a constant (CONFIG_PATH).
pub fn config_path_string() -> String {
    config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

/// Validates `value` the same way the startup loader would, then
/// writes/replaces `key = value` in ~/.inkrc. Takes effect on the NEXT
/// restart, not live -- making it live would mean sharing the running
/// Screen's Theme as mutable state, which is a bigger change than this
/// warrants right now. Returns false (no write happens) on an unknown
/// key or color.
pub fn set_theme_value(key: &str, value: &str) -> bool {
    let mut probe = Theme::default();
    let mut errors = Vec::new();
    let Some(color) = parse_color(value) else {
        return false;
    };
    apply(&mut probe, key, color, 0, &mut errors);
    if !errors.is_empty() {
        return false;
    }

    let Some(path) = config_path() else {
        return false;
    };
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let mut found = false;
    let mut lines: Vec<String> = existing
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if !found && !trimmed.starts_with('#') {
                if let Some((k, _)) = trimmed.split_once('=') {
                    if k.trim() == key {
                        found = true;
                        return format!("{key} = {value}");
                    }
                }
            }
            line.to_string()
        })
        .collect();

    if !found {
        lines.push(format!("{key} = {value}"));
    }

    fs::write(&path, lines.join("\n") + "\n").is_ok()
}
