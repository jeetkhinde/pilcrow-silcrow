use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::watch;

static WATCHERS_TX: OnceLock<watch::Sender<u32>> = OnceLock::new();

fn watchers_tx() -> &'static watch::Sender<u32> {
    WATCHERS_TX.get_or_init(|| {
        let (tx, _) = watch::channel(1u32);
        tokio::spawn(async move {
            let mut tick: u32 = 0;
            let mut interval = tokio::time::interval(Duration::from_secs(3));
            loop {
                interval.tick().await;
                tick = tick.wrapping_add(1);
                // Simple deterministic oscillation: no external rand crate needed.
                WATCHERS_TX.get().unwrap().send_modify(|v| {
                    *v = 5 + (tick % 7) + ((tick / 3) % 5);
                });
            }
        });
        tx
    })
}

pub struct Props {
    /// LiveProp scoped to this island — opens its own SSE connection at
    /// /__pilcrow/live/demo/async-live/islands/activity, independent from the
    /// parent page's SSE connection.
    pub watchers: pilcrow_web::LiveProp<u32>,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        watchers: pilcrow_web::LiveProp::watch(watchers_tx().subscribe()),
    })
}
