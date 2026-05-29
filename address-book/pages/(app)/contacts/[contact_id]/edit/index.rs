use pilcrow_web::AppError;
use pilcrow_web::live::*;

pub struct Props {
    pub id: String,
    pub first: String,
    pub last: String,
    pub twitter: String,
    pub avatar: String,
    pub notes: String,
    pub live: Live,
}

#[pilcrow::depends_on_route(contacts, contact_id)]
pub struct Live {
    pub updated_label: LiveProp<String>,
}

impl Live {
    pub fn query(params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        let id = params.get("contact_id").and_then(|v| v.as_str()).unwrap_or("");
        live_query!(
            "SELECT 'Updated ' || to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS UTC') AS updated_label FROM contacts WHERE id = $1",
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
        first: contact.first,
        last: contact.last,
        twitter: contact.twitter,
        avatar: contact.avatar,
        notes: contact.notes,
        live,
    })
}

pub async fn save(req: Req) -> ActionResult {
    let id = parse_id(&req)?;
    let update = crate::data::ContactUpdate {
        first: req.form.get("first").unwrap_or("").to_owned(),
        last: req.form.get("last").unwrap_or("").to_owned(),
        twitter: req.form.get("twitter").unwrap_or("").to_owned(),
        avatar: req.form.get("avatar").unwrap_or("").to_owned(),
        notes: req.form.get("notes").unwrap_or("").to_owned(),
    };
    crate::data::update(&id, update)
        .await?
        .ok_or_else(|| AppError::NotFound("contact not found".into()))?;
    req.fsr.invalidate_route(&format!("/contacts/{id}")).await;
    req.fsr.invalidate_route(&format!("/contacts/{id}/edit")).await;
    req.fsr.invalidate_route("/").await;
    redirect(format!("/contacts/{id}"))
}

fn parse_id(req: &Req) -> AppResult<String> {
    req.params
        .get("contact_id")
        .cloned()
        .ok_or_else(|| AppError::NotFound("invalid contact id".into()))
}
