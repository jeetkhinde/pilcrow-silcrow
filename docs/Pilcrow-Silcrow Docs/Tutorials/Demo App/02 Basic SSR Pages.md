# 02 — Basic SSR Pages

## Home page

The simplest possible Pilcrow page — static string props, no database.

`pages/index.rs`:

```rust
pub struct Props {
    pub title: &'static str,
    pub message: &'static str,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        title: "Pilcrow",
        message: "Welcome to Pilcrow — an Astro-like web framework for Rust.",
    })
}
```

`pages/index.html`:

```html
<pilcrow:head>
  <title>{{ title }} — Pilcrow</title>
</pilcrow:head>

<h1>{{ title }}</h1>
<p>{{ message }}</p>

<section>
  <h2>React island</h2>
  <react src="/react/Counter.tsx" strategy="visible" initial-count="3" />
</section>

<section>
  <h2>Solid island</h2>
  <solid src="/solid/Counter.tsx" strategy="visible" initial-count="5" />
</section>
```

The `<react>` and `<solid>` tags are Pilcrow island syntax — covered in [[06 React Islands]].

## About page — truly static

`pages/about/index.rs`:

```rust
pub struct Props {
    pub title: &'static str,
    pub description: &'static str,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        title: "About",
        description: "Pilcrow compiles .html templates with Rust frontmatter \
                      into type-safe Askama render functions at build time.",
    })
}
```

No database, no live fields, no islands. Pilcrow renders it on every GET.

## Products page — external API fetch

`pages/products/index.rs` fetches from `dummyjson.com` using `reqwest`:

```rust
use pilcrow_web::AppError;

pub struct ProductView {
    pub id: u32,
    pub title: String,
    pub price_display: String,
    pub image: String,
    pub category: String,
}

pub struct Props {
    pub products: Vec<ProductView>,
    pub categories: Vec<Category>,
    pub active_category: String,
}

pub async fn load(req: Req) -> AppResult<Props> {
    let active_category = req.query.get("category").unwrap_or("").to_owned();

    let url = if active_category.is_empty() {
        "https://dummyjson.com/products?limit=20".to_owned()
    } else {
        format!(
            "https://dummyjson.com/products/category/{}?limit=20",
            urlencoding::encode(&active_category)
        )
    };

    let response: ApiResponse = reqwest::get(&url)
        .await
        .map_err(|_| AppError::Internal)?
        .json()
        .await
        .map_err(|_| AppError::Internal)?;

    // map to ProductView, fetch categories list, return Props
    Ok(Props { products, categories, active_category })
}
```

Key points:
- `req.query.get("category")` reads the `?category=` query param — standard URL search param, no special handling.
- `reqwest::get()` is a plain async HTTP call. Pilcrow has no opinion on how you fetch external data.
- No FSR needed here — the data changes on the external API, not in your database. Every GET re-fetches.

The template uses category filter links with `s-get` to switch categories without a full page reload:

```html
{% for cat in categories %}
<a href="/products?category={{ cat.slug }}"
   s-get="/products?category={{ cat.slug }}"
   class="{% if cat.slug == active_category %}active{% endif %}">
  {{ cat.label }}
</a>
{% endfor %}
```

---

Next: [[03 FSR Dashboard]]
