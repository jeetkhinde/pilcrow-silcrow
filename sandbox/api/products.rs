use pilcrow_web::axum::{self, extract::Query, response::Html, routing};

#[derive(serde::Deserialize, Default)]
struct Product {
    title: String,
    price: f64,
    thumbnail: String,
    category: String,
}

#[derive(serde::Deserialize, Default)]
struct ApiResponse {
    products: Vec<Product>,
}

#[derive(serde::Deserialize)]
struct Params {
    category: Option<String>,
}

async fn get(Query(params): Query<Params>) -> Html<String> {
    let url = match params.category.as_deref().filter(|s| !s.is_empty()) {
        Some(cat) => format!(
            "https://dummyjson.com/products/category/{}?limit=20",
            urlencoding::encode(cat)
        ),
        None => "https://dummyjson.com/products?limit=20".to_string(),
    };

    let resp: ApiResponse = match reqwest::get(&url).await {
        Ok(r) => r.json().await.unwrap_or_default(),
        Err(_) => ApiResponse::default(),
    };

    Html(render_cards(&resp.products))
}

fn render_cards(products: &[Product]) -> String {
    if products.is_empty() {
        return r#"<p style="color:#888;padding:1rem 0">No products found.</p>"#.to_string();
    }
    products
        .iter()
        .map(|p| {
            format!(
                r#"<div class="product-card">
  <img src="{img}" alt="{title}" loading="lazy" />
  <div class="product-info">
    <span class="product-category">{cat}</span>
    <h3 class="product-title">{title}</h3>
    <p class="product-price">${price:.2}</p>
  </div>
</div>"#,
                img = escape(&p.thumbnail),
                title = escape(&p.title),
                cat = escape(&p.category),
                price = p.price,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn router() -> axum::Router {
    axum::Router::new().route("/", routing::get(get))
}
