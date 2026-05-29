use pilcrow_web::{AppError, AppResult};

#[derive(Clone, Debug, serde::Serialize, sqlx::FromRow)]
pub struct Contact {
    pub id: String,
    pub first: String,
    pub last: String,
    pub avatar: String,
    pub twitter: String,
    pub notes: String,
    pub favorite: bool,
    pub updated_label: String,
}

#[derive(Clone, serde::Serialize, sqlx::FromRow)]
pub struct ContactSummary {
    pub id: String,
    pub name: String,
    pub favorite: bool,
    pub href: String,
    #[sqlx(default)]
    pub active: bool,
}

#[derive(Default)]
pub struct ContactUpdate {
    pub first: String,
    pub last: String,
    pub twitter: String,
    pub avatar: String,
    pub notes: String,
}

pub async fn list(q: Option<&str>, active_id: Option<&str>) -> AppResult<Vec<ContactSummary>> {
    let needle = q.unwrap_or("").trim();
    let rows = if needle.is_empty() {
        sqlx::query_as::<_, ContactSummary>(
            "SELECT id,
                    COALESCE(NULLIF(TRIM(first || ' ' || last), ''), 'No Name') AS name,
                    favorite,
                    '/contacts/' || id AS href
             FROM contacts
             ORDER BY last, created_at DESC",
        )
        .fetch_all(crate::db::pool())
        .await
    } else {
        let pattern = format!("%{needle}%");
        sqlx::query_as::<_, ContactSummary>(
            "SELECT id,
                    COALESCE(NULLIF(TRIM(first || ' ' || last), ''), 'No Name') AS name,
                    favorite,
                    '/contacts/' || id AS href
             FROM contacts
             WHERE first ILIKE $1 OR last ILIKE $1 OR twitter ILIKE $1 OR notes ILIKE $1
             ORDER BY last, created_at DESC",
        )
        .bind(pattern)
        .fetch_all(crate::db::pool())
        .await
    }
    .map_err(|_| AppError::Internal)?;

    Ok(rows
        .into_iter()
        .map(|mut contact| {
            contact.active = active_id == Some(contact.id.as_str());
            contact
        })
        .collect())
}

pub async fn count() -> AppResult<i64> {
    sqlx::query_scalar("SELECT COUNT(*)::bigint FROM contacts")
        .fetch_one(crate::db::pool())
        .await
        .map_err(|_| AppError::Internal)
}

pub async fn get(id: &str) -> AppResult<Option<Contact>> {
    sqlx::query_as::<_, Contact>(
        "SELECT id, first, last, avatar, twitter, notes, favorite,
                'Updated ' || to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS UTC') AS updated_label
         FROM contacts
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(crate::db::pool())
    .await
    .map_err(|_| AppError::Internal)
}

pub async fn create_empty() -> AppResult<Contact> {
    let id = random_id();
    sqlx::query(
        "INSERT INTO contacts (id, first, last, avatar, twitter, notes, favorite)
         VALUES ($1, '', '', 'https://sessionize.com/image/124e-400o400o2-wHVdAuNaxi8KJrgtN3ZKci.jpg', '', '', FALSE)",
    )
    .bind(&id)
    .execute(crate::db::pool())
    .await
    .map_err(|_| AppError::Internal)?;

    get(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("new contact not found".into()))
}

pub async fn update(id: &str, update: ContactUpdate) -> AppResult<Option<Contact>> {
    sqlx::query(
        "UPDATE contacts
         SET first = $1, last = $2, twitter = $3, avatar = $4, notes = $5, updated_at = now()
         WHERE id = $6",
    )
    .bind(update.first.trim())
    .bind(update.last.trim())
    .bind(normalize_twitter(&update.twitter))
    .bind(default_avatar(&update.avatar))
    .bind(update.notes.trim())
    .bind(id)
    .execute(crate::db::pool())
    .await
    .map_err(|_| AppError::Internal)?;

    get(id).await
}

pub async fn delete(id: &str) -> AppResult<bool> {
    let result = sqlx::query("DELETE FROM contacts WHERE id = $1")
        .bind(id)
        .execute(crate::db::pool())
        .await
        .map_err(|_| AppError::Internal)?;
    Ok(result.rows_affected() > 0)
}

pub async fn set_favorite(id: &str, favorite: bool) -> AppResult<Option<Contact>> {
    sqlx::query("UPDATE contacts SET favorite = $1, updated_at = now() WHERE id = $2")
        .bind(favorite)
        .bind(id)
        .execute(crate::db::pool())
        .await
        .map_err(|_| AppError::Internal)?;

    get(id).await
}

pub fn display_name(contact: &Contact) -> String {
    let name = format!("{} {}", contact.first.trim(), contact.last.trim())
        .trim()
        .to_owned();
    if name.is_empty() {
        "No Name".into()
    } else {
        name
    }
}

pub fn normalize_twitter(value: &str) -> String {
    value.trim().trim_start_matches('@').to_owned()
}

fn default_avatar(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "https://sessionize.com/image/124e-400o400o2-wHVdAuNaxi8KJrgtN3ZKci.jpg".into()
    } else {
        value.to_owned()
    }
}

