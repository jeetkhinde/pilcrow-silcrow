use axum::http::{HeaderMap, header};

pub(crate) fn content_length_exceeds(headers: &HeaderMap, limit: usize) -> bool {
    headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|len| len > limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_length_exceeds_limit_only_for_valid_oversized_values() {
        let mut headers = HeaderMap::new();
        assert!(!content_length_exceeds(&headers, 10));

        headers.insert(header::CONTENT_LENGTH, "10".parse().unwrap());
        assert!(!content_length_exceeds(&headers, 10));

        headers.insert(header::CONTENT_LENGTH, "11".parse().unwrap());
        assert!(content_length_exceeds(&headers, 10));

        headers.insert(header::CONTENT_LENGTH, "not-a-number".parse().unwrap());
        assert!(!content_length_exceeds(&headers, 10));
    }
}
