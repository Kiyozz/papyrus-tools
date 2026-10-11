//! Conversions de casse, pour les suggestions des lints de nommage.
//!
//! Les acronymes de tête se replient d'un bloc : `HPMax` donne `hpMax`, comme
//! en Go. Les `_` séparent des mots, qui reprennent une majuscule.

/// Découpe sur `_`, en ignorant les segments vides.
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

/// Baisse la casse des majuscules de tête : `HitCount` → `hitCount`,
/// `HPMax` → `hpMax`, `HP` → `hp`.
fn lower_head(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let run = chars.iter().take_while(|c| c.is_ascii_uppercase()).count();
    // Dans `HPMax`, le `M` ouvre le mot suivant : il reste en majuscule.
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

/// Le nom en lowerCamelCase.
pub fn to_lower_camel(name: &str) -> String {
    let mut words = words(name);
    let Some(first) = words.next() else {
        return name.to_owned();
    };
    // Un premier mot tout en majuscules (`MAX_COUNT`) se replie entièrement.
    let first = if first.len() > 1 && !first.chars().any(|c| c.is_ascii_lowercase()) {
        first.to_ascii_lowercase()
    } else {
        lower_head(first)
    };
    words.fold(first, |acc, w| acc + &capitalize(w))
}

/// Le nom en PascalCase.
pub fn to_pascal(name: &str) -> String {
    let converted: String = words(name).map(capitalize).collect();
    if converted.is_empty() {
        name.to_owned()
    } else {
        converted
    }
}

/// Vrai si `name` est en lowerCamelCase.
pub(crate) fn is_lower_camel(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_lowercase()) && !name.contains('_')
}

/// Vrai si `name` est en PascalCase.
pub(crate) fn is_pascal(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase()) && !name.contains('_')
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn les_prefixes_hongrois_sont_du_lower_camel() {
        for name in [
            "akTarget", "abFlag", "aiCount", "afValue", "asName", "auiIndex",
        ] {
            assert!(is_lower_camel(name), "{name}");
        }
    }
}
