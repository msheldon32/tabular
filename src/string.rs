#[derive(PartialEq)]
enum CharType {
    Whitespace,
    Numeric,
    Punctuation,
    Alphabetic
}

impl CharType {
    fn from(c: char) -> CharType {
        if c.is_whitespace() {
            CharType::Whitespace
        } else if c.is_numeric() {
            CharType::Numeric
        } else if c.is_alphabetic() {
            CharType::Alphabetic
        } else {
            CharType::Punctuation
        }
    }
}

pub fn get_word_start(s: &String, idx: usize) -> usize{
    if s.len() <= 1 {
        return 0usize;
    }
    let cur_char = s.chars().nth(idx.saturating_sub(1)).unwrap_or('0');
    let cur_type = CharType::from(cur_char);
    let mut pass = false;

    for (i, c) in s[..idx.saturating_sub(1)].char_indices().rev() {
        if CharType::from(c) != cur_type {
            if cur_type == CharType::Whitespace {
                return i;
            }
            return i+1;
        }
        if CharType::from(c) == CharType::Whitespace {
            pass = true;
            continue;
        }
        if pass {
            return i;
        }
    }

    0usize
}

pub fn get_word_end(s: &String, idx: usize) -> usize{
    if s.len() <= 1 {
        return idx;
    }
    let cur_char = s.chars().nth(idx+1).unwrap_or('0');
    let cur_type = CharType::from(cur_char);
    let mut pass = false;

    for (i, c) in s[idx+1..].char_indices() {
        if CharType::from(c) != cur_type {
            if cur_type == CharType::Whitespace {
                return i+idx+1;
            }
            return i+idx;
        }
        if CharType::from(c) == CharType::Whitespace {
            pass = true;
            continue;
        }
        if pass {
            return i+idx;
        }
    }

    s.len().saturating_sub(1)
}
