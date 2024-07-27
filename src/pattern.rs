use anyhow::{anyhow, bail};
use std::cmp::PartialEq;
use std::ops::Index;
use std::str::{Bytes, FromStr};

#[derive(PartialEq, Clone, Debug)]
pub enum CharType {
    Digit,
    Alphanumeric,
}

impl CharType {
    pub fn match_char(&self, input_ch: &u8) -> bool {
        match self {
            CharType::Digit => input_ch.is_ascii_digit(),
            CharType::Alphanumeric => input_ch.is_ascii_alphanumeric(),
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub enum Token {
    CharExact(u8),
    Group(Vec<u8>),
    NegativeGroup(Vec<u8>),
    CharType(CharType),
    StartLine,
    EndLine,
}

impl Token {
    pub fn match_char(&self, input_ch: &u8) -> bool {
        match self {
            Token::CharExact(ch) => input_ch == ch,
            Token::CharType(char_type) => char_type.match_char(&input_ch),
            Token::Group(group) => group.contains(input_ch),
            Token::NegativeGroup(group) => !group.contains(input_ch),
            _ => false,
        }
    }
}

enum TokenModifier {
    Optional,
    OneOrMore,
}

#[derive(Clone, Debug)]
pub struct PatternItem {
    token: Token,
    optional: bool,
    more_than: Option<usize>,
    less_than: Option<usize>,
}

impl PatternItem {
    fn new(token: Token) -> Self {
        Self {
            token,
            optional: false,
            more_than: None,
            less_than: None,
        }
    }

    fn apply_modifier(&mut self, modifier: TokenModifier) {
        match modifier {
            TokenModifier::Optional => self.optional = true,
            TokenModifier::OneOrMore => self.more_than = Some(1),
        }
    }

    pub fn match_input(&self, input: &mut Bytes) -> Option<usize> {
        let Some(input_ch) = input.next() else {
            return None;
        };
        if !self.token.match_char(&input_ch) {
            return if self.optional { Some(0) } else { None };
        }

        if self.more_than.is_none() && self.less_than.is_none() {
            return Some(1);
        }

        let skip_chars = self.match_more(input) + 1;

        if !self.check_more_than(skip_chars) || !self.check_less_than(skip_chars) {
            return None;
        }

        Some(skip_chars)
    }

    fn match_more(&self, input: &mut Bytes) -> usize {
        input
            .take_while(|&input_ch| self.token.match_char(&input_ch))
            .count()
    }

    fn check_more_than(&self, skip_chars: usize) -> bool {
        let more_than = self.more_than.unwrap_or(1);
        skip_chars >= more_than
    }

    fn check_less_than(&self, skip_chars: usize) -> bool {
        if let Some(less_than) = self.less_than {
            skip_chars <= less_than
        } else {
            true
        }
    }
}

#[derive(Clone, Debug)]
pub struct Pattern {
    inner: Vec<PatternItem>,
    cursor: usize,
}

impl Pattern {
    pub fn is_next_token(&self, token_to_check: Token) -> bool {
        let t = self.inner.get(self.cursor);
        match t {
            Some(PatternItem { token, .. }) => *token == token_to_check,
            _ => false,
        }
    }

    pub fn is_next_optional(&self) -> bool {
        let Some(pattern_item) = self.inner.get(self.cursor) else {
            return false;
        };
        pattern_item.optional
    }

    pub fn next(&mut self) -> Option<&PatternItem> {
        let val = self.inner.get(self.cursor);
        self.cursor += 1;
        val
    }
}

impl FromStr for Pattern {
    type Err = anyhow::Error;

    fn from_str(pattern: &str) -> Result<Self, Self::Err> {
        let mut inner: Vec<PatternItem> = Vec::new();

        let length = pattern.len();
        let mut pattern = pattern.bytes().into_iter().enumerate();

        while let Some((i, char)) = pattern.next() {
            if char == b'?' {
                let Some(item) = inner.last_mut() else {
                    bail!("incorrect modifier usage: '?' used without token")
                };
                item.apply_modifier(TokenModifier::Optional);
            } else if char == b'+' {
                let Some(item) = inner.last_mut() else {
                    bail!("incorrect modifier usage: '+' used without token")
                };
                item.apply_modifier(TokenModifier::OneOrMore);
            } else if char == b'^' && i == 0 {
                inner.push(PatternItem::new(Token::StartLine))
            } else if char == b'$' && i == length - 1 {
                inner.push(PatternItem::new(Token::EndLine))
            } else if char == b'\\' {
                let (_, next_char) = pattern.next().ok_or(anyhow!(
                    "incorrect pattern: \\ symbol without value after it"
                ))?;
                match next_char {
                    b'd' => inner.push(PatternItem::new(Token::CharType(CharType::Digit))),
                    b'w' => inner.push(PatternItem::new(Token::CharType(CharType::Alphanumeric))),
                    _ => inner.push(PatternItem::new(Token::CharExact(next_char))),
                }
            } else if char == b'[' {
                let mut group = Vec::new();
                let (_, mut char) = pattern
                    .next()
                    .ok_or(anyhow!("incorrect pattern group: group open without end"))?;
                while char != b']' {
                    group.push(char);
                    char = pattern.next().ok_or(anyhow!("incorrect group pattern"))?.1;
                }
                if group.is_empty() {
                    bail!("incorrect group pattern: empty group")
                }

                if group[0] == b'^' {
                    group.remove(0);
                    inner.push(PatternItem::new(Token::NegativeGroup(group)))
                } else {
                    inner.push(PatternItem::new(Token::Group(group)))
                }
            } else {
                inner.push(PatternItem::new(Token::CharExact(char)))
            }
        }

        Ok(Self { inner, cursor: 0 })
    }
}

impl Index<usize> for Pattern {
    type Output = PatternItem;

    fn index(&self, index: usize) -> &Self::Output {
        &self.inner[index]
    }
}
