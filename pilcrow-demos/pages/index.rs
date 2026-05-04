pub struct Props {}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {})
}
