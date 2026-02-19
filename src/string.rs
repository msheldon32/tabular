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
    if s.chars().count() <= 1 {
        return 0;
    }
    let cur_char = s.chars().nth(idx.saturating_sub(1)).unwrap_or('0');
    let cur_type = CharType::from(cur_char);
    let mut pass = false;

    let chars_before: Vec<(usize, char)> = s.chars()
        .enumerate()
        .take(idx.saturating_sub(1))
        .collect();

    for (i, c) in chars_before.into_iter().rev() {
        if CharType::from(c) != cur_type {
            if cur_type == CharType::Whitespace {
                return i;
            }
            return i + 1;
        }
        if CharType::from(c) == CharType::Whitespace {
            pass = true;
            continue;
        }
        if pass {
            return i;
        }
    }

    0
}

pub fn get_word_end(s: &String, idx: usize) -> usize{
    let char_count = s.chars().count();
    if char_count <= 1 {
        return idx;
    }
    let cur_char = s.chars().nth(idx+1).unwrap_or('0');
    let cur_type = CharType::from(cur_char);
    let mut pass = false;

    for (i, c) in s.chars().enumerate().skip(idx+1) {
        if CharType::from(c) != cur_type {
            if cur_type == CharType::Whitespace {
                return i;
            }
            return i.saturating_sub(1);
        }
        if CharType::from(c) == CharType::Whitespace {
            pass = true;
            continue;
        }
        if pass {
            return i;
        }
    }

    char_count.saturating_sub(1)
}
