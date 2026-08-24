use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::AtomicI64;

use rhai::{Array, Dynamic, FnPtr, Map};

use rhai::plugin::*;

use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock, atomic::Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::editor::Editor;
use crate::warn::WarnPopup;

pub type Keymap = HashMap<String, (u64, usize, FnPtr)>;
pub type ChangeMap = HashMap<u64, (usize, FnPtr)>;

pub struct TimerState {
    pub document_id: u64,
    pub duration: Duration,
    pub next_fire: Instant,
    pub callback: FnPtr,
    pub plugin_index: usize,
}

#[derive(Clone)]
pub struct Ink {
    pub editor: Rc<RefCell<Editor>>,
    pub warn: Rc<RefCell<WarnPopup>>,
    pub active_keymap: Rc<RefCell<Option<Keymap>>>,
    pub current_plugin: Rc<RefCell<usize>>,
    pub timers: Rc<RefCell<HashMap<u64, TimerState>>>,
    pub next_timer_id: Rc<RefCell<u64>>,
    pub change_callbacks: Rc<RefCell<ChangeMap>>,
}

#[export_module]
pub mod ink_api {
    use crate::document::Document;

    use super::*;
    #[rhai_fn(global)]
    pub fn on_change(ink: &mut Ink, buffer_id: u64, callback: FnPtr) {
        let plugin_index = *ink.current_plugin.borrow();

        ink.change_callbacks
            .borrow_mut()
            .insert(buffer_id, (plugin_index, callback));
    }

    /// buffer's lines as an array of strings.
    #[rhai_fn(global, pure)]
    pub fn get_lines(ink: &mut Ink, id: u64) -> Array {
        ink.editor
            .borrow()
            .get_doc_by_id(id)
            .unwrap_or(&Document::new_empty())
            .lines
            .iter()
            .map(|l| Dynamic::from(l.clone()))
            .collect()
    }

    /// Replaces the whole buffer's content.
    #[rhai_fn(global)]
    pub fn set_lines(ink: &mut Ink, id: u64, lines: Array) {
        let mut editor = ink.editor.borrow_mut();
        let pdoc = editor.get_doc_by_id_mut(id);

        if let Some(doc) = pdoc {
            doc.lines = lines.into_iter().map(|d| d.to_string()).collect();

            if doc.lines.is_empty() {
                doc.lines.push(String::new());
            }

            doc.cursor_line = 0;
            doc.cursor_col = 0;
            doc.dirty = false;
        }
    }

    /// Current cursor position.
    /// Returns: #{"line": N, "col": N}
    #[rhai_fn(global, pure)]
    pub fn get_cursor(ink: &mut Ink) -> Map {
        let editor = ink.editor.borrow();
        let doc = editor.doc();

        let mut map = Map::new();
        map.insert("line".into(), (doc.cursor_line as i64).into());
        map.insert("col".into(), (doc.cursor_col as i64).into());

        map
    }

    /// Sets the current cursor position.
    #[rhai_fn(global)]
    pub fn set_cursor(ink: &mut Ink, line: i64, col: i64) {
        let mut editor = ink.editor.borrow_mut();
        let doc = editor.doc_mut();

        let max_line = doc.lines.len().saturating_sub(1);

        doc.cursor_line = (line.max(0) as usize).min(max_line);

        let line_len = doc.lines[doc.cursor_line].chars().count();

        doc.cursor_col = (col.max(0) as usize).min(line_len);
    }

    /// Display a warning popup.
    #[rhai_fn(global)]
    pub fn warn(ink: &mut Ink, message: &str) {
        ink.warn.borrow_mut().show(message.to_string());
    }

    /// Bind a key for the currently active buffer.
    ///
    /// The binding is automatically associated with:
    ///   - current buffer ID
    ///   - current plugin
    ///   - callback
    #[rhai_fn(global)]
    pub fn keymap(ink: &mut Ink, key: &str, callback: FnPtr) {
        let plugin_index = *ink.current_plugin.borrow();

        let buffer_id = {
            let editor = ink.editor.borrow();
            editor.doc().id
        };

        ink.active_keymap
            .borrow_mut()
            .get_or_insert_with(HashMap::new)
            .insert(key.to_string(), (buffer_id, plugin_index, callback));
    }

