pub const HELP: &str = r#"
INK - Help

Ink is a small, keyboard-driven terminal text editor.

Files are opened as buffers, and multiple buffers can be open at the same
time. Commands are entered through the command bar.

COMMANDS

Commands consist of a command letter followed by optional arguments.

Arguments are separated with ':' and terminated with ';'.

    e;file.txt;
    g;42;
    x;ls -la;

Commands can be modified using:

    !       force
    ?       show help
    *N      repeat N times

For example:

    q!          force quit
    g;42;?      show help for the goto command
    g;42;*5     repeat the goto command five times


MOTIONS

    >N      move up N lines
    <N      move down N lines

If N is omitted, the motion moves one line.


POSITIONING

The goto command supports both line numbers and named positions.

    g;42;     go to line 42
    g;sf;     start of file
    g;ef;     end of file
    g;sl;     start of line
    g;el;     end of line


BUFFERS

Multiple files can be open simultaneously.

The active buffer is displayed in the buffer bar at the top of the screen.

Buffer switching and management can be performed through commands.


GENERATED BUFFERS

Ink can open generated content as read-only buffers.

Command output and help are examples of generated buffers. They behave like
normal buffers for navigation and viewing, but are not associated with a file
on disk.


KEYBOARD INPUT

    Escape       Open command bar
    Enter        Insert newline
    Backspace    Delete backwards
    Delete       Delete forwards
    Ctrl+Left    Move word backwards
    Ctrl+Right   Move word forwards


HELP

Append '?' to a command to view information about that command.

    g?
    e?
    q?

Use '?' by itself to open this help.

SIMPLE COMMANDS

    e - edit (more info: :e?)
    s - save
    q - close file
    Q - quit ink
    u/r - undo/redo
    c - copy line
    p - paste
    A/D - Previous/Next file in editor
    f - find (more info: :f?)
    R - replace (more info: :R?)

"#;
