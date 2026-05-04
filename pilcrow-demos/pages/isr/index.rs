const TEMPLATE_SRC: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/pages/isr/index.html"));
const CODEBEHIND_SRC: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/pages/isr/index.rs"));

pub const REVALIDATE: u64 = 2;
pub const MAX_STALE: u64 = 10;
pub const CACHE_TAGS: &[&str] = &["demo-isr"];

pub struct Props {
    pub render_count: u64,
    pub rendered_at: String,
    pub template_src: &'static str,
    pub codebehind_src: &'static str,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        render_count: crate::data::counter::increment(),
        rendered_at: now_utc(),
        template_src: TEMPLATE_SRC,
        codebehind_src: CODEBEHIND_SRC,
    })
}

pub async fn bust(req: Req) -> ActionResult {
    req.cache.revalidate_tag("demo-isr");
    pilcrow_web::redirect("/isr")
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
