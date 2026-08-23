use std::io::Write;

/// Copy uses OSC 52 (tells the terminal itself to set the system
/// clipboard) plus arboard as a backup. Paste just uses arboard, with a
/// wl-paste/xclip/xsel fallback if arboard's format negotiation fails.
pub struct Clipboard {
    inner: Option<arboard::Clipboard>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self {
            inner: arboard::Clipboard::new().ok(),
        }
    }

    pub fn copy(&mut self, text: &str) -> Result<(), String> {
        let osc52_sent = write_osc52(text).is_ok();

        match &mut self.inner {
            Some(cb) => match cb.set_text(text.to_owned()) {
                Ok(()) => Ok(()),
                Err(e) if osc52_sent => {
                    // OSC 52 still went through, so don't treat this as a failure.
                    let _ = e;
                    Ok(())
                }
                Err(e) => Err(format!("clipboard copy failed: {e}")),
            },
            None if osc52_sent => Ok(()),
            None => Err(
                "no clipboard available (OSC 52 write failed and arboard has no backend)"
                    .to_string(),
            ),
        }
    }

    pub fn paste(&mut self) -> Result<String, String> {
        let arboard_result = match &mut self.inner {
            Some(cb) => cb
                .get_text()
                .map_err(|e| format!("clipboard paste failed: {e}")),
            None => Err("terminal has no clipboard. c/p is nop".to_string()),
        };

        match arboard_result {
            Ok(text) => Ok(text),
            Err(e) => shell_paste_fallback().ok_or(e),
        }
    }
}

/// Sends an OSC 52 escape code so the terminal sets the system clipboard.
/// Wrapped for tmux passthrough if we're running inside tmux.
fn write_osc52(text: &str) -> std::io::Result<()> {
    let encoded = base64_encode(text.as_bytes());
    let seq = format!("\x1b]52;c;{encoded}\x07");

    let seq = if std::env::var_os("TMUX").is_some() {
        let escaped = seq.replace('\x1b', "\x1b\x1b");
        format!("\x1bPtmux;{escaped}\x1b\\")
    } else {
        seq
    };

    let mut stdout = std::io::stdout();
    stdout.write_all(seq.as_bytes())?;
    stdout.flush()
}

/// Tries each clipboard CLI tool in turn; returns None if none are
/// installed or none work.
fn shell_paste_fallback() -> Option<String> {
    const CANDIDATES: &[(&str, &[&str])] = &[
        ("wl-paste", &["--no-newline"]),
        ("xclip", &["-selection", "clipboard", "-o"]),
        ("xsel", &["--clipboard", "--output"]),
    ];

    for (cmd, args) in CANDIDATES {
        if let Ok(output) = std::process::Command::new(cmd).args(*args).output() {
            if output.status.success() {
                return Some(String::from_utf8_lossy(&output.stdout).into_owned());
            }
        }
    }
    None
}

/// Basic base64 encoder, since OSC 52 needs the text base64-encoded.
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);

        out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}
