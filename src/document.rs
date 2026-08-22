use std::fs;
use std::io;
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;

use unicode_segmentation::UnicodeSegmentation;

const INDENT_WIDTH: usize = 4;

pub struct Document {
    pub path: Option<PathBuf>,
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub dirty: bool,
    pub read_only: bool,

    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
}

#[derive(Clone)]
struct Snapshot {
    lines: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
}

impl Document {
    pub fn new_empty() -> Self {
        Self {
            path: None,
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            dirty: false,
            read_only: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn open(path: PathBuf) -> io::Result<Self> {
        let content = fs::read_to_string(&path).unwrap_or_default();
        let read_only = is_read_only(path.as_path());

        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(String::from).collect()
        };

        Ok(Self {
            path: Some(path),
            lines,
            cursor_line: 0,
            cursor_col: 0,
            dirty: false,
            read_only,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    pub fn from_text(name: impl Into<String>, text: String) -> Self {
        let mut doc = Self::new_empty();

        doc.path = Some(std::path::PathBuf::from(name.into()));
        doc.lines = text.lines().map(String::from).collect();

        if doc.lines.is_empty() {
            doc.lines.push(String::new());
        }

        doc.dirty = false;

        doc
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no path associated with this document; use n to set one",
            ));
        };
        if self.read_only {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "document is read-only (use s! to override)",
            ));
        }
        fs::write(path, self.lines.join("\n"))?;
        self.dirty = false;
        Ok(())
    }

    pub fn save_forced(&mut self) -> io::Result<()> {
        self.read_only = false;
        self.save()
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            lines: self.lines.clone(),
            cursor_line: self.cursor_line,
            cursor_col: self.cursor_col,
        }
    }

    pub fn indent(&mut self) {
        self.push_undo();

        let indent = " ".repeat(INDENT_WIDTH);

        self.lines[self.cursor_line].insert_str(0, &indent);
        self.cursor_col += INDENT_WIDTH;
        self.dirty = true;
    }

    pub fn dedent(&mut self) {
        self.push_undo();

        let line = &mut self.lines[self.cursor_line];

        let remove = line
            .chars()
            .take(INDENT_WIDTH)
            .take_while(|c| *c == ' ')
            .count();

        if remove > 0 {
            line.drain(..remove);
            self.cursor_col = self.cursor_col.saturating_sub(remove);
            self.dirty = true;
        }
    }

    fn push_undo(&mut self) {
        self.undo_stack.push(self.snapshot());
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) {
        if let Some(snap) = self.undo_stack.pop() {
            self.redo_stack.push(self.snapshot());
            self.restore(snap);
        }
    }

    pub fn redo(&mut self) {
        if let Some(snap) = self.redo_stack.pop() {
            self.undo_stack.push(self.snapshot());
            self.restore(snap);
        }
    }

    fn restore(&mut self, snap: Snapshot) {
        self.lines = snap.lines;
        self.cursor_line = snap.cursor_line;
        self.cursor_col = snap.cursor_col;
        self.dirty = true;
    }

    fn current_line_graphemes(&self) -> Vec<&str> {
        self.lines[self.cursor_line]
            .graphemes(true)
            .collect::<Vec<_>>()
    }

    pub fn insert_char(&mut self, c: char) {
        self.push_undo();
        self.insert_char_raw(c);
    }

    pub fn insert_newline(&mut self) {
        self.push_undo();
        self.insert_newline_raw();
    }

    fn insert_char_raw(&mut self, c: char) {
        let line = &mut self.lines[self.cursor_line];
        let byte_idx = grapheme_byte_index(line, self.cursor_col);
        line.insert(byte_idx, c);
        self.cursor_col += 1;
        self.dirty = true;
    }

    fn insert_newline_raw(&mut self) {
        let line = &mut self.lines[self.cursor_line];
        let byte_idx = grapheme_byte_index(line, self.cursor_col);
        let rest = line.split_off(byte_idx);
        self.lines.insert(self.cursor_line + 1, rest);
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.dirty = true;
    }

    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.push_undo();
        for c in text.chars() {
            if c == '\n' {
                self.insert_newline_raw();
            } else {
                self.insert_char_raw(c);
            }
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor_col == 0 && self.cursor_line == 0 {
            return;
        }
        self.push_undo();
        if self.cursor_col == 0 {
            let cur = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            let prev_len = self.current_line_graphemes().len();
            self.lines[self.cursor_line].push_str(&cur);
            self.cursor_col = prev_len;
        } else {
            let line = &mut self.lines[self.cursor_line];
            let graphemes: Vec<&str> = line.graphemes(true).collect();
            let start = grapheme_byte_index(line, self.cursor_col - 1);
            let end = grapheme_byte_index(line, self.cursor_col);
            let _ = graphemes;
            line.replace_range(start..end, "");
            self.cursor_col -= 1;
        }
        self.dirty = true;
    }

    pub fn delete(&mut self) {
        let len = self.current_line_graphemes().len();
        if self.cursor_col == len && self.cursor_line + 1 >= self.lines.len() {
            return;
        }
        self.push_undo();
        if self.cursor_col == len {
            let next = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next);
        } else {
            let line = &mut self.lines[self.cursor_line];
            let start = grapheme_byte_index(line, self.cursor_col);
            let end = grapheme_byte_index(line, self.cursor_col + 1);
            line.replace_range(start..end, "");
        }
        self.dirty = true;
    }

    pub fn move_word_left(&mut self) {
        if self.cursor_col == 0 {
            self.move_left();
            return;
        }
        let line = self.lines[self.cursor_line].clone();
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        let mut i = self.cursor_col;

        while i > 0 && is_space(graphemes[i - 1]) {
            i -= 1;
        }
        if i > 0 {
            if is_word_char(graphemes[i - 1]) {
                while i > 0 && is_word_char(graphemes[i - 1]) {
                    i -= 1;
                }
            } else {
                while i > 0 && !is_word_char(graphemes[i - 1]) && !is_space(graphemes[i - 1]) {
                    i -= 1;
                }
            }
        }
        self.cursor_col = i;
    }

    pub fn move_word_right(&mut self) {
        let len = self.current_line_graphemes().len();
        if self.cursor_col >= len {
            self.move_right();
            return;
        }
        let line = self.lines[self.cursor_line].clone();
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        let mut i = self.cursor_col;

        if is_word_char(graphemes[i]) {
            while i < graphemes.len() && is_word_char(graphemes[i]) {
                i += 1;
            }
        } else if !is_space(graphemes[i]) {
            while i < graphemes.len() && !is_word_char(graphemes[i]) && !is_space(graphemes[i]) {
                i += 1;
            }
        }
        while i < graphemes.len() && is_space(graphemes[i]) {
            i += 1;
        }
        self.cursor_col = i;
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.current_line_graphemes().len();
        }
    }

    pub fn move_right(&mut self) {
        let len = self.current_line_graphemes().len();
        if self.cursor_col < len {
            self.cursor_col += 1;
        } else if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_up(&mut self, n: usize) {
        self.cursor_line = self.cursor_line.saturating_sub(n);
        self.clamp_col();
    }

    pub fn move_down(&mut self, n: usize) {
        self.cursor_line = (self.cursor_line + n).min(self.lines.len().saturating_sub(1));
        self.clamp_col();
    }

    pub fn goto_line(&mut self, line_1_indexed: usize) {
        let target = line_1_indexed.saturating_sub(1);
        self.cursor_line = target.min(self.lines.len().saturating_sub(1));
        self.clamp_col();
    }

    fn clamp_col(&mut self) {
        let len = self.current_line_graphemes().len();
        if self.cursor_col > len {
            self.cursor_col = len;
        }
    }

    pub fn find_next(&mut self, needle: &str) -> bool {
        if needle.is_empty() {
            return false;
        }
        let start_line = self.cursor_line;
        let start_col = self.cursor_col;
        let n = self.lines.len();

        for offset in 0..=n {
            let line_idx = (start_line + offset) % n;
            let line = &self.lines[line_idx];
            let search_from_byte = if offset == 0 {
                grapheme_byte_index(line, start_col + 1)
            } else {
                0
            };
            if search_from_byte <= line.len() {
                if let Some(pos) = line[search_from_byte..].find(needle) {
                    let byte_idx = search_from_byte + pos;
                    self.cursor_line = line_idx;
                    self.cursor_col = byte_to_grapheme_index(line, byte_idx);
                    return true;
                }
            }
        }
        false
    }

    pub fn replace_next(&mut self, needle: &str, replacement: &str) -> bool {
        if !self.find_next(needle) {
            return false;
        }
        self.push_undo();
        let line = &mut self.lines[self.cursor_line];
        let start = grapheme_byte_index(line, self.cursor_col);
        let end = start + needle.len();
        line.replace_range(start..end, replacement);
        self.cursor_col = byte_to_grapheme_index(line, start + replacement.len());
        self.dirty = true;
        true
    }

    pub fn jump_to_match(&mut self) -> bool {
        const PAIRS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];

        let Some(cur) = self.char_at(self.cursor_line, self.cursor_col) else {
            return false;
        };
        let Some(&(open, close)) = PAIRS.iter().find(|(o, c)| *o == cur || *c == cur) else {
            return false;
        };
        let forward = cur == open;

        let mut depth: i32 = 0;
        let mut line_idx = self.cursor_line;
        let mut col_idx = self.cursor_col;

        loop {
            if forward {
                col_idx += 1;
                if col_idx >= self.line_len(line_idx) {
                    line_idx += 1;
                    if line_idx >= self.lines.len() {
                        return false;
                    }
                    col_idx = 0;
                }
            } else {
                if col_idx == 0 {
                    if line_idx == 0 {
                        return false;
                    }
                    line_idx -= 1;
                    col_idx = self.line_len(line_idx);
                    if col_idx == 0 {
                        continue;
                    }
                }
                col_idx -= 1;
            }

            let Some(ch) = self.char_at(line_idx, col_idx) else {
                continue;
            };

            if forward {
                if ch == open {
                    depth += 1;
                } else if ch == close {
                    if depth == 0 {
                        self.cursor_line = line_idx;
                        self.cursor_col = col_idx;
                        return true;
                    }
                    depth -= 1;
                }
            } else {
                if ch == close {
                    depth += 1;
                } else if ch == open {
                    if depth == 0 {
                        self.cursor_line = line_idx;
                        self.cursor_col = col_idx;
                        return true;
                    }
                    depth -= 1;
                }
            }
        }
    }

    fn line_len(&self, line_idx: usize) -> usize {
        self.lines[line_idx].graphemes(true).count()
    }

    fn char_at(&self, line_idx: usize, col_idx: usize) -> Option<char> {
        self.lines
            .get(line_idx)?
            .graphemes(true)
            .nth(col_idx)?
            .chars()
            .next()
    }

    pub fn delete_word_forward(&mut self) {
        self.push_undo();
        let line = self.lines[self.cursor_line].clone();
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        let start = self.cursor_col.min(graphemes.len());
        let mut end = start;

        let is_word_char = |g: &str| {
            g.chars()
                .next()
                .map(|c| c.is_alphanumeric() || c == '_')
                .unwrap_or(false)
        };
        let is_space = |g: &str| g.chars().all(|c| c.is_whitespace());

        if end < graphemes.len() && is_word_char(graphemes[end]) {
            while end < graphemes.len() && is_word_char(graphemes[end]) {
                end += 1;
            }
        } else if end < graphemes.len() && !is_space(graphemes[end]) {
            while end < graphemes.len()
                && !is_word_char(graphemes[end])
                && !is_space(graphemes[end])
            {
                end += 1;
            }
        }
        while end < graphemes.len() && is_space(graphemes[end]) {
            end += 1;
        }

        let start_byte = grapheme_byte_index(&line, start);
        let end_byte = grapheme_byte_index(&line, end);
        self.lines[self.cursor_line].replace_range(start_byte..end_byte, "");
        self.dirty = true;
    }

    pub fn delete_word_backward(&mut self) {
        if self.cursor_col == 0 {
            return;
        }

        self.push_undo();

        let line = self.lines[self.cursor_line].clone();
        let graphemes: Vec<&str> = line.graphemes(true).collect();

        let end = self.cursor_col.min(graphemes.len());
        let mut start = end;

        let is_word_char = |g: &str| {
            g.chars()
                .next()
                .map(|c| c.is_alphanumeric() || c == '_')
                .unwrap_or(false)
        };

        let is_space = |g: &str| g.chars().all(|c| c.is_whitespace());

        while start > 0 && is_space(graphemes[start - 1]) {
            start -= 1;
        }

        if start > 0 && is_word_char(graphemes[start - 1]) {
            while start > 0 && is_word_char(graphemes[start - 1]) {
                start -= 1;
            }
        } else {
            while start > 0
                && !is_word_char(graphemes[start - 1])
                && !is_space(graphemes[start - 1])
            {
                start -= 1;
            }
        }

        let start_byte = grapheme_byte_index(&line, start);
        let end_byte = grapheme_byte_index(&line, end);

        self.lines[self.cursor_line].replace_range(start_byte..end_byte, "");

        self.cursor_col = start;
        self.dirty = true;
    }

    pub fn delete_to_end_of_line(&mut self) {
        self.push_undo();
        let line = &mut self.lines[self.cursor_line];
        let start_byte = grapheme_byte_index(line, self.cursor_col);
        line.truncate(start_byte);
        self.dirty = true;
    }
    pub fn duplicate_line(&mut self) {
        self.push_undo();
        let current = self.lines[self.cursor_line].clone();
        self.lines.insert(self.cursor_line + 1, current);
        self.cursor_line += 1;
        self.dirty = true;
    }
}

fn grapheme_byte_index(s: &str, grapheme_idx: usize) -> usize {
    s.grapheme_indices(true)
        .nth(grapheme_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

fn byte_to_grapheme_index(s: &str, byte_idx: usize) -> usize {
    s.grapheme_indices(true)
        .take_while(|(i, _)| *i < byte_idx)
        .count()
}

fn is_word_char(g: &str) -> bool {
    g.chars()
        .next()
        .map(|c| c.is_alphanumeric() || c == '_')
        .unwrap_or(false)
}

fn is_space(g: &str) -> bool {
    g.chars().all(|c| c.is_whitespace())
}

#[cfg(unix)]
fn is_read_only(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match std::fs::metadata(path) {
        Ok(metadata) => metadata.permissions().mode() & 0o222 == 0,
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn is_read_only(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.permissions().readonly())
        .unwrap_or(false)
}
