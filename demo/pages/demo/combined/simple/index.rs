use pilcrow_web::LiveProp;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::watch;
use tokio::time::interval;

// ── Regular struct — fine to nest in Props. No special types inside. ──────────
pub struct ProductMeta {
    pub name: String,
    pub category: String,
    pub sku: String,
}

pub struct Props {
    pub meta: ProductMeta,
    pub review_count: i64,
    pub stock: LiveProp<u32>,
}

// ── Live state ────────────────────────────────────────────────────────────────
static STOCK_TX: OnceLock<watch::Sender<u32>> = OnceLock::new();

fn stock_tx() -> &'static watch::Sender<u32> {
    STOCK_TX.get_or_init(|| {
        let (tx, _) = watch::channel(42u32);
        spawn(async move {
            let mut tick: u32 = 0;
            let mut interval = interval(Duration::from_secs(4));
            loop {
                interval.tick().await;
                tick = tick.wrapping_add(1);
                STOCK_TX.get().unwrap().send_modify(|v| {
                    // Oscillate between 38 and 55 so the demo is visible.
                    *v = 38 + (tick % 18);
                });
            }
        });
        tx
    })
}

// ── load ──────────────────────────────────────────────────────────────────────
pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        meta: ProductMeta {
            name: "Wireless Headphones".into(),
            category: "Electronics".into(),
            sku: "SKU-WH-2024".into(),
        },
        review_count: 847_i64,
        stock: LiveProp::watch(stock_tx().subscribe()),
    })
}
