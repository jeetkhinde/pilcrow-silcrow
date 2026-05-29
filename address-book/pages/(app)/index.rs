use pilcrow_web::live::*;

pub struct Props {
    pub live: Live,
}

pub const PROMOTE_AFTER: u32 = 0;
pub const FSR_JSON: bool = true;

pub struct Live {
    pub total_contacts: LiveProp<i64>,
}

impl Live {
    pub fn query(_params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        live_query!("SELECT COUNT(*)::bigint AS total_contacts FROM contacts")
    }
}

pub async fn load(_req: Req, live: Live) -> AppResult<Props> {
    crate::bake::bake_index_pane(live.total_contacts.value).await?;
    Ok(Props { live })
}

pub async fn create(req: Req) -> ActionResult {
    let contact = crate::data::create_empty().await?;
    req.fsr.invalidate_route("/").await;
    let path = format!("/contacts/{}/edit", contact.id);
    redirect(&path).retarget("#detail").push_history(&path)
}