    /// Remove a keymap owned by the current plugin on the current buffer.
    /// Returns true if a binding was removed.
    #[rhai_fn(global)]
    pub fn remove_keymap(ink: &mut Ink, key: &str) -> bool {
        let plugin_index = *ink.current_plugin.borrow();

        let buffer_id = {
            let editor = ink.editor.borrow();
            editor.doc().id
        };

        let mut keymap = ink.active_keymap.borrow_mut();

        let Some(map) = keymap.as_mut() else {
            return false;
        };

        let Some((bound_buffer_id, bound_plugin_index, _)) = map.get(key) else {
            return false;
        };

        if *bound_buffer_id != buffer_id {
            return false;
        }

        if *bound_plugin_index != plugin_index {
            return false;
        }

        map.remove(key).is_some()
    }

    /// Open a file as a new buffer.
    /// Returns the stable buffer ID, or 0 on failure.
    #[rhai_fn(global)]
    pub fn open(ink: &mut Ink, path: &str) -> u64 {
        match Document::open(PathBuf::from(path)) {
            Ok(doc) => ink.editor.borrow_mut().open(doc),

            Err(e) => {
                ink.warn.borrow_mut().show(format!("open failed: {e}"));

                0
            }
        }
    }

    /// Create a new buffer.
    /// Returns the stable buffer ID.
    #[rhai_fn(global)]
    pub fn new_buffer(ink: &mut Ink, name: &str, readonly: bool) -> u64 {
        let mut editor = ink.editor.borrow_mut();

        let mut doc = Document::from_text(name, "".to_string());

        doc.read_only = readonly;

        editor.open(doc)
    }

    /// Return all buffers.
    /// Each Document contains its stable `id`.
    #[rhai_fn(global)]
    pub fn get_buffers(ink: &mut Ink) -> Vec<Document> {
        ink.editor.borrow().documents.clone()
    }

    #[rhai_fn(global)]
    pub fn get_buffer_amount(ink: &mut Ink) -> usize {
        ink.editor.borrow().get_docs_len()
    }

    /// Get a buffer by stable ID.
    #[rhai_fn(global)]
    pub fn get_buffer(ink: &mut Ink, id: u64) -> Option<Document> {
        ink.editor.borrow().get_doc_by_id(id).cloned()
    }

    /// Replace a buffer by stable ID.
    /// Returns false if the ID does not exist.
    #[rhai_fn(global)]
    pub fn set_buffer(ink: &mut Ink, id: u64, doc: Document) -> bool {
        ink.editor.borrow_mut().set_doc_by_id(id, doc)
    }

    /// Close a buffer by stable ID.
    /// Returns false if the ID does not exist.
    #[rhai_fn(global)]
    pub fn close_buffer(ink: &mut Ink, id: u64) -> bool {
        let mut editor = ink.editor.borrow_mut();

        let closed = editor.close_doc(id);

        if closed {
            if let Some(keymap) = ink.active_keymap.borrow_mut().as_mut() {
                keymap.retain(|_, (buffer_id, _, _)| *buffer_id != id);
            }
            ink.timers
                .borrow_mut()
                .retain(|_, timer| timer.document_id != id);
        }

        closed
    }

    /// Write `key = value` into ~/.inkrc.
    #[rhai_fn(global)]
    pub fn set_theme(key: &str, value: &str) -> bool {
        crate::config::set_theme_value(key, value)
    }

    /// Initialise a timer id.
    /// Returns timer_id
    #[rhai_fn(global)]
    pub fn timer_reserve(ink: &mut Ink) -> u64 {
        let timer_id = {
            let mut next_id = ink.next_timer_id.borrow_mut();
            let id = *next_id;
            *next_id += 1;
            id
        };
        timer_id
    }

    /// Starts a repeating timer linked to a document id.
    /// If the document is closed, the timer is automatically killed.
    /// Returns success.
    #[rhai_fn(global)]
    pub fn timer_start(
        ink: &mut Ink,
        timer_id: u64,
        document_id: u64,
        duration_in_secs: f64,
        callback: FnPtr,
    ) -> bool {
        let plugin_index = *ink.current_plugin.borrow();

        if duration_in_secs <= 0.0 || !duration_in_secs.is_finite() {
            return false;
        }

        let document_exists = {
            let editor = ink.editor.borrow();

            editor.documents.iter().any(|doc| doc.id == document_id)
        };

        if !document_exists {
            return false;
        }

        let duration = Duration::from_secs_f64(duration_in_secs);

        ink.timers.borrow_mut().insert(
            timer_id,
            TimerState {
                document_id,
                duration,
                next_fire: Instant::now() + duration,
                callback,
                plugin_index,
            },
        );

        return true;
    }

