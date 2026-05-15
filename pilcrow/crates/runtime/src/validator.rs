use crate::response::response::{FormErrors, form_errors};

/// Fluent form validation builder.
///
/// Accumulates field errors and echoed values in a single pass. Call
/// [`into_result`](Validator::into_result) at the end to get `Ok(())` when
/// all rules pass or `Err(FormErrors)` to pass to [`Req::fail`].
///
/// ```rust,ignore
/// pub async fn create(req: Req) -> ActionResult {
///     let name  = req.form.get("name").unwrap_or("");
///     let email = req.form.get("email").unwrap_or("");
///
///     if let Err(errs) = Validator::new()
///         .required("name",  name)
///         .required("email", email)
///         .min_length("name", name, 2)
///         .into_result()
///     {
///         return req.fail(errs);
///     }
///     redirect("/items")
/// }
/// ```
pub struct Validator {
    inner: FormErrors,
}

impl Validator {
    pub fn new() -> Self {
        Self {
            inner: form_errors(),
        }
    }

    /// Echo a field value for form repopulation, whether or not the field has errors.
    pub fn value(mut self, key: &str, val: &str) -> Self {
        self.inner = self.inner.value(key, val);
        self
    }

    /// Fail if `value.trim()` is empty.
    pub fn required(mut self, key: &str, value: &str) -> Self {
        self.inner = self.inner.value(key, value);
        if value.trim().is_empty() {
            self.inner = self.inner.error(key, "required");
        }
        self
    }

    /// Fail if the byte length of `value` is less than `min`.
    pub fn min_length(mut self, key: &str, value: &str, min: usize) -> Self {
        self.inner = self.inner.value(key, value);
        if value.len() < min {
            self.inner = self
                .inner
                .error(key, format!("must be at least {min} characters"));
        }
        self
    }

    /// Fail if the byte length of `value` exceeds `max`.
    pub fn max_length(mut self, key: &str, value: &str, max: usize) -> Self {
        self.inner = self.inner.value(key, value);
        if value.len() > max {
            self.inner = self
                .inner
                .error(key, format!("must be at most {max} characters"));
        }
        self
    }

    /// Fail if `value` does not contain an `@` sign with content before and after it.
    pub fn email(mut self, key: &str, value: &str) -> Self {
        self.inner = self.inner.value(key, value);
        let valid = value.contains('@') && {
            let mut parts = value.splitn(2, '@');
            let local = parts.next().unwrap_or("");
            let domain = parts.next().unwrap_or("");
            !local.is_empty() && domain.contains('.')
        };
        if !valid {
            self.inner = self.inner.error(key, "must be a valid email address");
        }
        self
    }

    /// Fail with a custom `message` when `condition` is `true`.
    pub fn custom(mut self, key: &str, value: &str, condition: bool, message: &str) -> Self {
        self.inner = self.inner.value(key, value);
        if condition {
            self.inner = self.inner.error(key, message);
        }
        self
    }

    /// Consume the validator. Returns `Ok(())` when no rules failed, or
    /// `Err(FormErrors)` containing all accumulated errors.
    #[allow(clippy::result_large_err)]
    pub fn into_result(self) -> Result<(), FormErrors> {
        if self.inner.has_errors {
            Err(self.inner)
        } else {
            Ok(())
        }
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_passes_for_nonempty_value() {
        assert!(
            Validator::new()
                .required("name", "Alice")
                .into_result()
                .is_ok()
        );
    }

    #[test]
    fn required_fails_for_empty_string() {
        let err = Validator::new()
            .required("name", "")
            .into_result()
            .unwrap_err();
        assert!(err.has_errors);
        assert_eq!(err.errors.get("name").map(|s| s.as_str()), Some("required"));
    }

    #[test]
    fn required_fails_for_whitespace_only() {
        let err = Validator::new()
            .required("name", "   ")
            .into_result()
            .unwrap_err();
        assert!(err.has_errors);
    }

    #[test]
    fn min_length_passes_at_boundary() {
        assert!(
            Validator::new()
                .min_length("pass", "abc", 3)
                .into_result()
                .is_ok()
        );
    }

    #[test]
    fn min_length_fails_below_boundary() {
        let err = Validator::new()
            .min_length("pass", "ab", 3)
            .into_result()
            .unwrap_err();
        assert!(err.errors.contains_key("pass"));
    }

    #[test]
    fn max_length_passes_at_boundary() {
        assert!(
            Validator::new()
                .max_length("code", "abc", 3)
                .into_result()
                .is_ok()
        );
    }

    #[test]
    fn max_length_fails_above_boundary() {
        let err = Validator::new()
            .max_length("code", "abcd", 3)
            .into_result()
            .unwrap_err();
        assert!(err.errors.contains_key("code"));
    }

    #[test]
    fn email_passes_valid_address() {
        assert!(
            Validator::new()
                .email("email", "user@example.com")
                .into_result()
                .is_ok()
        );
    }

    #[test]
    fn email_fails_missing_at_sign() {
        let err = Validator::new()
            .email("email", "notanemail")
            .into_result()
            .unwrap_err();
        assert!(err.errors.contains_key("email"));
    }

    #[test]
    fn multiple_errors_accumulate() {
        let err = Validator::new()
            .required("name", "")
            .required("email", "")
            .into_result()
            .unwrap_err();
        assert_eq!(err.error_list.len(), 2);
    }

    #[test]
    fn values_echoed_for_repopulation() {
        let err = Validator::new()
            .required("name", "")
            .into_result()
            .unwrap_err();
        assert_eq!(err.values.get("name").map(|s| s.as_str()), Some(""));
    }

    #[test]
    fn custom_rule_fires_on_true_condition() {
        let err = Validator::new()
            .custom("age", "17", true, "must be 18 or older")
            .into_result()
            .unwrap_err();
        assert_eq!(
            err.errors.get("age").map(|s| s.as_str()),
            Some("must be 18 or older")
        );
    }
}
