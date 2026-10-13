//! Case conversions and checks, for the naming conventions.
//!
//! Leading acronyms fold as one block: `HPMax` gives `hpMax`, as in Go.
//! `_` separates words, which get a capital letter back.

/// Splits on `_`, ignoring empty segments.
fn words(name: &str) -> impl Iterator<Item = &str> {
    name.split('_').filter(|w| !w.is_empty())
}

/// `COUNT` → `Count`, `count` → `Count`, `hitCount` → `HitCount`.
fn capitalize(word: &str) -> String {
    let all_caps = word.len() > 1 && !word.chars().any(|c| c.is_ascii_lowercase());
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let rest: String = chars.collect();
    let rest = if all_caps {
        rest.to_ascii_lowercase()
    } else {
        rest
    };
    format!("{}{rest}", first.to_ascii_uppercase())
}

/// Lowercases the leading capitals: `HitCount` → `hitCount`,
/// `HPMax` → `hpMax`, `HP` → `hp`.
fn lower_head(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let run = chars.iter().take_while(|c| c.is_ascii_uppercase()).count();
    // In `HPMax`, the `M` starts the next word: it stays uppercase.
    let keep_last = run > 1 && run < chars.len() && chars[run].is_ascii_lowercase();
    let lowered = if keep_last { run - 1 } else { run };
    chars
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if i < lowered {
                c.to_ascii_lowercase()
            } else {
                *c
            }
        })
        .collect()
}

/// The name in lowerCamelCase.
pub fn to_lower_camel(name: &str) -> String {
    let mut words = words(name);
    let Some(first) = words.next() else {
        return name.to_owned();
    };
    // An all-caps first word (`MAX_COUNT`) folds entirely.
    let first = if first.len() > 1 && !first.chars().any(|c| c.is_ascii_lowercase()) {
        first.to_ascii_lowercase()
    } else {
        lower_head(first)
    };
    words.fold(first, |acc, w| acc + &capitalize(w))
}

/// The name in PascalCase.
pub fn to_pascal(name: &str) -> String {
    let converted: String = words(name).map(capitalize).collect();
    if converted.is_empty() {
        name.to_owned()
    } else {
        converted
    }
}

/// True if `name` is in lowerCamelCase.
pub fn is_lower_camel(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_lowercase()) && !name.contains('_')
}

/// True if `name` is in PascalCase.
pub fn is_pascal(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase()) && !name.contains('_')
}

/// The style `name` is written in, for messages: `snake_case`,
/// `SCREAMING_SNAKE_CASE`, `PascalCase`, `lowerCamelCase`, or `mixed case`.
pub fn style_of(name: &str) -> &'static str {
    let letters = name.chars().filter(char::is_ascii_alphabetic);
    let upper = letters.clone().any(|c| c.is_ascii_uppercase());
    let lower = letters.clone().any(|c| c.is_ascii_lowercase());
    let underscored = name.trim_matches('_').contains('_');
    match (underscored, upper, lower) {
        (true, false, _) => "snake_case",
        (true, true, false) => "SCREAMING_SNAKE_CASE",
        (true, true, true) => "mixed case",
        (false, false, _) => "lowercase",
        (false, true, false) if name.len() > 1 => "uppercase",
        _ if is_pascal(name) => "PascalCase",
        _ if is_lower_camel(name) => "lowerCamelCase",
        _ => "mixed case",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_are_named() {
        for (name, style) in [
            ("a_reference", "snake_case"),
            ("MAX_COUNT", "SCREAMING_SNAKE_CASE"),
            ("Private_Shift", "mixed case"),
            ("initialize", "lowercase"),
            ("COUNT", "uppercase"),
            ("HitCount", "PascalCase"),
            ("hitCount", "lowerCamelCase"),
            ("_private", "lowercase"),
        ] {
            assert_eq!(style_of(name), style, "{name}");
        }
    }

    #[test]
    fn lower_camel() {
        for (name, expected) in [
            ("HitCount", "hitCount"),
            ("hitCount", "hitCount"),
            ("HP", "hp"),
            ("HPMax", "hpMax"),
            ("I", "i"),
            ("hit_count", "hitCount"),
            ("MAX_COUNT", "maxCount"),
            ("_private", "private"),
            ("akTarget", "akTarget"),
            ("Target2", "target2"),
        ] {
            assert_eq!(to_lower_camel(name), expected, "{name}");
            assert!(is_lower_camel(expected), "{expected}");
        }
    }

    #[test]
    fn pascal() {
        for (name, expected) in [
            ("onInit", "OnInit"),
            ("OnInit", "OnInit"),
            ("Private_Shift", "PrivateShift"),
            ("do_thing", "DoThing"),
            ("GET_VALUE", "GetValue"),
        ] {
            assert_eq!(to_pascal(name), expected, "{name}");
            assert!(is_pascal(expected), "{expected}");
        }
    }

    #[test]
    fn hungarian_prefixes_are_lower_camel() {
        for name in [
            "akTarget", "abFlag", "aiCount", "afValue", "asName", "auiIndex",
        ] {
            assert!(is_lower_camel(name), "{name}");
        }
    }
}