    /// Kills a timer by timer ID.
    /// Returns whether it was successfully killed.
    #[rhai_fn(global)]
    pub fn timer_kill(ink: &mut Ink, timer_id: u64) -> bool {
        ink.timers.borrow_mut().remove(&timer_id).is_some()
    }
}

#[export_module]
pub mod pathutils {
    use super::*;

    fn expand_tilde(path: &str) -> PathBuf {
        let cleaned = path.trim().trim_matches(';');

        if cleaned == "~" || cleaned.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                if cleaned == "~" {
                    return home;
                }
                return home.join(&cleaned[2..]);
            }
        }
        PathBuf::from(cleaned)
    }

    #[rhai_fn(global)]
    pub fn cwd() -> String {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    #[rhai_fn(global)]
    pub fn resolve(path: &str) -> String {
        let p = expand_tilde(path);
        if p.is_absolute() {
            return p.to_string_lossy().into_owned();
        }

        std::env::current_dir()
            .map(|cwd| cwd.join(&p).to_string_lossy().into_owned())
            .unwrap_or_else(|_| p.to_string_lossy().into_owned())
    }

    #[rhai_fn(global)]
    pub fn read_dir(path: &str) -> Array {
        let mut entries: Array = match std::fs::read_dir(path) {
            Ok(directory) => directory
                .filter_map(|entry| {
                    let entry = entry.ok()?;

                    Some(Dynamic::from(
                        entry.file_name().to_string_lossy().into_owned(),
                    ))
                })
                .collect(),

            Err(_) => return Array::new(),
        };

        entries.sort_by(|a, b| {
            a.clone()
                .into_string()
                .unwrap_or_default()
                .cmp(&b.clone().into_string().unwrap_or_default())
        });

        entries
    }

    #[rhai_fn(global)]
    pub fn join(base: &str, child: &str) -> String {
        let base_path = expand_tilde(base);
        let child_path = expand_tilde(child);

        if child_path.is_absolute() {
            return child_path.to_string_lossy().into_owned();
        }

        base_path.join(child_path).to_string_lossy().into_owned()
    }

    #[rhai_fn(global)]
    pub fn parent(path: &str) -> String {
        let p = PathBuf::from(path);

        match p.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_string_lossy().into_owned(),
            _ => std::path::MAIN_SEPARATOR.to_string(),
        }
    }

    #[rhai_fn(global)]
    pub fn filename(path: &str) -> String {
        PathBuf::from(path)
            .file_name()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    #[rhai_fn(global)]
    pub fn extension(path: &str) -> String {
        PathBuf::from(path)
            .extension()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    #[rhai_fn(global)]
    pub fn stem(path: &str) -> String {
        PathBuf::from(path)
            .file_stem()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    #[rhai_fn(global)]
    pub fn exists(path: &str) -> bool {
        PathBuf::from(path).exists()
    }

    #[rhai_fn(global)]
    pub fn is_file(path: &str) -> bool {
        PathBuf::from(path).is_file()
    }

    #[rhai_fn(global)]
    pub fn is_dir(path: &str) -> bool {
        PathBuf::from(path).is_dir()
    }

    #[rhai_fn(global)]
    pub fn canonicalize(path: &str) -> String {
        std::fs::canonicalize(path)
            .map(|p| {
                let s = p.to_string_lossy().into_owned();
                // Remove Windows UNC prefix \\?\ if present
                s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
            })
            .unwrap_or_else(|_| path.to_string())
    }
}

struct ProcessState {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    stdout: Arc<Mutex<Vec<u8>>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    exit_code: Arc<Mutex<Option<i32>>>,
}

static NEXT_PROCESS_ID: AtomicI64 = AtomicI64::new(1);

static PROCESSES: OnceLock<Mutex<HashMap<i64, Arc<ProcessState>>>> = OnceLock::new();

fn processes() -> &'static Mutex<HashMap<i64, Arc<ProcessState>>> {
    PROCESSES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn spawn_reader<R>(reader: R, output: Arc<Mutex<Vec<u8>>>)
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = reader;
        let mut buffer = [0u8; 4096];

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,

                Ok(n) => {
                    output.lock().unwrap().extend_from_slice(&buffer[..n]);
                }

                Err(_) => break,
            }
        }
    });
}

