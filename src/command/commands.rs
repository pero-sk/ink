use std::path::PathBuf;

use super::ast::Arg;
use crate::clipboard::Clipboard;
use crate::document::Document;
use crate::editor::Editor;
use crate::warn::WarnPopup;

pub struct ExecContext<'a> {
    pub editor: &'a mut Editor,
    pub clipboard: &'a mut Clipboard,
    pub warn: &'a mut WarnPopup,
    pub should_quit: &'a mut bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Save,
    Quit,
    Exit,
    Edit,
    Find,
    Replace,
    Goto,
    Undo,
    Redo,
    Execute,
    Copy,
    Paste,
    Delete,
    Match,
    PreviousFile,
    NextFile,
    Name,
    Yank,
}

impl CommandKind {
    pub fn from_char(c: char) -> Option<Self> {
        use CommandKind::*;
        Some(match c {
            's' => Save,
            'q' => Quit,
            'Q' => Exit,
            'e' => Edit,
            'f' => Find,
            'R' => Replace,
            'g' => Goto,
            'u' => Undo,
            'r' => Redo,
            'x' => Execute,
            'c' => Copy,
            'p' => Paste,
            'd' => Delete,
            'm' => Match,
            'A' => PreviousFile,
            'D' => NextFile,
            'n' => Name,
            'y' => Yank,
            _ => return None,
        })
    }

