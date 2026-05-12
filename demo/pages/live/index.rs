use std::sync::OnceLock;
use tokio::sync::watch;

static COUNTER: OnceLock<watch::Sender<u64>> = OnceLock::new();

fn counter_tx() -> &'static watch::Sender<u64> {
    COUNTER.get_or_init(|| {
        let (tx, _rx) = watch::channel(0u64);
        // Increment every 2 seconds so we can observe live updates without user interaction.
        tokio::spawn(async move {
            let tx = COUNTER.get().unwrap();
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
    pub count: pilcrow_web::LiveProp<u64>,
    pub label: String,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    let rx = counter_tx().subscribe();
    Ok(Props {
        count: pilcrow_web::LiveProp::watch(rx),
        label: "Live counter".to_string(),
    })
}
