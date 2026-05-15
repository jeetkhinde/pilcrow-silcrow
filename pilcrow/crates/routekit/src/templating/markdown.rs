use markdown::{Options, to_html_with_options};

use crate::templating::compiler::{HtmlModuleParseError, HtmlModuleParts};

/// Transpiles a Markdown or MDX file into Pilcrow HTML module parts.
///
/// This converts the Markdown content to HTML, preserving any JSX/HTML tags
/// (like `<Component />`) so that the Pilcrow template compiler can later
/// transpile them into Askama component calls.
///
/// Any YAML frontmatter is consumed by `markdown-rs` and currently discarded
/// from the returned `HtmlModuleParts`, keeping the `rust` section empty.
/// To add Rust logic to a `.md` or `.mdx` route, use a code-behind `.rs` file.
pub fn transpile_markdown(input: &str) -> Result<HtmlModuleParts, HtmlModuleParseError> {
    let mut opts = Options::gfm();
    opts.parse.constructs.frontmatter = true;
    opts.compile.allow_dangerous_html = true;

    let template =
        to_html_with_options(input, &opts).map_err(|_| HtmlModuleParseError::EmptyTemplate)?;

    if template.trim().is_empty() {
        return Err(HtmlModuleParseError::EmptyTemplate);
    }

    Ok(HtmlModuleParts {
        rust: String::new(),
        template,
    })
}
