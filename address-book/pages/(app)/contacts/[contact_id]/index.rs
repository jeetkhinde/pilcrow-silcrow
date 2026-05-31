use pilcrow_web::AppError;
use pilcrow_web::live::*;

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct FavoriteMark {
    pub text: String,
    pub class: String,
}

pub struct Props {
    pub id: String,
    pub name: String,
    pub first: String,
    pub last: String,
    pub avatar: String,
    pub twitter: String,
    pub notes: String,
    pub favorite: bool,
    pub live: Live,
}

#[pilcrow::depends_on_route(contacts, contact_id)]
pub struct Live {
    #[pilcrow::allow_unused]
    pub favorite_mark: LiveProp<FavoriteMark>,
    pub updated_label: LiveProp<String>,
}

impl Live {
    pub fn query(params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        let id = params
            .get("contact_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        live_query!(
            "SELECT
               json_build_object(
                 'text', CASE WHEN favorite THEN '★' ELSE '☆' END,
                 'class', CASE WHEN favorite THEN 'text-amber-500' ELSE 'text-slate-400' END
               ) AS favorite_mark,
               'Updated ' || to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS UTC') AS updated_label
             FROM contacts
             WHERE id = $1",
            id
        )
    }
}

pub async fn load(req: Req, live: Live) -> AppResult<Props> {
    let id = parse_id(&req)?;
    let contact = crate::data::get(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("contact not found".into()))?;

    Ok(Props {
        id,
        name: crate::data::display_name(&contact),
        first: contact.first,
        last: contact.last,
        avatar: contact.avatar,
        twitter: contact.twitter,
        notes: contact.notes,
        favorite: contact.favorite,
        live,
    })
}

pub async fn favorite(req: Req) -> ActionResult {
    let id = parse_id(&req)?;
    let favorite = req.form.get("favorite").unwrap_or("false") == "true";
    crate::data::set_favorite(&id, favorite)
        .await?
        .ok_or_else(|| AppError::NotFound("contact not found".into()))?;
    req.fsr.invalidate_dep_key("contacts", &id).await;
    let path = format!("/contacts/{id}");
    redirect(&path).retarget("#detail").push_history(&path)
}

pub async fn destroy(req: Req) -> ActionResult {
    let id = parse_id(&req)?;
    crate::data::delete(&id).await?;
    req.fsr.tombstone(&format!("/contacts/{id}")).await;
    req.fsr.invalidate_route("/").await;
    redirect("/").retarget("#detail").push_history("/")
}

fn parse_id(req: &Req) -> AppResult<String> {
    req.params
        .get("contact_id")
        .cloned()
        .ok_or_else(|| AppError::NotFound("invalid contact id".into()))
}
