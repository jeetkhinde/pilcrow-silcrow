/// Matches only non-empty strings that parse as valid integers.
pub fn match_param(value: &str) -> bool {
    !value.is_empty() && value.parse::<i64>().is_ok()
}
