use super::ast::{Arg, MotionDir, Node};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error at {}: {}", self.position, self.message)
    }
}

pub fn parse(input: &str) -> Result<Vec<Node>, ParseError> {
    let chars: Vec<char> = input.chars().collect();
    let mut p = Parser { chars, pos: 0 };
    p.parse_sequence()
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn err(&self, message: impl Into<String>) -> ParseError {
        ParseError {
            message: message.into(),
            position: self.pos,
        }
    }

    fn parse_sequence(&mut self) -> Result<Vec<Node>, ParseError> {
        let mut nodes = Vec::new();
        while self.peek().is_some() {
            let node = if self.peek() == Some(',') {
                self.parse_block()?
            } else {
                let item = self.parse_item()?;
                self.parse_modifier(item)?
            };
            nodes.push(node);
        }
        Ok(nodes)
    }

    fn parse_block(&mut self) -> Result<Node, ParseError> {
        self.advance();
        let mut items = Vec::new();
        loop {
            match self.peek() {
                None => break,
                Some(',') => {
                    self.advance();
                    break;
                }
                _ => {
                    let item = self.parse_item()?;
                    let item = self.parse_modifier(item)?;
                    items.push(item);
                }
            }
        }
        if items.is_empty() {
            return Err(self.err("empty block"));
        }
        let block = Node::Block { items };

        self.parse_modifier(block)
    }

    fn parse_item(&mut self) -> Result<Node, ParseError> {
        match self.peek() {
            Some(c) if c.is_ascii_alphabetic() => self.parse_command(),
            Some('>') | Some('<') => self.parse_motion(),
            Some(c) => Err(self.err(format!("unexpected character '{c}'"))),
            None => Err(self.err("unexpected end of input")),
        }
    }

    fn parse_command(&mut self) -> Result<Node, ParseError> {
        let letter = self.advance().expect("checked by caller");
        let args = if self.peek() == Some(';') {
            self.parse_args()?
        } else {
            Vec::new()
        };
        Ok(Node::Command { letter, args })
    }

    fn parse_args(&mut self) -> Result<Vec<Arg>, ParseError> {
        self.advance();
        let mut args = Vec::new();

        if self.peek() == Some(';') {
            self.advance();
            return Ok(args);
        }

        loop {
            let arg = if self.peek() == Some('"') {
                self.parse_literal_arg()?
            } else {
                self.parse_plain_arg()?
            };
            args.push(arg);

            match self.peek() {
                Some(':') => {
                    self.advance();
                    continue;
                }
                Some(';') => {
                    self.advance();
                    break;
                }
                None => return Err(self.err("unterminated argument list, expected ';'")),
                Some(c) => return Err(self.err(format!("unexpected '{c}' in argument list"))),
            }
        }
        Ok(args)
    }

    fn parse_literal_arg(&mut self) -> Result<Arg, ParseError> {
        self.advance();
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('"') => break,
                Some(c) => s.push(c),
                None => return Err(self.err("unterminated literal argument, expected '\"'")),
            }
        }
        Ok(Arg::Literal(s))
    }

    fn parse_plain_arg(&mut self) -> Result<Arg, ParseError> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == ':' || c == ';' {
                break;
            }
            s.push(c);
            self.advance();
        }
        Ok(Arg::Plain(s))
    }

    fn parse_motion(&mut self) -> Result<Node, ParseError> {
        let dir = match self.advance().unwrap() {
            '>' => MotionDir::Up,
            '<' => MotionDir::Down,
            _ => unreachable!(),
        };
        let digits = self.consume_digits();
        let n = if digits.is_empty() {
            1
        } else {
            digits
                .parse()
                .map_err(|_| self.err("invalid motion count"))?
        };
        Ok(Node::Motion { dir, n })
    }

    fn parse_modifier(&mut self, node: Node) -> Result<Node, ParseError> {
        match self.peek() {
            Some('*') => {
                self.advance();
                let digits = self.consume_digits();
                if digits.is_empty() {
                    return Err(self.err("expected a number after '*'"));
                }
                let n: usize = digits
                    .parse()
                    .map_err(|_| self.err("invalid repeat count"))?;
                Ok(Node::Repeat {
                    target: Box::new(node),
                    n,
                })
            }
            Some('!') => {
                self.advance();
                Ok(Node::Force {
                    target: Box::new(node),
                })
            }
            Some('?') => {
                self.advance();
                Ok(Node::Help {
                    target: Box::new(node),
                })
            }
            _ => Ok(node),
        }
    }

    fn consume_digits(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        s
    }
}
