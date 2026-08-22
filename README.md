# ink

Ink is a small, terminal-based text editor built in Rust.

It is designed to be lightweight and keyboard-driven, with a simple command language and a small codebase.

## Status

Ink is usable for basic editing, but it is not yet feature-complete nor a drop-in replacement for more mature editors.

The project is still actively evolving, so internal APIs and behaviour may change.

## Building

Ink requires a recent stable Rust toolchain.

Build it with:

```sh
cargo build
```

and optionally install it:

```sh
cargo install --path .
```

Or run it directly with a file:

```sh
cargo run -- path/to/file.txt
```

## Usage

Ink can be started with a file path:

```sh
ink file.txt
```

or without a file:

```sh
ink
```

The editor is primarily keyboard-driven. Commands are entered through the command bar.

Ink has built-in help for its command language. Use the bare `?` modifier to inspect the available commands and their usage rather than relying on a static command list.
(use `?` just by itself to get information in ink)

## Project structure

```
src /
- main.rs
- clipboard.rs
- document.rs
- editor.rs
- terminal.rs
- warn.rs
- command /
    - ast.rs
    - commands.rs
    - executor.rs
    - parser.rs
```

### ```main.rs```

Owns the main event loop and connects the editor, terminal, command system, clipboard, and warnings.

Keyboard input is handled here and dispatched either to the active document or to the command bar.

### ```document.rs```

Contains the text buffer and document-level editing behaviour.

A ```Document``` represents one open buffer and owns things such as:

- Text contents
- Cursor position
- File path
- Dirty/read-only state
- Editing operations
- Cursor movement
- Undo state

Document operations should generally live here rather than in ```main.rs```.

### ```editor.rs```

```Editor``` manages the collection of open documents and tracks the active document.

This is where buffer-level operations belong, such as:

- Opening documents
- Closing documents
- Switching between documents
- Accessing the active document

A ```Document``` represents a buffer; an ```Editor``` represents the collection of buffers being worked on.

### ```terminal.rs```

Contains the terminal UI and rendering logic.

```Screen``` is responsible for:

- Rendering documents
- Rendering the buffer bar
- Rendering the status bar
- Rendering the command bar
- Positioning the terminal cursor
- Managing the visible document viewport
- Terminal setup and teardown

Terminal-specific behaviour should remain here rather than leaking into the document model.

### ```command/```

Contains Ink's command language.

```
ast.rs       Command syntax and AST types
parser.rs    Converts command input into the AST
commands.rs  Defines command behaviour
executor.rs  Executes parsed commands
```

Commands operate through an execution context which provides access to the editor and other shared editor state.

The command language is deliberately separate from the terminal input handling so commands can be parsed and executed independently of how they were entered.

### ```clipboard.rs```

Provides clipboard integration for editor operations.

### ```warn.rs```

Handles temporary warnings and messages displayed by the terminal UI.

## Architecture

The main separation in Ink is between the document, editor, and screen.

A ```Document``` is concerned with the contents and state of a single buffer, while ```Editor``` manages the set of buffers.

The terminal layer renders that state but does not own the document contents.

## Generated buffers

Ink can represent generated content as normal documents.

For example, command output and help output can be opened as read-only buffers.

This keeps generated content within the same buffer model as regular files and means it can use the same navigation and viewport behaviour.

## Contributing

Ink is intentionally small. Prefer straightforward implementations over introducing abstractions that are not currently needed.