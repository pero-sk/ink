use crate::command::help::HELP;
use crate::document::Document;

use super::ast::{MotionDir, Node};
use super::commands::{CommandKind, ExecContext};

pub fn run(nodes: &[Node], ctx: &mut ExecContext) -> bool {
    let changed = nodes.iter().any(Node::trigger_change);
    for node in nodes {
        exec_node(node, ctx, false);
    }

    changed
}

fn exec_node(node: &Node, ctx: &mut ExecContext, forced: bool) {
    match node {
        Node::Command { letter, args } => {
            let string_args: Vec<String> = args.iter().map(|a| a.as_str().to_string()).collect();
            if !ctx.plugins.dispatch_command(*letter, &string_args) {
                match CommandKind::from_char(*letter) {
                    Some(kind) => kind.run(args, forced, ctx),
                    None => ctx
                        .warn
                        .borrow_mut()
                        .show(format!("unknown command '{letter}'")),
                }
            }
        }
        Node::Motion { dir, n } => match dir {
            MotionDir::Up => ctx.editor.borrow_mut().doc_mut().move_up(*n),
            MotionDir::Down => ctx.editor.borrow_mut().doc_mut().move_down(*n),
        },
        Node::Block { items } => {
            for item in items {
                exec_node(item, ctx, forced);
            }
        }
        Node::Repeat { target, n } => {
            for _ in 0..*n {
                exec_node(target, ctx, forced);
            }
        }
        Node::Force { target } => exec_node(target, ctx, true),
        Node::Help { target } => show_help(target, ctx),
        Node::HelpRoot => return,
    }
}

fn show_help(node: &Node, ctx: &mut ExecContext) {
    let text = match node {
        Node::Command { letter, .. } => match CommandKind::from_char(*letter) {
            Some(kind) => kind.help().to_string(),
            None => format!("no help available for '{letter}'"),
        },

        Node::Block { items } => {
            let letters: Vec<String> = items
                .iter()
                .filter_map(|n| match n {
                    Node::Command { letter, .. } => Some(letter.to_string()),
                    _ => None,
                })
                .collect();

            format!("block: {}", letters.join(", "))
        }

        Node::Motion { dir, .. } => match dir {
            MotionDir::Up => format!("move up N line(s)"),
            MotionDir::Down => format!("move down N line(s)"),
        },

        Node::HelpRoot => HELP.to_string(),

        _ => "no help available".to_string(),
    };

    ctx.editor
        .borrow_mut()
        .open(Document::from_text("help", text));
}
