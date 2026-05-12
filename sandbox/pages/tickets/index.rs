use pilcrow_web::LiveProp;
use std::sync::OnceLock;
use tokio::sync::watch;
static S1: OnceLock<watch::Sender<&'static str>> = OnceLock::new();
static S2: OnceLock<watch::Sender<&'static str>> = OnceLock::new();
static S3: OnceLock<watch::Sender<&'static str>> = OnceLock::new();

fn init(
    slot: &'static OnceLock<watch::Sender<&'static str>>,
) -> &'static watch::Sender<&'static str> {
    slot.get_or_init(|| watch::channel("Open").0)
}

pub struct Props {
    pub status_1: LiveProp<&'static str>,
    pub status_2: LiveProp<&'static str>,
    pub status_3: LiveProp<&'static str>,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        status_1: LiveProp::watch(init(&S1).subscribe()),
        status_2: LiveProp::watch(init(&S2).subscribe()),
        status_3: LiveProp::watch(init(&S3).subscribe()),
    })
}

pub async fn resolve_1(_req: Req) -> ActionResult {
    let t = init(&S1);
    let _ = t.send(if *t.borrow() == "Open" { "Resolved" } else { "Open" });
    ok().with_toast("Status updated", ToastLevel::Success)
}

pub async fn resolve_2(_req: Req) -> ActionResult {
    let t = init(&S2);
    let _ = t.send(if *t.borrow() == "Open" { "Resolved" } else { "Open" });
    ok().with_toast("Status updated", ToastLevel::Success)
}

pub async fn resolve_3(_req: Req) -> ActionResult {
    let t = init(&S3);
    let _ = t.send(if *t.borrow() == "Open" { "Resolved" } else { "Open" });
    ok().with_toast("Status updated", ToastLevel::Success)
}
