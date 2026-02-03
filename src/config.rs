use std::collections::HashMap;
use crate::input::SequenceAction;

type ActionBuilder = fn(char) -> SequenceAction;

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub enum KeySequence {
    Zero,
    One(char),
    Two(char, char)
}

impl KeySequence {
    pub fn from_str(s: Vec<char>) -> Self {
        let mut it = s.iter();

        let first = it.next();

        if !first.is_some() {
            return KeySequence::Zero;
        }

        let second = it.next();

        if second.is_some() {
            KeySequence::Two(*first.unwrap(), *second.unwrap())
        } else {
            KeySequence::One(*first.unwrap())
        }
    }

    pub fn split(&self) -> Option<(KeySequence, char)> {
        match self {
            KeySequence::Two(a,b) => Some((KeySequence::One(a.clone()), b.clone())),
            _default => None
        }
    }
}

pub struct CommandTable {
    basic_map: HashMap<KeySequence, SequenceAction>, // basic mappings
    wildcard_map: HashMap<KeySequence, ActionBuilder> // wildcard mapping
}

impl CommandTable {
    pub fn new(basic_map: HashMap<KeySequence, SequenceAction>, 
               wildcard_map: HashMap<KeySequence, ActionBuilder>) -> Self {
        Self { basic_map, wildcard_map }
    }

    pub fn get(&self, seq: KeySequence) -> Option<SequenceAction> {
        self.basic_map.get(&seq).copied()
    }

    pub fn match_sequence(&self, s: Vec<char>) -> Option<SequenceAction> {
        let keyseq = KeySequence::from_str(s.clone());

        let tried_action = self.basic_map.get(&keyseq);

        if tried_action.is_none() {
            // fallthrough to wildcard
            let mut it = s.iter();
            match keyseq.split() {
                Some((a, b)) => {
                    return self.wildcard_map.get(&a).map(|f| f(b));
                },
                _default => {}
            }
        }

        tried_action.cloned()
    }
}


impl Default for CommandTable {
    fn default() -> Self {
        Self {
            basic_map: HashMap::from([
                (KeySequence::Two('g', 'g'), SequenceAction::MoveToTop),
                (KeySequence::Two('d', 'r'), SequenceAction::DeleteRow),
                (KeySequence::Two('d', 'c'), SequenceAction::DeleteCol),
                (KeySequence::Two('d', 'd'), SequenceAction::Delete),
                (KeySequence::Two('y', 'r'), SequenceAction::YankRow),
                (KeySequence::Two('y', 'c'), SequenceAction::YankCol),
                (KeySequence::Two('y', 'y'), SequenceAction::Yank),

                (KeySequence::One('j'), SequenceAction::MoveDown),
                (KeySequence::One('k'), SequenceAction::MoveUp),
                (KeySequence::One('h'), SequenceAction::MoveLeft),
                (KeySequence::One('l'), SequenceAction::MoveRight),

                (KeySequence::Two('f', 'f'), SequenceAction::FormatDefault),
                (KeySequence::Two('f', ','), SequenceAction::FormatCommas),
                (KeySequence::Two('f', '$'), SequenceAction::FormatCurrency),
                (KeySequence::Two('f', 'e'), SequenceAction::FormatScientific),
                (KeySequence::Two('f', '%'), SequenceAction::FormatPercentage),
            ]),
            wildcard_map: HashMap::from([
                (KeySequence::One('^'), SequenceAction::SelectRegister as ActionBuilder)
            ])
        }
    }
}

pub struct AppConfig {
    pub commands: CommandTable
}

impl AppConfig {
    pub fn new() -> Self {
        Self {
            commands: CommandTable::default()
        }
    }
}
