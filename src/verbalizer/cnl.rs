//! Controlled Natural Language helpers.

pub fn indefinite_article(display: &str) -> &'static str {
    let Some(first) = display.chars().next() else {
        return "";
    };
    if matches!(
        first,
        'a' | 'e' | 'i' | 'o' | 'u' | 'A' | 'E' | 'I' | 'O' | 'U'
    ) {
        "an "
    } else {
        "a "
    }
}

/// Prefix an indefinite article unless `display` is empty or already has one.
pub fn with_article(display: &str) -> String {
    let d = display.trim();
    if d.is_empty() {
        return String::new();
    }
    let lower = d.to_ascii_lowercase();
    if lower.starts_with("a ") || lower.starts_with("an ") {
        return d.to_string();
    }
    format!("{}{d}", indefinite_article(d))
}

/// Join child clause strings: one child is prefixed with a space; several become `(a, and b)`.
pub fn join_clauses(parts: &[String]) -> String {
    match parts.len() {
        0 => String::new(),
        1 => format!(" {}", parts[0]),
        _ => format!(" ({})", parts.join(", and ")),
    }
}

pub fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn articles() {
        assert_eq!(with_article(""), "");
        assert_eq!(with_article("pizza"), "a pizza");
        assert_eq!(with_article("olive"), "an olive");
        assert_eq!(with_article("a meat pizza"), "a meat pizza");
        assert_eq!(with_article("an olive topping"), "an olive topping");
    }

    #[test]
    fn clauses() {
        assert_eq!(join_clauses(&[]), "");
        assert_eq!(join_clauses(&["a pizza".into()]), " a pizza");
        assert_eq!(
            join_clauses(&["a mozzarella topping".into(), "a tomato topping".into()]),
            " (a mozzarella topping, and a tomato topping)"
        );
    }
}
