use pilcrow_web::{AsyncHtml, AsyncValue};

const TEMPLATE_SRC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/pages/deferred/index.html"
));
const CODEBEHIND_SRC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/pages/deferred/index.rs"
));

pub struct Props {
    pub rendered_at: String,
    pub slow_count: AsyncValue<u64>,
    pub post_list: AsyncHtml,
    pub template_src: &'static str,
    pub codebehind_src: &'static str,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        rendered_at: now_utc(),
        slow_count: AsyncValue::spawn(async {
            tokio::time::sleep(std::time::Duration::from_millis(0000)).await;
            crate::data::counter::increment()
        }),
        post_list: AsyncHtml::spawn(async {
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            render_posts()
        })
        .with_loading("<div class='skeleton-list'><div class='skeleton-item'></div><div class='skeleton-item'></div><div class='skeleton-item'></div></div>"),
        template_src: TEMPLATE_SRC,
        codebehind_src: CODEBEHIND_SRC,
    })
}

fn render_posts() -> String {
    let posts = [
        ("Building fast Rust web apps", "20 min read"),
        ("ISR vs SSG: when to use each", "4 min read"),
        ("Streaming HTML with Tokio", "3 min read"),
        ("Deferred fields in practice", "5 min read"),
    ];
    maud::html! {
        ul class="post-list" {
            @for (title, meta) in &posts {
                li class="post-item" {
                    span class="post-title" { (title) }
                    span class="post-meta" { (meta) }
                }
            }
        }
    }
    .into_string()
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
