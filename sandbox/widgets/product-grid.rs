use serde::Serialize;

#[derive(Serialize)]
struct Row {
    id: u64,
    name: &'static str,
    status: &'static str,
}

pub struct Props {
    pub rows_json: String,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    let rows = vec![
        Row {
            id: 1,
            name: "Routekit",
            status: "Ready",
        },
        Row {
            id: 2,
            name: "Runtime",
            status: "Watching",
        },
        Row {
            id: 3,
            name: "MCP",
            status: "Indexed",
        },
    ];
    Ok(Props {
        rows_json: serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_string()),
    })
}

pub async fn refresh(_req: Req) -> ActionResult {
    Ok(::pilcrow_web::axum::response::IntoResponse::into_response(
        pilcrow_web::json(serde_json::json!({
        "ok": true,
        "message": "Fragment action handled by /widgets/product-grid?/refresh"
        })),
    ))
}