    pub fn help(&self) -> &'static str {
        use CommandKind::*;
        match self {
            Save => {
                "[s] save the current document (s! to override read-only, might need root privilages)"
            }
            Quit => "[q] close file (q! to force close with unsaved changes)",
            Exit => "[Q] quit ink",
            Edit => "[e]{path} open/edit a file",
            Find => "[f]{find} find next occurrence of find, wrapping to the start if needed",
            Replace => "[R]{find}{replace} replace the next occurrence of find with replace",
            Goto => {
                "[g]{N/arg} go to line N (or: sl-startOfLine, el-endOfLine, sf-startOfFile, ef-endOfFile)"
            }
            Undo => "[u] undo",
            Redo => "[r] redo",
            Execute => "[x]{command} execute a shell command",
            Copy => "[c] copy current line to the system clipboard",
            Paste => "[p] paste from the system clipboard",
            Delete => "[d] delete to end of line from the cursor",
            Match => "[m] match to the relevant delimiter",
            PreviousFile => "[A] go to the previous file in the editor",
            NextFile => "[D] go to the next file in the editor",
            Name => "[n] set the name of the currently selected file.",
            Yank => "[y] duplicate the current line",
        }
    }

    pub fn run(&self, args: &[Arg], forced: bool, ctx: &mut ExecContext) {
        use CommandKind::*;
        match self {
            Save => {
                let result = if forced {
                    ctx.editor.doc_mut().save_forced()
                } else {
                    ctx.editor.doc_mut().save()
                };
                if let Err(e) = result {
                    ctx.warn.show(e.to_string());
                }
            }
            Quit => {
                if ctx.editor.doc().dirty && !forced {
                    ctx.warn.show("unsaved changes. use q! to force quit");
                } else {
                    ctx.editor.close();

                    if ctx.editor.doc_len() == 0 {
                        *ctx.should_quit = true;
                    }
                }
            }
            Exit => {
                if ctx.editor.has_dirty() && !forced {
                    ctx.warn
                        .show("unsaved changes in a file. use Q! to force quit");
                } else {
                    *ctx.should_quit = true;
                }
            }
            Edit => match args.first() {
                Some(arg) => match Document::open(PathBuf::from(arg.as_str())) {
                    Ok(new_doc) => ctx.editor.open(new_doc),
                    Err(e) => ctx.warn.show(format!("open failed: {e}")),
                },

                None => {
                    ctx.editor.open(Document::new_empty());
                }
            },
            Find => match args.first() {
                Some(needle) => {
                    if !ctx.editor.doc_mut().find_next(needle.as_str()) {
                        ctx.warn.show(format!("not found: {}", needle.as_str()));
                    }
                }
                None => ctx.warn.show("f requires a search term, e.g. f;needle;"),
            },
            Replace => match (args.first(), args.get(1)) {
                (Some(needle), Some(replacement)) => {
                    let _ = forced;
                    if !ctx
                        .editor
                        .doc_mut()
                        .replace_next(needle.as_str(), replacement.as_str())
                    {
                        ctx.warn.show(format!("not found: {}", needle.as_str()));
                    }
                }
                _ => ctx
                    .warn
                    .show("R requires two arguments, e.g. R;needle:replacement;"),
            },
            Goto => match args.first().map(|a| a.as_str()) {
                Some("ef") => {
                    let doc = ctx.editor.doc_mut();

                    if let Some(line) = doc.lines.last() {
                        doc.cursor_line = doc.lines.len().saturating_sub(1);
                        doc.cursor_col = line.len();
                    }
                }

                Some("sf") => {
                    ctx.editor.doc_mut().goto_line(0);
                    ctx.editor.doc_mut().cursor_col = 0;
                }

                Some("el") => {
                    if let Some(line) = ctx.editor.doc().lines.get(ctx.editor.doc().cursor_line) {
                        ctx.editor.doc_mut().cursor_col = line.len();
                    }
                }

                Some("sl") => {
                    ctx.editor.doc_mut().cursor_col = 0;
                }

                Some(arg) => match arg.parse::<usize>() {
                    Ok(line) => ctx.editor.doc_mut().goto_line(line),
                    Err(_) => ctx
                        .warn
                        .show("g requires a line number or position, g;N; / g;ef/sf; g;el/sl;"),
                },

                None => ctx
                    .warn
                    .show("g requires a line number or position, g;N; / g;ef/sf; g;el/sl;"),
            },
            Undo => ctx.editor.doc_mut().undo(),
            Redo => ctx.editor.doc_mut().redo(),
            Execute => match args.first() {
                Some(cmdline) => {
                    match std::process::Command::new("sh")
                        .arg("-c")
                        .arg(cmdline.as_str())
                        .output()
                    {
                        Ok(output) => {
                            let mut text = String::new();

                            if !output.stdout.is_empty() {
                                text.push_str(&String::from_utf8_lossy(&output.stdout));
                            }

                            if !output.stderr.is_empty() {
                                if !text.is_empty() && !text.ends_with('\n') {
                                    text.push('\n');
                                }

                                text.push_str(&String::from_utf8_lossy(&output.stderr));
                            }

                            if !text.is_empty() {
                                let mut doc = Document::from_text("out", text);
                                doc.read_only = true;
                                ctx.editor.open(doc);
                            }
                        }

                        Err(e) => {
                            ctx.warn.show(format!("failed to run command: {e}"));
                        }
                    }
                }

                None => ctx.warn.show("x requires a command, e.g. x;cargo check;"),
            },
            Copy => {
                let text = ctx
                    .editor
                    .doc()
                    .lines
                    .get(ctx.editor.doc().cursor_line)
                    .cloned()
                    .unwrap_or_default();
                if let Err(e) = ctx.clipboard.copy(&text) {
                    ctx.warn.show(e);
                }
            }
            Paste => match ctx.clipboard.paste() {
                Ok(text) => ctx.editor.doc_mut().insert_text(&text),
                Err(e) => ctx.warn.show(e),
            },
            Delete => ctx.editor.doc_mut().delete_to_end_of_line(),
            Match => {
                ctx.editor.doc_mut().jump_to_match();
            }

            PreviousFile => ctx.editor.previous(),
            NextFile => ctx.editor.next(),

            Name => match args.first() {
                Some(arg) => {
                    ctx.editor.doc_mut().path = Some(PathBuf::from(arg.as_str()));
                }

                None => ctx.warn.show("n requires a filename, e.g. n;file.txt;"),
            },

            Yank => ctx.editor.doc_mut().duplicate_line(),
        }
    }
}