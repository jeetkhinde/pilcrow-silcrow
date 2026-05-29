use pilcrow_web::{AppError, AppResult};
use serde_json::json;

pub async fn bake_index_pane(total_contacts: i64) -> AppResult<()> {
    let html_path = ".pilcrow-baked/pages/index.html";
    let json_path = ".pilcrow-baked/data/index.json";
    let html = format!(
        r#"<section id="index-page" class="mx-auto flex min-h-[70vh] max-w-2xl flex-col items-center justify-center text-center">
  <p class="mb-3 text-sm font-semibold uppercase tracking-wide text-blue-700">Pilcrow + Silcrow FSR</p>
  <h1 class="text-4xl font-bold tracking-tight text-slate-950 sm:text-5xl">Address Book</h1>
  <p class="mt-4 text-base leading-7 text-slate-600">
    This rebuild uses file routes, Rust loaders/actions, Silcrow-enhanced forms, Postgres-backed
    contacts, Redis-backed FSR, and SQL-driven live slots.
  </p>
  <div class="mt-8 rounded-lg border border-slate-200 bg-white px-6 py-4 shadow-sm">
    <span class="text-3xl font-bold text-slate-950"><span s-live="total_contacts">{total_contacts}</span></span>
    <span class="ml-2 text-sm font-medium text-slate-500">live contacts</span>
  </div>
</section>"#
    );

    write_artifacts("/", html_path, json_path, &html, json!({
        "route": "/",
        "pane": "right",
        "total_contacts": total_contacts,
    }))
    .await
}

pub async fn bake_contact_pane(contact: &crate::data::Contact) -> AppResult<()> {
    let route = format!("/contacts/{}", contact.id);
    let file_name = format!("contacts__{}", safe_file_segment(&contact.id));
    let html_path = format!(".pilcrow-baked/pages/{file_name}.html");
    let json_path = format!(".pilcrow-baked/data/{file_name}.json");
    let name = crate::data::display_name(contact);
    let favorite_mark = if contact.favorite { "*" } else { "\u{2606}" };
    let favorite_value = if contact.favorite { "false" } else { "true" };
    let favorite_label = if contact.favorite {
        "Remove from favorites"
    } else {
        "Add to favorites"
    };
    let twitter = if contact.twitter.is_empty() {
        String::new()
    } else {
        format!(
            r#"
    <p class="mt-2"><a class="font-medium text-blue-700 hover:text-blue-900" href="https://twitter.com/{twitter}">@{twitter}</a></p>"#,
            twitter = escape_attr(&contact.twitter)
        )
    };
    let notes = if contact.notes.is_empty() {
        r#"
    <p class="mt-6 italic text-slate-500">No notes yet.</p>"#
            .to_owned()
    } else {
        format!(
            r#"
    <p class="mt-6 whitespace-pre-line leading-7 text-slate-700">{}</p>"#,
            escape_html(&contact.notes)
        )
    };
    let html = format!(
        r#"<article id="contact" class="mx-auto grid max-w-4xl gap-8 md:grid-cols-[13rem_1fr]">
  <div>
    <img class="h-52 w-52 rounded-lg border border-slate-200 bg-white object-cover shadow-sm" alt="{name} avatar" src="{avatar}" />
  </div>
  <div class="min-w-0">
    <h1 class="flex flex-wrap items-center gap-3 text-4xl font-bold tracking-tight text-slate-950">
      {name}
      <form method="post" action="?/favorite" s-post="?/favorite" s-target="#detail" onsubmit="window.__optimisticStar && window.__optimisticStar(this)">
        <button
          class="rounded-md px-2 text-3xl leading-none text-amber-500 hover:bg-amber-50"
          aria-label="{favorite_label}"
          name="favorite"
          value="{favorite_value}"
          type="submit"><span id="favorite-slot"><span s-live="favorite_mark">{favorite_mark}</span></span></button>
      </form>
    </h1>{twitter}{notes}
    <p class="mt-4 text-sm text-slate-500"><span s-live="updated_label">{updated_label}</span></p>
    <div class="mt-8 flex flex-wrap gap-3">
      <a href="/contacts/{id}/edit" s-get="/contacts/{id}/edit" class="rounded-md border border-slate-300 bg-white px-4 py-2 text-sm font-semibold text-slate-800 shadow-sm hover:bg-slate-50">Edit</a>
      <form method="post" action="?/destroy" s-post="?/destroy" s-target="#detail" onsubmit="return confirm('Please confirm you want to delete this record.')">
        <button class="rounded-md border border-red-200 bg-white px-4 py-2 text-sm font-semibold text-red-700 shadow-sm hover:bg-red-50" type="submit">Delete</button>
      </form>
    </div>
  </div>
</article>"#,
        id = escape_attr(&contact.id),
        name = escape_html(&name),
        avatar = escape_attr(&contact.avatar),
        favorite_label = favorite_label,
        favorite_value = favorite_value,
        favorite_mark = favorite_mark,
        twitter = twitter,
        notes = notes,
        updated_label = escape_html(&contact.updated_label),
    );

    write_artifacts(&route, &html_path, &json_path, &html, json!({
        "route": route,
        "pane": "right",
        "favorite_mark": favorite_mark,
        "updated_label": contact.updated_label,
    }))
    .await
}

async fn write_artifacts(
    route: &str,
    html_path: &str,
    json_path: &str,
    html: &str,
    json_value: serde_json::Value,
) -> AppResult<()> {
    tokio::fs::create_dir_all(".pilcrow-baked/pages")
        .await
        .map_err(|_| AppError::Internal)?;
    tokio::fs::create_dir_all(".pilcrow-baked/data")
        .await
        .map_err(|_| AppError::Internal)?;
    tokio::fs::write(html_path, html)
        .await
        .map_err(|_| AppError::Internal)?;
    let json_bytes = serde_json::to_vec(&json_value).map_err(|_| AppError::Internal)?;
    tokio::fs::write(json_path, json_bytes)
        .await
        .map_err(|_| AppError::Internal)?;

    sqlx::query(
        "UPDATE pilcrow_fsr
         SET promoted = TRUE, html_path = $1, json_path = $2
         WHERE route = $3",
    )
    .bind(html_path)
    .bind(json_path)
    .bind(route)
    .execute(crate::db::pool())
    .await
    .map_err(|_| AppError::Internal)?;

    Ok(())
}

fn safe_file_segment(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_attr(value: &str) -> String {
    escape_html(value)
}
