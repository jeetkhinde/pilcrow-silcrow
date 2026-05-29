use pilcrow_web::AppError;
use pilcrow_web::live::*;
use serde::{Deserialize, Serialize};

/// Unit enum serialised as a plain JSON string ("open" / "closed").
#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TicketStatus {
    #[default]
    Open,
    Closed,
}

impl std::fmt::Display for TicketStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Closed => write!(f, "closed"),
        }
    }
}

/// Object live field published to the Silcrow atom "fsr.priority".
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PriorityBadge {
    pub text: String,
    pub class: String,
    pub raw: String,
}

pub struct Props {
    pub id: i64,
    pub title: String,
    pub live: Live,
}

#[pilcrow::depends_on_route(tickets, id)]
pub struct Live {
    pub status: LiveProp<TicketStatus>,

    #[pilcrow::allow_unused]
    pub priority: LiveProp<PriorityBadge>,
}

impl Live {
    pub fn query(params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("0");
        live_query!(
            "SELECT
               status,
               json_build_object(
                 'text', CASE priority
                   WHEN 'normal'    THEN 'Normal'
                   WHEN 'high'      THEN 'High'
                   WHEN 'very_high' THEN 'Very High'
                   WHEN 'critical'  THEN 'Critical'
                   ELSE initcap(priority) END,
                 'class', 'badge badge-' || priority,
                 'raw',   priority
               ) AS priority
             FROM tickets WHERE id = $1::bigint",
            id
        )
    }
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
