pub struct Props {
    pub contacts: Vec<crate::data::ContactSummary>,
    pub q: String,
    pub searching: bool,
    pub sidebar_contact_count: i64,
}

pub async fn load(req: Req) -> AppResult<Props> {
    let q = req.query.get("q").unwrap_or("").to_owned();
    let active_id = req
        .params
        .get("contact_id")
        .map(String::as_str);
    let contacts = crate::data::list(Some(&q), active_id).await?;
    let initial_count = crate::data::count().await?;

    Ok(Props {
        contacts,
        searching: !q.is_empty(),
        q,
        sidebar_contact_count: initial_count,
    })
}
