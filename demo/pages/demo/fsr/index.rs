use std::sync::OnceLock;
use tokio::sync::watch;

static TICK: OnceLock<watch::Sender<u64>> = OnceLock::new();

fn tick_tx() -> &'static watch::Sender<u64> {
    TICK.get_or_init(|| {
        let (tx, _rx) = watch::channel(0u64);
        tokio::spawn(async move {
            let tx = TICK.get().unwrap();
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                tx.send_modify(|v| *v += 1);
            }
        });
        tx
    })
}

pub struct Props {
    pub server_time: String,
    pub live_count: pilcrow_web::LiveProp<u64>,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    let rx = tick_tx().subscribe();
    Ok(Props {
        server_time: now_utc(),
        live_count: pilcrow_web::LiveProp::watch(rx),
    })
}

fn now_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!(
        "{:02}:{:02}:{:02} UTC",
        (secs % 86400) / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}
