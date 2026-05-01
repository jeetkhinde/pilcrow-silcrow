pub struct Props {
    pub name: String,
    pub bio: String,
}

pub async fn load(req: Req) -> AppResult<Props> {
    let name = req.query.get("name").unwrap_or("Anonymous").to_string();
    Ok(Props {
        name,
        bio: "A Pilcrow user.".to_string(),
    })
}
