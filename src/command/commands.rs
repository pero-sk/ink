use std::path::PathBuf;

use super::ast::Arg;
use crate::clipboard::Clipboard;
use crate::document::Document;
use crate::warn::WarnPopup;

pub struct ExecContext<'a> {
    pub doc: &'a mut Document,
    pub clipboard: &'a mut Clipboard,
    pub warn: &'a mut WarnPopup,
    pub should_quit: &'a mut bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Save,
    Quit,
    Open,
    Find,
    Replace,
    GotoLine,
    Undo,
    Redo,
    Execute,
    Copy,
    Paste,
    Delete,
}

impl CommandKind {
    pub fn from_char(c: char) -> Option<Self> {
        use CommandKind::*;
        Some(match c {
            's' => Save,
            'q' => Quit,
            'e' => Open,
            'f' => Find,
            'R' => Replace,
            'g' => GotoLine,
            'u' => Undo,
            'r' => Redo,
            'x' => Execute,
            'c' => Copy,
            'p' => Paste,
            'd' => Delete,
            _ => return None,
        })
    }

    pub fn help(&self) -> &'static str {
        use CommandKind::*;
        match self {
            Save => "[s] save the current document (s! to override read-only)",
            Quit => "[q] quit (q! to force quit with unsaved changes)",
            Open => "[e]{path} open/edit a file",
            Find => "[f]{find} find next occurrence of find, wrapping to the start if needed",
            Replace => "[R]{find}{replace} replace the next occurrence of find with replace",
            GotoLine => "[g]{N} go to line N",
            Undo => "[u] undo",
            Redo => "[r] redo",
            Execute => "[x]{command} execute a shell command",
            Copy => "[c] copy current line to the system clipboard",
            Paste => "[p] paste from the system clipboard",
            Delete => "[d];w; delete word forward from the cursor. ;l; delete to end of line from the cursor",
        }
    }

    pub fn run(&self, args: &[Arg], forced: bool, ctx: &mut ExecContext) {
        use CommandKind::*;
        match self {
            Save => {
                let result = if forced { ctx.doc.save_forced() } else { ctx.doc.save() };
                if let Err(e) = result {
                    ctx.warn.show(e.to_string());
                }
            }
            Quit => {
                if ctx.doc.dirty && !forced {
                    ctx.warn.show("unsaved changes. use q! to force quit");
                } else {
                    *ctx.should_quit = true;
                }
            }
            Open => match args.first() {
                Some(arg) => match Document::open(PathBuf::from(arg.as_str()), forced) {
                    Ok(new_doc) => *ctx.doc = new_doc,
                    Err(e) => ctx.warn.show(format!("open failed: {e}")),
                },
                None => ctx.warn.show("e requires a path argument, e.g. e;file.txt;"),
            },
            Find => match args.first() {
                Some(needle) => {
                    if !ctx.doc.find_next(needle.as_str()) {
                        ctx.warn.show(format!("not found: {}", needle.as_str()));
                    }
                }
                None => ctx.warn.show("f requires a search term, e.g. f;needle;"),
            },
            Replace => match (args.first(), args.get(1)) {
                (Some(needle), Some(replacement)) => {
                    let _ = forced;
                    if !ctx.doc.replace_next(needle.as_str(), replacement.as_str()) {
                        ctx.warn.show(format!("not found: {}", needle.as_str()));
                    }
                }
                _ => ctx.warn.show("R requires two arguments, e.g. R;needle:replacement;"),
            },
            GotoLine => match args.first().and_then(|a| a.as_str().parse::<usize>().ok()) {
                Some(line) => ctx.doc.goto_line(line),
                None => ctx.warn.show("g requires a line number, e.g. g;42;"),
            },
            Undo => ctx.doc.undo(),
            Redo => ctx.doc.redo(),
            Execute => match args.first() {
                Some(cmdline) => {
                    match std::process::Command::new("sh").arg("-c").arg(cmdline.as_str()).output() {
                        Ok(output) => ctx.warn.show(summarise_output(&output)),
                        Err(e) => ctx.warn.show(format!("failed to run command: {e}")),
                    }
                }
                None => ctx.warn.show("x requires a command, e.g. x;python3 main.py;"),
            },

            Copy => {
                let text = ctx.doc.lines.get(ctx.doc.cursor_line).cloned().unwrap_or_default();
                if let Err(e) = ctx.clipboard.copy(&text) {
                    ctx.warn.show(e);
                }
            }
            Paste => match ctx.clipboard.paste() {
                Ok(text) => {
                    for c in text.chars() {
                        if c == '\n' {
                            ctx.doc.insert_newline();
                        } else {
                            ctx.doc.insert_char(c);
                        }
                    }
                }
                Err(e) => ctx.warn.show(e),
            },
            Delete => match args.first().map(|a| a.as_str()) {
                Some("w") => ctx.doc.delete_word_forward(),
                Some("l") => ctx.doc.delete_to_end_of_line(),
                Some(other) => ctx.warn.show(format!("unknown delete target '{other}' (use w or l)")),
                None => ctx.warn.show("d requires a target, e.g. d;w; or d;l;"),
            },
        }
    }
}

fn summarise_output(output: &std::process::Output) -> String {
    const MAX_LEN: usize = 200;
 
    let mut msg = if output.status.success() {
        String::from_utf8_lossy(&output.stdout).trim().replace('\n', " \u{2502} ")
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().replace('\n', " \u{2502} ");
        let code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".to_string());
        if stderr.is_empty() {
            format!("exited {code}")
        } else {
            format!("exited {code}: {stderr}")
        }
    };
 
    if msg.is_empty() {
        msg = "(no output)".to_string();
    }
    if msg.chars().count() > MAX_LEN {
        msg = msg.chars().take(MAX_LEN).collect::<String>() + "...";
    }
    msg
}
