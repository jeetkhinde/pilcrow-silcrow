pub struct Props {
    pub title: &'static str,
    pub message: &'static str,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        title: "Pilcrow",
        message: "Welcome to Pilcrow — an Astro-like web framework for Rust.",
    })
}