#[export_module]
pub mod processutils {
    use std::io::Write;
    /// Start a shell process.
    /// Returns the process ID, or 0 on failure.
    #[rhai_fn(global)]
    pub fn start(cmd: &str) -> i64 {
        let mut child = match Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,

            Err(_) => return 0,
        };

        let stdin = match child.stdin.take() {
            Some(stdin) => stdin,
            None => return 0,
        };
        let stdout = Arc::new(Mutex::new(Vec::new()));
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let exit_code = Arc::new(Mutex::new(None));

        if let Some(stdout_pipe) = child.stdout.take() {
            spawn_reader(stdout_pipe, Arc::clone(&stdout));
        }

        if let Some(stderr_pipe) = child.stderr.take() {
            spawn_reader(stderr_pipe, Arc::clone(&stderr));
        }

        let state = Arc::new(ProcessState {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            stdout: Arc::clone(&stdout),
            stderr: Arc::clone(&stderr),
            exit_code: Arc::clone(&exit_code),
        });

        let id = NEXT_PROCESS_ID.fetch_add(1, Ordering::Relaxed);

        processes().lock().unwrap().insert(id, Arc::clone(&state));

        // Wait for the process in the background.
        thread::spawn(move || {
            let result = state.child.lock().unwrap().wait();

            let code = match result {
                Ok(status) => status.code(),
                Err(_) => None,
            };

            *state.exit_code.lock().unwrap() = code;
        });

        id
    }

    /// Poll a process.
    ///
    /// Returns the complete output accumulated so far.
    #[rhai_fn(global)]
    pub fn poll(id: i64) -> Map {
        let mut result = Map::new();

        let state = {
            let processes = processes().lock().unwrap();

            match processes.get(&id) {
                Some(state) => Arc::clone(state),
                None => {
                    result.insert("exists".into(), false.into());
                    return result;
                }
            }
        };

        let stdout = {
            let output = state.stdout.lock().unwrap();
            String::from_utf8_lossy(&output).into_owned()
        };

        let stderr = {
            let output = state.stderr.lock().unwrap();
            String::from_utf8_lossy(&output).into_owned()
        };

        let exit_code = *state.exit_code.lock().unwrap();

        result.insert("exists".into(), true.into());
        result.insert("stdout".into(), stdout.into());
        result.insert("stderr".into(), stderr.into());
        result.insert("running".into(), exit_code.is_none().into());

        match exit_code {
            Some(code) => {
                result.insert("exit_code".into(), (code as i64).into());
            }

            None => {
                result.insert("exit_code".into(), Dynamic::UNIT);
            }
        }

        result
    }

    /// Write to a process.
    /// Returns true if the data was successfully written
    #[rhai_fn(global)]
    pub fn write(id: i64, data: &str) -> bool {
        let state = {
            let processes = processes().lock().unwrap();

            match processes.get(&id) {
                Some(state) => Arc::clone(state),
                None => return false,
            }
        };

        let mut stdin = state.stdin.lock().unwrap();

        if stdin.write_all(data.as_bytes()).is_err() {
            return false;
        }

        stdin.flush().is_ok()
    }

    #[rhai_fn(global)]
    pub fn flush_stdout(id: i64) -> String {
        let state = {
            let processes = processes().lock().unwrap();

            match processes.get(&id) {
                Some(state) => Arc::clone(state),
                None => return String::new(),
            }
        };

        let mut output = state.stdout.lock().unwrap();

        let result = String::from_utf8_lossy(&output).into_owned();

        output.clear();

        result
    }

    /// Kill a process.
    /// Returns true if the process was successfully killed.
    #[rhai_fn(global)]
    pub fn kill(id: i64) -> bool {
        let state = {
            let processes = processes().lock().unwrap();

            match processes.get(&id) {
                Some(state) => Arc::clone(state),
                None => return false,
            }
        };

        let mut child = state.child.lock().unwrap();

        match child.try_wait() {
            Ok(Some(_)) => false,

            Ok(None) => child.kill().is_ok(),

            Err(_) => false,
        }
    }
}

