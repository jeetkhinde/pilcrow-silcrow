use pilcrow_web::AppError;
use pilcrow_web::live::*;

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

pub struct Live {
    pub total_count: LiveProp<i64>,
    pub open_count: LiveProp<i64>,
    pub normal_count: LiveProp<i64>,
    pub high_count: LiveProp<i64>,
    pub very_high_count: LiveProp<i64>,
    pub critical_count: LiveProp<i64>,
}

impl Live {
    pub fn query(_params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        live_query!(
            "SELECT \
             COUNT(*)::bigint AS total_count, \
             COUNT(*) FILTER (WHERE status = 'open')::bigint AS open_count, \
             COUNT(*) FILTER (WHERE priority = 'normal')::bigint AS normal_count, \
             COUNT(*) FILTER (WHERE priority = 'high')::bigint AS high_count, \
             COUNT(*) FILTER (WHERE priority = 'very_high')::bigint AS very_high_count, \
             COUNT(*) FILTER (WHERE priority = 'critical')::bigint AS critical_count \
             FROM tickets"
        )
    }
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
