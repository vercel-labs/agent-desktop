/// Text as searches compare it: lowercase, single spaces, and without accents. Accents
/// are folded because accessibility text mixes both Unicode forms (a title taken from a
/// file name arrives decomposed, "i" plus a combining accent, while most apps send "í"),
/// and because an agent often types a name without them ("Configuracion").
pub(crate) fn normalize(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut pending_space = false;
    for character in value.chars() {
        if is_bidirectional_control(character) || is_combining_mark(character) {
            continue;
        }
        let character = fold_latin_accent(character).unwrap_or(character);
        if character.is_whitespace() {
            pending_space = !normalized.is_empty();
            continue;
        }
        if pending_space {
            normalized.push(' ');
            pending_space = false;
        }
        if character.is_ascii() {
            normalized.push(character.to_ascii_lowercase());
        } else {
            normalized.extend(character.to_lowercase());
        }
    }
    normalized
}

fn is_combining_mark(character: char) -> bool {
    matches!(
        character,
        '\u{0300}'..='\u{036f}'
            | '\u{1ab0}'..='\u{1aff}'
            | '\u{1dc0}'..='\u{1dff}'
            | '\u{20d0}'..='\u{20ff}'
            | '\u{fe20}'..='\u{fe2f}'
    )
}

/// The base letter of an accented Latin letter (Latin-1 Supplement and Latin Extended-A).
fn fold_latin_accent(character: char) -> Option<char> {
    Some(match character {
        'À'..='Å' | 'à'..='å' | 'Ā'..='ą' => 'a',
        'Ç' | 'ç' | 'Ć'..='č' => 'c',
        'Ď'..='đ' => 'd',
        'È'..='Ë' | 'è'..='ë' | 'Ē'..='ě' => 'e',
        'Ĝ'..='ģ' => 'g',
        'Ĥ'..='ħ' => 'h',
        'Ì'..='Ï' | 'ì'..='ï' | 'Ĩ'..='ı' => 'i',
        'Ĵ' | 'ĵ' => 'j',
        'Ķ' | 'ķ' => 'k',
        'Ĺ'..='ł' => 'l',
        'Ñ' | 'ñ' | 'Ń'..='ň' => 'n',
        'Ò'..='Ö' | 'Ø' | 'ò'..='ö' | 'ø' | 'Ō'..='ő' => 'o',
        'Ŕ'..='ř' => 'r',
        'Ś'..='š' => 's',
        'Ţ'..='ŧ' => 't',
        'Ù'..='Ü' | 'ù'..='ü' | 'Ũ'..='ų' => 'u',
        'Ŵ' | 'ŵ' => 'w',
        'Ý' | 'ý' | 'ÿ' | 'Ŷ'..='Ÿ' => 'y',
        'Ź'..='ž' => 'z',
        _ => return None,
    })
}

pub(crate) fn is_bidirectional_control(character: char) -> bool {
    matches!(
        character,
        '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
    )
}

pub(crate) fn contains(haystack: &str, normalized_needle: &str) -> bool {
    if normalized_needle.is_empty() {
        return true;
    }
    normalize(haystack).contains(normalized_needle)
}

pub(crate) fn node_contains(node: &crate::AccessibilityNode, normalized_needle: &str) -> bool {
    [
        node.identity.name.as_deref(),
        node.identity.value.as_deref(),
        node.identity.description.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|text| contains(text, normalized_needle))
}

#[cfg(test)]
#[path = "search_text_tests.rs"]
mod tests;
