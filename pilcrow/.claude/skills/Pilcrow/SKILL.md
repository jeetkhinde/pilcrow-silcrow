```markdown
# Pilcrow Development Patterns

> Auto-generated skill from repository analysis

## Overview
This skill teaches the core development patterns and conventions used in the Pilcrow Rust codebase. You'll learn how to structure files, write imports and exports, follow commit message conventions, and implement and run tests. These patterns ensure consistency and maintainability in Pilcrow's Rust projects.

## Coding Conventions

### File Naming
- Use **camelCase** for file names.
  - Example: `myModule.rs`, `dataParser.rs`

### Imports
- Use **relative imports** within the codebase.
  - Example:
    ```rust
    mod utils;
    use crate::utils::parseData;
    ```

### Exports
- Use **named exports** to expose specific items.
  - Example:
    ```rust
    pub fn processData() { /* ... */ }
    pub struct DataItem { /* ... */ }
    ```

### Commit Messages
- Follow **conventional commit** style.
- Use the `feat` prefix for new features.
- Keep commit messages concise (average ~56 characters).
  - Example:
    ```
    feat: add support for custom data parsers
    ```

## Workflows

### Feature Development
**Trigger:** When adding a new feature to the codebase  
**Command:** `/feature-development`

1. Create a new file using camelCase naming.
2. Implement the feature using relative imports for dependencies.
3. Export new functions or structs as named exports.
4. Write a test file matching `*.test.*` for the new feature.
5. Commit changes using the `feat` prefix and a concise message.

### Testing
**Trigger:** When verifying code changes or new features  
**Command:** `/run-tests`

1. Locate or create test files following the `*.test.*` pattern.
2. Implement tests for new or modified code.
3. Run tests using the appropriate Rust test command (e.g., `cargo test`).
4. Review test results and address any failures.

## Testing Patterns

- Test files follow the `*.test.*` naming pattern.
  - Example: `parser.test.rs`
- Testing framework is not explicitly specified; use Rust's built-in testing tools.
- Example test:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_parse_data() {
          let result = parseData("input");
          assert_eq!(result, expected_output);
      }
  }
  ```

## Commands
| Command              | Purpose                                  |
|----------------------|------------------------------------------|
| /feature-development | Start a new feature with proper patterns |
| /run-tests           | Run all tests in the codebase            |
```