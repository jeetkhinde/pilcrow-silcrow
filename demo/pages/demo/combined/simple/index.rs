use pilcrow_web::{AsyncValue, LiveProp};
use std::sync::OnceLock;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::watch;
use tokio::time::{interval, sleep};

// ── Regular struct — fine to nest in Props. No special types inside. ──────────
pub struct ProductMeta {
    pub name: String,
    pub category: String,
    pub sku: String,
}

// ── Props ─────────────────────────────────────────────────────────────────────
// AsyncValue, AsyncHtml, and LiveProp MUST be flat top-level fields.
// Codegen scans only the direct fields of Props — types buried inside a
// nested struct are invisible to it and will not get async/live behaviour.
pub struct Props {
    // Normal nested struct — rendered synchronously in the shell. ✓
    pub meta: ProductMeta,

    // AsyncValue: renders empty in shell, scalar value streams in once. ✓
    pub review_count: AsyncValue<i64>,

    // LiveProp: initial value baked into shell, SSE keeps it updated. ✓
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
        // Regular struct: assembled inline, rendered in shell immediately.
        meta: ProductMeta {
            name: "Wireless Headphones".into(),
            category: "Electronics".into(),
            sku: "SKU-WH-2024".into(),
        },

        // AsyncValue: spawn the expensive query; shell shows "" until it resolves.
        review_count: AsyncValue::spawn(async {
            sleep(Duration::from_secs(2)).await; // simulate slow DB
            847_i64
        }),

        // LiveProp: initial value from watch channel, SSE keeps patching.
        stock: LiveProp::watch(stock_tx().subscribe()),
    })
}
