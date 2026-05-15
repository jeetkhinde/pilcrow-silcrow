#[derive(Debug, Clone, PartialEq, Default)]
pub enum LayoutOption {
    #[default]
    Inherit,
    None,
    Root,
    Named(String),
    Pattern(String),
}
