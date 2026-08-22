use super::ast::{MotionDir, Node};
use super::commands::{CommandKind, ExecContext};

pub fn run(nodes: &[Node], ctx: &mut ExecContext) {
    for node in nodes {
        exec_node(node, ctx, false);
    }
}

fn exec_node(node: &Node, ctx: &mut ExecContext, forced: bool) {
    match node {
        Node::Command { letter, args } => match CommandKind::from_char(*letter) {
            Some(kind) => kind.run(args, forced, ctx),
            None => ctx.warn.show(format!("unknown command '{letter}'")),
        },
        Node::Motion { dir, n } => match dir {
            MotionDir::Up => ctx.doc.move_up(*n),
            MotionDir::Down => ctx.doc.move_down(*n),
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
        Node::Motion { dir, n } => match dir {
            MotionDir::Up => format!("move up {n} line(s)"),
            MotionDir::Down => format!("move down {n} line(s)"),
        },
        _ => "no help available".to_string(),
    };
    ctx.warn.show(text);
}
