pub mod detection;
pub mod parser;
/// Route module for file-based routing
///
/// Contains pure functional components for route parsing and matching.
/// All modules follow functional programming principles:
/// - Pure functions (same input → same output)
/// - Immutable data structures
/// - Pattern matching for control flow
/// - Zero-copy optimizations where possible
pub mod pattern;

// Re-export commonly used types
pub use detection::{detect_intercepting_route, detect_parallel_route, extract_layout_name};
pub use parser::{calculate_priority, parse_pattern};
