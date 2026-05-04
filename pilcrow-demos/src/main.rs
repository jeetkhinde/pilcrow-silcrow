pub mod data;

pilcrow_web::pilcrow_app!();

#[tokio::main]
async fn main() {
    pilcrow_start(pilcrow_router()).await
}