#[export_module]
pub mod jsonutils {
    use super::*;
    use serde_json::{Map as JsonMap, Value};

    fn dynamic_to_json(value: Dynamic) -> Option<Value> {
        if value.is::<()>() {
            return Some(Value::Null);
        }

        if let Some(value) = value.clone().try_cast::<bool>() {
            return Some(Value::Bool(value));
        }

        if let Some(value) = value.clone().try_cast::<i64>() {
            return Some(Value::Number(value.into()));
        }

        if let Some(value) = value.clone().try_cast::<f64>() {
            return serde_json::Number::from_f64(value).map(Value::Number);
        }

        if let Some(value) = value.clone().try_cast::<String>() {
            return Some(Value::String(value));
        }

        if let Some(value) = value.clone().try_cast::<Array>() {
            let mut result = Vec::new();

            for item in value {
                result.push(dynamic_to_json(item)?);
            }

            return Some(Value::Array(result));
        }

        if let Some(value) = value.try_cast::<Map>() {
            let mut result = JsonMap::new();

            for (key, value) in value {
                result.insert(key.to_string(), dynamic_to_json(value)?);
            }

            return Some(Value::Object(result));
        }

        None
    }

    fn json_to_dynamic(value: Value) -> Dynamic {
        match value {
            Value::Null => Dynamic::UNIT,

            Value::Bool(value) => value.into(),

            Value::Number(value) => {
                if let Some(value) = value.as_i64() {
                    value.into()
                } else if let Some(value) = value.as_f64() {
                    value.into()
                } else {
                    Dynamic::UNIT
                }
            }

            Value::String(value) => value.into(),

            Value::Array(value) => value
                .into_iter()
                .map(json_to_dynamic)
                .collect::<Array>()
                .into(),

            Value::Object(value) => {
                let mut map = Map::new();

                for (key, value) in value {
                    map.insert(key.into(), json_to_dynamic(value));
                }

                map.into()
            }
        }
    }

    /// Parse a JSON string into a Rhai value.
    #[rhai_fn(global)]
    pub fn parse(json: &str) -> Map {
        let mut result = Map::new();

        match serde_json::from_str::<serde_json::Value>(json) {
            Ok(value) => {
                result.insert("success".into(), true.into());
                result.insert("value".into(), json_to_dynamic(value));
            }

            Err(e) => {
                result.insert("success".into(), false.into());
                result.insert("error".into(), e.to_string().into());
            }
        }

        result
    }

    /// Convert a Rhai value into JSON.
    ///
    /// Returns an empty string if the value cannot be represented as JSON.
    #[rhai_fn(global)]
    pub fn stringify(value: Dynamic) -> String {
        match dynamic_to_json(value) {
            Some(value) => serde_json::to_string(&value).unwrap_or_default(),
            None => String::new(),
        }
    }
}

#[export_module]
pub mod configutils {
    use std::path::{Component, PathBuf};

    fn config_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|home| home.join(".ink").join("configs"))
    }

    /// Resolve a config name inside ~/.ink/configs.
    ///
    fn config_path(name: &str) -> Option<PathBuf> {
        let name = name.trim();

        if name.is_empty() {
            return None;
        }

        let path = PathBuf::from(name);

        // Don't allow absolute paths.
        if path.is_absolute() {
            return None;
        }

        // Don't allow paths that can escape ~/.ink/configs.
        for component in path.components() {
            match component {
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return None;
                }
                _ => {}
            }
        }

        Some(config_dir()?.join(path))
    }

    #[rhai_fn(global)]
    pub fn exists(name: &str) -> bool {
        let Some(path) = config_path(name) else {
            return false;
        };

        path.is_file()
    }

    #[rhai_fn(global)]
    pub fn load(name: &str) -> String {
        let Some(path) = config_path(name) else {
            return String::new();
        };

        std::fs::read_to_string(path).unwrap_or_default()
    }

    #[rhai_fn(global)]
    pub fn save(name: &str, contents: &str) -> bool {
        let Some(path) = config_path(name) else {
            return false;
        };

        // Create ~/.ink/configs and any requested subdirectories.
        let Some(parent) = path.parent() else {
            return false;
        };

        if std::fs::create_dir_all(parent).is_err() {
            return false;
        }

        std::fs::write(path, contents).is_ok()
    }
}
