use std::str::FromStr;

/// Parameter constraint for validating dynamic route parameters
///
/// Uses functional pattern matching for validation logic.
/// Constraints ensure type safety and input validation at routing level.
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterConstraint {
    /// No constraint - accepts any value (default)
    Any,
    /// Integer numbers only: 123, -456
    Int,
    /// Unsigned integer: 123, 456 (no negatives)
    UInt,
    /// Alphabetic characters only: abc, XYZ
    Alpha,
    /// Alphanumeric: abc123, Test99
    AlphaNum,
    /// Slug format: hello-world, my_post
    Slug,
    /// UUID format: 550e8400-e29b-41d4-a716-446655440000
    Uuid,
    /// User-defined external matcher: calls `src/params/<name>::match_param(value)` at runtime.
    /// Validated in the generated handler, not during route matching.
    External(String),
}

impl ParameterConstraint {
    /// Validates a value against this constraint (functional predicate)
    ///
    /// Pure function that maps (constraint, value) → bool
    ///
    /// # Examples
    ///
    /// ```
    /// use pilcrow_routekit::ParameterConstraint;
    ///
    /// assert!(ParameterConstraint::Int.validate("123"));
    /// assert!(!ParameterConstraint::Int.validate("abc"));
    ///
    /// assert!(ParameterConstraint::Alpha.validate("hello"));
    /// assert!(!ParameterConstraint::Alpha.validate("hello123"));
    /// ```
    pub fn validate(&self, value: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Int => value.parse::<i64>().is_ok(),
            Self::UInt => value.parse::<u64>().is_ok(),
            Self::Alpha => value.chars().all(|c| c.is_alphabetic()),
            Self::AlphaNum => value.chars().all(|c| c.is_alphanumeric()),
            Self::Slug => value
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_'),
            Self::Uuid => {
                // Simple UUID validation: 8-4-4-4-12 hex digits
                // Pure functional approach: split → map → fold
                let parts: Vec<&str> = value.split('-').collect();
                parts.len() == 5
                    && parts[0].len() == 8
                    && parts[1].len() == 4
                    && parts[2].len() == 4
                    && parts[3].len() == 4
                    && parts[4].len() == 12
                    && parts
                        .iter()
                        .all(|p| p.chars().all(|c| c.is_ascii_hexdigit()))
            }

            // External matchers are validated in the generated handler, not here.
            Self::External(_) => true,
        }
    }
}

impl FromStr for ParameterConstraint {
    type Err = ();

    /// Parses constraint from string (functional parser)
    ///
    /// Maps string → ParameterConstraint using pattern matching
    ///
    /// Unknown values default to `Self::Any`, so parsing always succeeds.
    ///
    /// # Examples
    ///
    /// ```
    /// use pilcrow_routekit::ParameterConstraint;
    /// use std::str::FromStr;
    ///
    /// assert_eq!(ParameterConstraint::from_str("int").unwrap(), ParameterConstraint::Int);
    /// assert_eq!(ParameterConstraint::from_str("alpha").unwrap(), ParameterConstraint::Alpha);
    /// assert_eq!(ParameterConstraint::from_str("uuid").unwrap(), ParameterConstraint::Uuid);
    /// ```
    ///
    /// Supported values: "int", "uint", "alpha", "alphanum", "slug", "uuid"
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "int" | "integer" => Self::Int,
            "uint" | "unsigned" => Self::UInt,
            "alpha" => Self::Alpha,
            "alphanum" | "alphanumeric" => Self::AlphaNum,
            "slug" => Self::Slug,
            "uuid" => Self::Uuid,

            _ => Self::Any,
        })
    }
}
