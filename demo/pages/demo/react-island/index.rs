pub struct Props {
    pub username: String,
    pub member_since: String,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        username: "pilcrow-user".to_string(),
        member_since: "January 2026".to_string(),
    })
}
