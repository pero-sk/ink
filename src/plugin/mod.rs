use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent};
use rhai::{AST, Array, Dynamic, Engine, FnPtr, Scope};

mod api;
pub use api::Ink;

use crate::command::commands::CommandKind;
use crate::plugin::api::ChangeMap;

/// Maps a raw key event to the string name used by plugin keymaps.
///
/// Character keys map to themselves:
///
///     "j"
///     " "
///
/// Named keys map to lowercase names:
///
///     "up"
///     "down"
///     "enter"
///     "backspace"
///
/// This excludes escape as it would be really stupid to allow plugins to override the command bar
pub fn key_name(key: &KeyEvent) -> Option<String> {
    Some(match key.code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Delete => "delete".to_string(),
        KeyCode::Tab => "tab".to_string(),
        _ => return None,
    })
}

pub struct PluginRuntime {
    engine: Engine,
    plugins: Vec<LoadedPlugin>,

    /// Command-bar letter -> (plugin index, callback).
    commands: HashMap<char, (usize, FnPtr)>,

    /// Buffer ID -> (plugin index, callback)
    changes: HashMap<u64, (usize, FnPtr)>,

    ink: Ink,
}

struct LoadedPlugin {
    ast: AST,
}

impl PluginRuntime {
    pub fn load(
        editor: Rc<RefCell<crate::editor::Editor>>,
        warn: Rc<RefCell<crate::warn::WarnPopup>>,
    ) -> Self {
        let ink = Ink {
            editor,
            warn,
            active_keymap: Rc::new(RefCell::new(None)),
            current_plugin: Rc::new(RefCell::new(0)),
            timers: Rc::new(RefCell::new(HashMap::new())),
            next_timer_id: Rc::new(RefCell::new(1)),
            change_callbacks: Rc::new(RefCell::new(ChangeMap::new())),
        };

        let mut engine = Engine::new();

        engine.register_type_with_name::<Ink>("Ink");

        engine.register_global_module(rhai::exported_module!(api::ink_api).into());
        engine.register_static_module("pathutils", rhai::exported_module!(api::pathutils).into());
        engine.register_static_module(
            "processutils",
            rhai::exported_module!(api::processutils).into(),
        );
        engine.register_static_module("jsonutils", rhai::exported_module!(api::jsonutils).into());
        engine.register_static_module(
            "configutils",
            rhai::exported_module!(api::configutils).into(),
        );

        let ink_for_fn = ink.clone();

        engine.register_fn("ink", move || ink_for_fn.clone());

        let config_path = crate::config::config_path_string();

        engine.register_fn("config_path", move || config_path.clone());

        let mut runtime = Self {
            engine,
            plugins: Vec::new(),
            commands: HashMap::new(),
            changes: HashMap::new(),
            ink,
        };

        runtime.load_all();

        runtime
    }

    fn plugin_dir() -> Option<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()?;

