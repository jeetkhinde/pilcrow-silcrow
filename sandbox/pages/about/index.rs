pub struct Props {
    pub title: &'static str,
    pub description: &'static str,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        title: "About",
        description: "Pilcrow compiles .html templates with Rust frontmatter into \
                      type-safe Askama render functions at build time.",
    })
}
