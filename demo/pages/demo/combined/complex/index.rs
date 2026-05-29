use pilcrow_web::LiveProp;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::watch;
use tokio::time::interval;

// ── Regular nested structs (no special types — safe to nest) ──────────────────

pub struct ReportHeader {
    pub title: String,
    pub period: String,
    pub generated_by: String,
}

pub struct SalesBreakdown {
    pub top_category: String,
    pub growth_pct: f64,
}

// ── Props (used only by the initial HTTP render / load()) ──────────────────────
pub struct Props {
    // SSR nested structs — rendered synchronously in shell.
    pub header: ReportHeader,
    pub sales: SalesBreakdown,

    pub report_body: String,
    pub total_orders: i64,
    pub viewers: LiveProp<u32>,
    pub health: LiveProp<String>,
}

// ── LiveProps (used only by the SSE route via live()) ─────────────────────────
//
// Pattern B: declare a separate LiveProps + live() so that the expensive
// load() (DB queries, async HTML render) never runs for SSE reconnects.
// Every field here must also exist in Props with the same type.
pub struct LiveProps {
    pub viewers: LiveProp<u32>,
    pub health: LiveProp<String>,
}

// ── Live state ────────────────────────────────────────────────────────────────

static VIEWERS_TX: OnceLock<watch::Sender<u32>> = OnceLock::new();
static HEALTH_TX: OnceLock<watch::Sender<String>> = OnceLock::new();

fn viewers_tx() -> &'static watch::Sender<u32> {
    VIEWERS_TX.get_or_init(|| {
        let (tx, _) = watch::channel(1u32);
        spawn(async move {
            let mut tick: u32 = 0;
            let mut interval = interval(Duration::from_secs(3));
            loop {
                interval.tick().await;
                tick = tick.wrapping_add(1);
                VIEWERS_TX.get().unwrap().send_modify(|v| {
                    *v = 3 + (tick % 12);
                });
            }
        });
        tx
    })
}

fn health_tx() -> &'static watch::Sender<String> {
    HEALTH_TX.get_or_init(|| {
        let (tx, _) = watch::channel("Healthy".to_string());
        spawn(async move {
            let statuses = ["Healthy", "Healthy", "Healthy", "Degraded", "Healthy"];
            let mut i = 0usize;
            let mut interval = interval(Duration::from_secs(7));
            loop {
                interval.tick().await;
                i = (i + 1) % statuses.len();
                let _ = HEALTH_TX.get().unwrap().send(statuses[i].to_string());
            }
        });
        tx
    })
}

// ── load() — runs once per HTTP request ───────────────────────────────────────
pub async fn load(_req: Req) -> AppResult<Props> {
    // Expensive synchronous work: runs only on the initial page load.
    let header = ReportHeader {
        title: "Q1 2025 Executive Report".into(),
        period: "Jan 1 – Mar 31, 2025".into(),
        generated_by: "analytics-svc".into(),
    };
    let sales = SalesBreakdown {
        top_category: "Electronics".into(),
        growth_pct: 18.4,
    };

    Ok(Props {
        header,
        sales,

        report_body: render_report_html(),
        total_orders: 14_392_i64,
        viewers: LiveProp::initial(0),
        health: LiveProp::initial("Healthy".to_string()),
    })
}

// ── live() — runs for every SSE connection / reconnect ────────────────────────
//
// load() is NOT called here. Only LiveProps fields are needed — the watch
// channels provide the initial SSE value from *rx.borrow().
pub async fn live(_req: Req) -> AppResult<LiveProps> {
    Ok(LiveProps {
        viewers: LiveProp::watch(viewers_tx().subscribe()),
        health: LiveProp::watch(health_tx().subscribe()),
    })
}

fn render_report_html() -> String {
    r#"<div class="report-body">
      <p>Revenue: <strong>$4.2M</strong> — up 18.4% YoY.</p>
      <ul>
        <li>Electronics: $1.8M <span class="up">▲ 22%</span></li>
        <li>Apparel: $1.1M <span class="up">▲ 14%</span></li>
        <li>Home goods: $820K <span class="up">▲ 9%</span></li>
        <li>Other: $480K <span class="flat">→ 1%</span></li>
      </ul>
    </div>"#
        .to_string()
}
