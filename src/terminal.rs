use std::io::{self, Write};

use crossterm::{
    cursor, execute, queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use unicode_width::UnicodeWidthStr;

use crate::warn::WarnPopup;
use crate::{document::Document, editor::Editor};

#[derive(Clone, Copy)]
pub struct Theme {
    pub text: Color,
    pub tilde: Color,

    pub buffer_bar_background: Color,
    pub buffer_active_background: Color,
    pub buffer_active_foreground: Color,
    pub buffer_inactive_foreground: Color,

    pub status_background: Color,
    pub status_foreground: Color,

    pub command_prefix: Color,
    pub command_text: Color,

    pub warning_background: Color,
    pub warning_foreground: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            text: Color::White,
            tilde: Color::DarkCyan,

            buffer_bar_background: Color::DarkGrey,
            buffer_active_background: Color::DarkCyan,
            buffer_active_foreground: Color::White,
            buffer_inactive_foreground: Color::Grey,

            status_background: Color::DarkCyan,
            status_foreground: Color::White,

            command_prefix: Color::DarkCyan,
            command_text: Color::White,

            warning_background: Color::DarkCyan,
            warning_foreground: Color::White,
        }
    }
}

pub struct Screen {
    stdout: io::Stdout,
    pub width: u16,
    pub height: u16,
    scroll_top: usize,
    pub theme: Theme,
}

impl Screen {
    pub fn init(theme: Theme) -> io::Result<Self> {
        terminal::enable_raw_mode()?;

        let mut stdout = io::stdout();

        execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

        let (width, height) = terminal::size()?;

        Ok(Self {
            stdout,
            width,
            height,
            scroll_top: 0,
            theme,
        })
    }

    pub fn refresh_size(&mut self) -> io::Result<()> {
        let (w, h) = terminal::size()?;
        self.width = w;
        self.height = h;
        Ok(())
    }

    fn text_rows(&self) -> u16 {
        self.height.saturating_sub(3)
    }

    fn text_start_row(&self) -> u16 {
        1
    }

    pub fn draw(
        &mut self,
        editor: &Editor,
        command_bar: Option<&str>,
        warn: &WarnPopup,
    ) -> io::Result<()> {
        queue!(
            self.stdout,
            cursor::Hide,
            cursor::MoveTo(0, 0),
            Clear(ClearType::All),
        )?;

        self.draw_buffer_bar(editor)?;

        if let Some(w) = warn.render_text() {
            queue!(
                self.stdout,
                cursor::MoveTo(0, 0),
                SetForegroundColor(self.theme.warning_foreground),
                SetBackgroundColor(self.theme.warning_background),
                Print(truncate_to_width(&w, self.width as usize)),
                ResetColor,
            )?;
        }

        let doc = editor.doc();
        self.ensure_cursor_visible(doc);

        let start = self.text_start_row();
        let rows = self.text_rows();

        for i in 0..rows {
            let row = start + i;

            queue!(self.stdout, cursor::MoveTo(0, row))?;

            let line_index = self.scroll_top + i as usize;

            if let Some(line) = doc.lines.get(line_index) {
                let truncated = truncate_to_width(line, self.width as usize);

                queue!(
                    self.stdout,
                    SetForegroundColor(self.theme.text),
                    Print(truncated),
                    ResetColor,
                )?;
            } else {
                queue!(
                    self.stdout,
                    SetForegroundColor(self.theme.tilde),
                    Print("~"),
                    ResetColor,
                )?;
            }
        }

        self.draw_status_bar(doc)?;

        let bottom_row = self.height.saturating_sub(1);

        queue!(self.stdout, cursor::MoveTo(0, bottom_row))?;

        if let Some(cmd) = command_bar {
            queue!(
                self.stdout,
                SetForegroundColor(self.theme.command_prefix),
                Print(":"),
                SetForegroundColor(self.theme.command_text),
                Print(cmd),
                ResetColor,
            )?;
        }

        if let Some(cmd) = command_bar {
            let col = 1 + UnicodeWidthStr::width(cmd);

            queue!(
                self.stdout,
                cursor::MoveTo(col as u16, bottom_row),
                cursor::Show,
            )?;
        } else {
            queue!(
                self.stdout,
                cursor::MoveTo(
                    doc.cursor_col as u16,
                    start + (doc.cursor_line - self.scroll_top) as u16,
                ),
                cursor::Show,
            )?;
        }

        self.stdout.flush()
    }

    fn draw_buffer_bar(&mut self, editor: &Editor) -> io::Result<()> {
        queue!(
            self.stdout,
            cursor::MoveTo(0, 0),
            SetBackgroundColor(self.theme.buffer_bar_background),
            SetForegroundColor(self.theme.buffer_active_foreground),
        )?;

        let mut col = 0usize;

        for (index, doc) in editor.documents.iter().enumerate() {
            let name = doc
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("[no name]");

            let dirty = if doc.dirty { "*" } else { "" };

            let label = format!(" {}{} ", name, dirty);
            let width = UnicodeWidthStr::width(label.as_str());

            if col + width > self.width as usize {
                break;
            }

            if index == editor.active {
                queue!(
                    self.stdout,
                    SetBackgroundColor(self.theme.buffer_active_background),
                    SetForegroundColor(self.theme.buffer_active_foreground),
                    Print(&label),
                    SetBackgroundColor(self.theme.buffer_bar_background),
                )?;
            } else {
                queue!(
                    self.stdout,
                    SetForegroundColor(self.theme.buffer_inactive_foreground),
                    Print(&label),
                )?;
            }

            col += width;
        }

        if col < self.width as usize {
            queue!(self.stdout, Print(" ".repeat(self.width as usize - col)),)?;
        }

        queue!(self.stdout, ResetColor)?;

        Ok(())
    }

    fn ensure_cursor_visible(&mut self, doc: &Document) {
        let cursor_line = doc.cursor_line;
        let rows = self.text_rows() as usize;

        if rows == 0 {
            self.scroll_top = cursor_line;
            return;
        }

        if cursor_line < self.scroll_top {
            self.scroll_top = cursor_line;
        }

        if cursor_line >= self.scroll_top + rows {
            self.scroll_top = cursor_line - rows + 1;
        }
    }

    fn draw_status_bar(&mut self, doc: &Document) -> io::Result<()> {
        let status_row = self.height.saturating_sub(2);

        queue!(
            self.stdout,
            cursor::MoveTo(0, status_row),
            SetForegroundColor(self.theme.status_foreground),
            SetBackgroundColor(self.theme.status_background),
        )?;

        let name = doc
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("[no name]");

        let dirty = if doc.dirty { "*" } else { "" };
        let ro = if doc.read_only { " [ro]" } else { "" };

        let status = format!(
            "{name}{dirty}{ro} -- {}:{}",
            doc.cursor_line + 1,
            doc.cursor_col + 1
        );

        queue!(
            self.stdout,
            Print(truncate_to_width(&status, self.width as usize)),
            ResetColor,
        )?;

        Ok(())
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, cursor::Show, terminal::LeaveAlternateScreen,);

        let _ = terminal::disable_raw_mode();
    }
}

fn truncate_to_width(s: &str, max_width: usize) -> String {
    let mut out = String::new();
    let mut w = 0;

    for c in s.chars() {
        let cw = UnicodeWidthStr::width(c.to_string().as_str());

        if w + cw > max_width {
            break;
        }

        out.push(c);
        w += cw;
    }

    out
}
