const TEMPLATE_SRC: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/pages/streaming/index.html"));
const CODEBEHIND_SRC: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/pages/streaming/index.rs"));

pub const STREAMING: bool = true;

#[derive(serde::Serialize)]
pub struct Props {
    pub render_count: u64,
    pub rendered_at: String,
    pub slow_message: String,
    pub template_src: &'static str,
    pub codebehind_src: &'static str,
}

impl Default for Props {
    fn default() -> Self {
        Self {
            render_count: 0,
            rendered_at: "Loading…".into(),
            slow_message: "Fetching…".into(),
            template_src: TEMPLATE_SRC,
            codebehind_src: CODEBEHIND_SRC,
        }
    }
}

pub async fn load(_req: Req) -> AppResult<Props> {
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    Ok(Props {
        render_count: crate::data::counter::increment(),
        rendered_at: now_utc(),
        slow_message: "Data arrived via stream patch!".into(),
        template_src: TEMPLATE_SRC,
        codebehind_src: CODEBEHIND_SRC,
    })
}

fn now_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02} UTC", h, m, s)
}
