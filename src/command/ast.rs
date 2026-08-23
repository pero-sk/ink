use crate::command::commands::CommandKind;

#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Plain(String),
    Literal(String),
}

impl Arg {
    pub fn as_str(&self) -> &str {
        match self {
            Arg::Plain(s) => s,
            Arg::Literal(s) => s,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MotionDir {
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Command { letter: char, args: Vec<Arg> },
    Motion { dir: MotionDir, n: usize },
    Block { items: Vec<Node> },
    Repeat { target: Box<Node>, n: usize },
    Force { target: Box<Node> },
    Help { target: Box<Node> },
    HelpRoot,
}
impl Node {
    pub fn trigger_change(&self) -> bool {
        match self {
            Node::Command { letter, .. } => CommandKind::from_char(*letter)
                .map(|kind| kind.trigger_change())
                .unwrap_or(false),

            Node::Block { items } => items.iter().any(|item| item.trigger_change()),

            Node::Repeat { target, .. } | Node::Force { target } | Node::Help { target } => {
                target.trigger_change()
            }

            Node::Motion { .. } | Node::HelpRoot => false,
        }
    }
}
