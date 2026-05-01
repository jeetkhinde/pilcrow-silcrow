pilcrow_web::pilcrow_app!();

#[tokio::main]
async fn main() {
    pilcrow_web::start(pilcrow_router()).await
}
