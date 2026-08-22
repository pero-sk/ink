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
