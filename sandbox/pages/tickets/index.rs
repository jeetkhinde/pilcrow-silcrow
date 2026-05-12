use std::sync::OnceLock;
use tokio::sync::watch;

static S1: OnceLock<watch::Sender<&'static str>> = OnceLock::new();
static S2: OnceLock<watch::Sender<&'static str>> = OnceLock::new();
static S3: OnceLock<watch::Sender<&'static str>> = OnceLock::new();

fn init(slot: &'static OnceLock<watch::Sender<&'static str>>) -> &'static watch::Sender<&'static str> {
    slot.get_or_init(|| watch::channel("Open").0)
}

pub struct Props {
    pub status_1: pilcrow_web::LiveProp<&'static str>,
    pub status_2: pilcrow_web::LiveProp<&'static str>,
    pub status_3: pilcrow_web::LiveProp<&'static str>,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        status_1: pilcrow_web::LiveProp::watch(init(&S1).subscribe()),
        status_2: pilcrow_web::LiveProp::watch(init(&S2).subscribe()),
        status_3: pilcrow_web::LiveProp::watch(init(&S3).subscribe()),
    })
}

pub async fn resolve_1(_req: Req) -> pilcrow_web::ActionResult {
    let t = init(&S1);
    let _ = t.send(if *t.borrow() == "Open" { "Resolved" } else { "Open" });
    Ok(pilcrow_web::axum::response::IntoResponse::into_response(
        pilcrow_web::navigate("/tickets"),
    ))
}

pub async fn resolve_2(_req: Req) -> pilcrow_web::ActionResult {
    let t = init(&S2);
    let _ = t.send(if *t.borrow() == "Open" { "Resolved" } else { "Open" });
    Ok(pilcrow_web::axum::response::IntoResponse::into_response(
        pilcrow_web::navigate("/tickets"),
    ))
}

pub async fn resolve_3(_req: Req) -> pilcrow_web::ActionResult {
    let t = init(&S3);
    let _ = t.send(if *t.borrow() == "Open" { "Resolved" } else { "Open" });
    Ok(pilcrow_web::axum::response::IntoResponse::into_response(
        pilcrow_web::navigate("/tickets"),
    ))
}
