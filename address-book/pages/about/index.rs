pub struct Props {}

pub const PROMOTE_AFTER: u32 = 0;

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {})
}
