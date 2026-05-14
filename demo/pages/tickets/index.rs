use pilcrow_web::AppError;

#[derive(sqlx::FromRow)]
pub struct Ticket {
    pub id: i64,
    pub title: String,
    pub status: String,
    pub priority: String,
}

pub struct Props {
    pub tickets: Vec<Ticket>,
    pub live: Live,
}

pub async fn load(_req: Req, live: Live) -> AppResult<Props> {
    let pool = crate::db::pool();

    let tickets = sqlx::query_as::<_, Ticket>(
        "SELECT id, title, status, priority FROM tickets ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::Internal)?;

    Ok(Props { tickets, live })
}
