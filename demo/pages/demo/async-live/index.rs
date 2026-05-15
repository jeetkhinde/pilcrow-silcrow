use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::watch;

static STATUS_TX: OnceLock<watch::Sender<String>> = OnceLock::new();

fn status_tx() -> &'static watch::Sender<String> {
    STATUS_TX.get_or_init(|| {
        let (tx, _) = watch::channel("Generating...".to_string());
        tx
    })
}

pub struct Props {
    pub title: String,
    /// AsyncHTML: empty shell + skeleton while the expensive render runs.
    pub summary: pilcrow_web::AsyncHtml,
    /// LiveProp: bakes "Generating..." into the shell, then SSE patches it to
    /// "Ready" the moment the AsyncHtml future completes.
    pub status: pilcrow_web::LiveProp<String>,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    let tx = status_tx();
    // Reset for this page load so each visitor sees the transition.
    let _ = tx.send("Generating...".to_string());

    Ok(Props {
        title: "Q1 2025 Product Analysis".into(),

        summary: pilcrow_web::AsyncHtml::spawn(async {
            // Simulate expensive server-side work (DB joins, template render, etc.)
            tokio::time::sleep(Duration::from_secs(2)).await;
            // Signal the LiveProp channel once the work is done.
            if let Some(tx) = STATUS_TX.get() {
                let _ = tx.send("Ready".to_string());
            }
            render_summary_html()
        })
        .with_loading("<p class='summary-loading'>⏳ Generating summary…</p>"),

        status: pilcrow_web::LiveProp::watch(tx.subscribe()),
    })
}

fn render_summary_html() -> String {
    r#"<div class="summary-body">
      <p>Strong performance across all categories. Revenue up 18% YoY.</p>
      <ul>
        <li>Electronics: $1.2M <span class="up">▲ 22%</span></li>
        <li>Apparel: $840K <span class="up">▲ 14%</span></li>
        <li>Home goods: $620K <span class="up">▲ 9%</span></li>
      </ul>
    </div>"#
    .to_string()
}