        Some(PathBuf::from(home).join(".ink").join("plugins"))
    }

    fn load_all(&mut self) {
        let Some(dir) = Self::plugin_dir() else {
            return;
        };

        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) == Some("rhai") {
                self.load_one(&path);
            }
        }
    }

    fn load_one(&mut self, path: &Path) {
        let ast = match self.engine.compile_file(path.to_path_buf()) {
            Ok(ast) => ast,

            Err(e) => {
                self.ink
                    .warn
                    .borrow_mut()
                    .show(format!("plugin {}: {e}", path.display()));

                return;
            }
        };

        let plugin_index = self.plugins.len();

        *self.ink.current_plugin.borrow_mut() = plugin_index;

        let mut scope = Scope::new();

        let pending: Rc<RefCell<Vec<(char, FnPtr)>>> = Rc::new(RefCell::new(Vec::new()));

        {
            let pending = pending.clone();

            self.engine
                .register_fn("register", move |letter: &str, callback: FnPtr| {
                    if let Some(c) = letter.chars().next() {
                        pending.borrow_mut().push((c, callback));
                    }
                });
        }

        if let Err(e) = self.engine.run_ast_with_scope(&mut scope, &ast) {
            self.ink
                .warn
                .borrow_mut()
                .show(format!("plugin {}: {e}", path.display()));
        }

        for (letter, callback) in pending.borrow_mut().drain(..) {
            self.commands.insert(letter, (plugin_index, callback));
        }

        self.plugins.push(LoadedPlugin { ast /*scope*/ });
    }

    /// Dispatch a command registered by a plugin.
    pub fn dispatch_command(&mut self, letter: char, args: &[String]) -> bool {
        let Some((idx, callback)) = self.commands.get(&letter).cloned() else {
            return false;
        };

        *self.ink.current_plugin.borrow_mut() = idx;

        let plugin = &self.plugins[idx];

        let rhai_args: Array = args.iter().map(|a| Dynamic::from(a.clone())).collect();

        if let Err(e) = callback.call::<()>(&self.engine, &plugin.ast, (rhai_args,)) {
            self.ink
                .warn
                .borrow_mut()
                .show(format!("plugin error: {e}"));
        }

        true
    }

    /// Dispatch a buffer-local plugin key.
    ///
    /// A keymap entry is only valid when its buffer ID matches the
    /// currently active buffer.
    pub fn dispatch_key(&mut self, key: &str) -> bool {
        let active_buffer_id = {
            let editor = self.ink.editor.borrow();
            editor.doc().id
        };

        let entry = {
            let map = self.ink.active_keymap.borrow();

            map.as_ref().and_then(|m| m.get(key)).cloned()
        };

        let Some((buffer_id, plugin_index, callback)) = entry else {
            return false;
        };

        // This keymap belongs to a different buffer.
        if buffer_id != active_buffer_id {
            return false;
        }

        *self.ink.current_plugin.borrow_mut() = plugin_index;

        let plugin = &self.plugins[plugin_index];

        if let Err(e) = callback.call::<()>(&self.engine, &plugin.ast, ()) {
            self.ink
                .warn
                .borrow_mut()
                .show(format!("plugin error: {e}"));
        }

        true
    }

    /// Runs any timers that are due.
    ///
    /// This must be called from the editor's main thread/event loop.
    /// Timer callbacks are executed synchronously through Rhai.
    pub fn tick_timers(&mut self) {
        let now = Instant::now();

        let mut callbacks = Vec::new();

        {
            let mut timers = self.ink.timers.borrow_mut();

            timers.retain(|_, timer| {
                let document_exists = {
                    let editor = self.ink.editor.borrow();

                    editor
                        .documents
                        .iter()
                        .any(|doc| doc.id == timer.document_id)
                };

                if !document_exists {
                    return false;
                }

                if now >= timer.next_fire {
                    callbacks.push((timer.plugin_index, timer.callback.clone()));

                    timer.next_fire = now + timer.duration;
                }

                true
            });
        }

        for (plugin_index, callback) in callbacks {
            let Some(plugin) = self.plugins.get(plugin_index) else {
                continue;
            };

            if let Err(e) = callback.call::<()>(&self.engine, &plugin.ast, ()) {
                self.ink
                    .warn
                    .borrow_mut()
                    .show(format!("plugin error: {e}"));
            }
        }
    }

    pub fn notify_change(&mut self, buffer_id: u64) {
        let callbacks = {
            let changes = self.ink.change_callbacks.borrow();

            // change_callbacks stores a single (plugin_index, callback) per buffer;
            // convert it to a Vec for uniform iteration.
            changes
                .get(&buffer_id)
                .cloned()
                .map(|cb| vec![cb])
                .unwrap_or_default()
        };

        for (plugin_index, callback) in callbacks {
            let Some(plugin) = self.plugins.get(plugin_index) else {
                continue;
            };

            *self.ink.current_plugin.borrow_mut() = plugin_index;

            if let Err(e) = callback.call::<()>(&self.engine, &plugin.ast, ()) {
                self.ink
                    .warn
                    .borrow_mut()
                    .show(format!("plugin error: {e}"));
            }
        }
    }

    /// Returns true only when the active buffer actually has a
    /// plugin keymap.
    pub fn in_plugin_mode(&self) -> bool {
        let active_buffer_id = {
            let editor = self.ink.editor.borrow();
            editor.doc().id
        };

        let map = self.ink.active_keymap.borrow();

        let Some(map) = map.as_ref() else {
            return false;
        };

        map.values()
            .any(|(buffer_id, _, _)| *buffer_id == active_buffer_id)
    }

    /// Esc always exits plugin mode.
    pub fn exit_plugin_mode(&mut self) {
        *self.ink.active_keymap.borrow_mut() = None;
    }
}
