use pilcrow_web::AppError;

// REVALIDATE and CACHE_TAGS were removed when ISR was replaced by FSR.
// This page fetches from an external API — it runs plain SSR on every request.

#[derive(serde::Deserialize)]
pub struct ApiResponse {
    pub products: Vec<Product>,
}

#[derive(serde::Deserialize)]
pub struct Product {
    pub id: u32,
    pub title: String,
    pub price: f64,
    pub thumbnail: String,
    pub category: String,
}
pub struct ProductView {
    pub id: u32,
    pub title: String,
    pub price_display: String,
    pub image: String,
    pub category: String,
}
#[derive(serde::Deserialize)]
pub struct CategoryInfo {
    pub slug: String,
    pub name: String,
}

pub struct Category {
    pub label: String,
    pub slug: String,
}

pub struct Props {
    pub products: Vec<ProductView>,
    pub categories: Vec<Category>,
    pub active_category: String,
}

pub async fn load(req: Req) -> AppResult<Props> {
    // Category filter comes from ?category=<slug> — no separate API route needed.
    let active_category = req.query.get("category").unwrap_or("").to_owned();

    let products_url = if active_category.is_empty() {
        "https://dummyjson.com/products?limit=20".to_owned()
    } else {
        format!(
            "https://dummyjson.com/products/category/{}?limit=20",
            urlencoding::encode(&active_category)
        )
    };

    let response: ApiResponse = ::reqwest::get(&products_url)
        .await
        .map_err(|_| AppError::Internal)?
        .json()
        .await
        .map_err(|_| AppError::Internal)?;

    let raw_cats: Vec<CategoryInfo> =
        ::reqwest::get("https://dummyjson.com/products/categories")
            .await
            .map_err(|_| AppError::Internal)?
            .json()
            .await
            .map_err(|_| AppError::Internal)?;

    let categories = raw_cats
        .into_iter()
        .map(|c| Category {
            label: c.name,
            slug: c.slug,
        })
        .collect();

    let products = response.products
        .into_iter()
        .map(|p| ProductView {
            id: p.id,
            title: p.title,
            price_display: format!("${:.2}", p.price),
            image: p.thumbnail,
            category: p.category,
        })
        .collect();

    Ok(Props {
        products,
        categories,
        active_category,
    })
}
