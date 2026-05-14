use pilcrow_web::AppError;

pub struct Props {
    pub id: i64,
    pub title: String,
    pub live: Live,
}

pub async fn load(req: Req, live: Live) -> AppResult<Props> {
    let id: i64 = req
        .params
        .get("id")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::NotFound("invalid ticket id".into()))?;

    let pool = crate::db::pool();

    let title: Option<String> = sqlx::query_scalar(
        "SELECT title FROM tickets WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::Internal)?;

    let title = title.ok_or_else(|| AppError::NotFound("ticket not found".into()))?;

    Ok(Props { id, title, live })
}
