pub struct Props {
    pub title: String,
    pub summary: String,
    pub status: pilcrow_web::LiveProp<String>,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        title: "Q1 2025 Product Analysis".into(),
        summary: render_summary_html(),
        status: pilcrow_web::LiveProp::initial("Ready".to_string()),
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