fn random_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("contact-{nanos:x}")
}

pub async fn seed_if_empty(pool: &sqlx::PgPool) -> sqlx::Result<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM contacts")
        .fetch_one(pool)
        .await?;

    if count > 0 {
        return Ok(());
    }

    for contact in SEED_CONTACTS {
        sqlx::query(
            "INSERT INTO contacts (id, first, last, avatar, twitter, notes, favorite)
             VALUES ($1, $2, $3, $4, $5, '', FALSE)
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(contact.id)
        .bind(contact.first)
        .bind(contact.last)
        .bind(contact.avatar)
        .bind(contact.twitter.unwrap_or(""))
        .execute(pool)
        .await?;
    }

    Ok(())
}

struct SeedContact {
    id: &'static str,
    first: &'static str,
    last: &'static str,
    avatar: &'static str,
    twitter: Option<&'static str>,
}

const SEED_CONTACTS: &[SeedContact] = &[
    SeedContact { id: "shruti-kapoor", first: "Shruti", last: "Kapoor", avatar: "https://sessionize.com/image/124e-400o400o2-wHVdAuNaxi8KJrgtN3ZKci.jpg", twitter: Some("@shrutikapoor08") },
    SeedContact { id: "glenn-reyes", first: "Glenn", last: "Reyes", avatar: "https://sessionize.com/image/1940-400o400o2-Enh9dnYmrLYhJSTTPSw3MH.jpg", twitter: Some("@glnnrys") },
    SeedContact { id: "ryan-florence", first: "Ryan", last: "Florence", avatar: "https://sessionize.com/image/9273-400o400o2-3tyrUE3HjsCHJLU5aUJCja.jpg", twitter: None },
    SeedContact { id: "oscar-newman", first: "Oscar", last: "Newman", avatar: "https://sessionize.com/image/d14d-400o400o2-pyB229HyFPCnUcZhHf3kWS.png", twitter: Some("@__oscarnewman") },
    SeedContact { id: "michael-jackson", first: "Michael", last: "Jackson", avatar: "https://sessionize.com/image/fd45-400o400o2-fw91uCdGU9hFP334dnyVCr.jpg", twitter: None },
    SeedContact { id: "christopher-chedeau", first: "Christopher", last: "Chedeau", avatar: "https://sessionize.com/image/b07e-400o400o2-KgNRF3S9sD5ZR4UsG7hG4g.jpg", twitter: Some("@Vjeux") },
    SeedContact { id: "cameron-matheson", first: "Cameron", last: "Matheson", avatar: "https://sessionize.com/image/262f-400o400o2-UBPQueK3fayaCmsyUc1Ljf.jpg", twitter: Some("@cmatheson") },
    SeedContact { id: "brooks-lybrand", first: "Brooks", last: "Lybrand", avatar: "https://sessionize.com/image/820b-400o400o2-Ja1KDrBAu5NzYTPLSC3GW8.jpg", twitter: Some("@BrooksLybrand") },
    SeedContact { id: "alex-anderson", first: "Alex", last: "Anderson", avatar: "https://sessionize.com/image/df38-400o400o2-JwbChVUj6V7DwZMc9vJEHc.jpg", twitter: Some("@ralex1993") },
    SeedContact { id: "kent-c-dodds", first: "Kent C.", last: "Dodds", avatar: "https://sessionize.com/image/5578-400o400o2-BMT43t5kd2U1XstaNnM6Ax.jpg", twitter: Some("@kentcdodds") },
    SeedContact { id: "nevi-shah", first: "Nevi", last: "Shah", avatar: "https://sessionize.com/image/c9d5-400o400o2-Sri5qnQmscaJXVB8m3VBgf.jpg", twitter: Some("@nevikashah") },
    SeedContact { id: "andrew-petersen", first: "Andrew", last: "Petersen", avatar: "https://sessionize.com/image/2694-400o400o2-MYYTsnszbLKTzyqJV17w2q.png", twitter: None },
    SeedContact { id: "scott-smerchek", first: "Scott", last: "Smerchek", avatar: "https://sessionize.com/image/907a-400o400o2-9TM2CCmvrw6ttmJiTw4Lz8.jpg", twitter: Some("@smerchek") },
    SeedContact { id: "giovanni-benussi", first: "Giovanni", last: "Benussi", avatar: "https://sessionize.com/image/08be-400o400o2-WtYGFFR1ZUJHL9tKyVBNPV.jpg", twitter: Some("@giovannibenussi") },
    SeedContact { id: "igor-minar", first: "Igor", last: "Minar", avatar: "https://sessionize.com/image/f814-400o400o2-n2ua5nM9qwZA2hiGdr1T7N.jpg", twitter: Some("@IgorMinar") },
    SeedContact { id: "brandon-kish", first: "Brandon", last: "Kish", avatar: "https://sessionize.com/image/fb82-400o400o2-LbvwhTVMrYLDdN3z4iEFMp.jpeg", twitter: None },
    SeedContact { id: "arisa-fukuzaki", first: "Arisa", last: "Fukuzaki", avatar: "https://sessionize.com/image/fcda-400o400o2-XiYRtKK5Dvng5AeyC8PiUA.png", twitter: Some("@arisa_dev") },
    SeedContact { id: "alexandra-spalato", first: "Alexandra", last: "Spalato", avatar: "https://sessionize.com/image/c8c3-400o400o2-PR5UsgApAVEADZRixV4H8e.jpeg", twitter: Some("@alexadark") },
    SeedContact { id: "cat-johnson", first: "Cat", last: "Johnson", avatar: "https://sessionize.com/image/7594-400o400o2-hWtdCjbdFdLgE2vEXBJtyo.jpg", twitter: None },
    SeedContact { id: "ashley-narcisse", first: "Ashley", last: "Narcisse", avatar: "https://sessionize.com/image/5636-400o400o2-TWgi8vELMFoB3hB9uPw62d.jpg", twitter: Some("@_darkfadr") },
    SeedContact { id: "edmund-hung", first: "Edmund", last: "Hung", avatar: "https://sessionize.com/image/6aeb-400o400o2-Q5tAiuzKGgzSje9ZsK3Yu5.JPG", twitter: Some("@_edmundhung") },
    SeedContact { id: "clifford-fajardo", first: "Clifford", last: "Fajardo", avatar: "https://sessionize.com/image/30f1-400o400o2-wJBdJ6sFayjKmJycYKoHSe.jpg", twitter: Some("@cliffordfajard0") },
    SeedContact { id: "erick-tamayo", first: "Erick", last: "Tamayo", avatar: "https://sessionize.com/image/6faa-400o400o2-amseBRDkdg7wSK5tjsFDiG.jpg", twitter: Some("@ericktamayo") },
    SeedContact { id: "paul-bratslavsky", first: "Paul", last: "Bratslavsky", avatar: "https://sessionize.com/image/feba-400o400o2-R4GE7eqegJNFf3cQ567obs.jpg", twitter: Some("@codingthirty") },
    SeedContact { id: "pedro-cattori", first: "Pedro", last: "Cattori", avatar: "https://sessionize.com/image/c315-400o400o2-spjM5A6VVfVNnQsuwvX3DY.jpg", twitter: Some("@pcattori") },
    SeedContact { id: "andre-landgraf", first: "Andre", last: "Landgraf", avatar: "https://sessionize.com/image/eec1-400o400o2-HkvWKLFqecmFxLwqR9KMRw.jpg", twitter: Some("@AndreLandgraf94") },
    SeedContact { id: "monica-powell", first: "Monica", last: "Powell", avatar: "https://sessionize.com/image/c73a-400o400o2-4MTaTq6ftC15hqwtqUJmTC.jpg", twitter: Some("@indigitalcolor") },
    SeedContact { id: "brian-lee", first: "Brian", last: "Lee", avatar: "https://sessionize.com/image/cef7-400o400o2-KBZUydbjfkfGACQmjbHEvX.jpeg", twitter: Some("@brian_dlee") },
    SeedContact { id: "sean-mcquaid", first: "Sean", last: "McQuaid", avatar: "https://sessionize.com/image/f83b-400o400o2-Pyw3chmeHMxGsNoj3nQmWU.jpg", twitter: Some("@SeanMcQuaidCode") },
    SeedContact { id: "shane-walker", first: "Shane", last: "Walker", avatar: "https://sessionize.com/image/a9fc-400o400o2-JHBnWZRoxp7QX74Hdac7AZ.jpg", twitter: Some("@swalker326") },
    SeedContact { id: "jon-jensen", first: "Jon", last: "Jensen", avatar: "https://sessionize.com/image/6644-400o400o2-aHnGHb5Pdu3D32MbfrnQbj.jpg", twitter: Some("@jenseng") },
];
