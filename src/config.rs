use std::fs;
use std::path::PathBuf;

use crossterm::style::Color;

use crate::terminal::Theme;

/// Loads ~/.inkrc (or $INK_CONFIG if set) and applies any recognized settings on top of Theme::default().
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

fn config_path() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("INK_CONFIG") {
        return Some(PathBuf::from(custom));
    }
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".inkrc"))
}