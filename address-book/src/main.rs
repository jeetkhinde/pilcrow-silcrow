pilcrow_web::pilcrow_app!();

mod data;
mod db;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pilcrow_start(pilcrow_router()).await
}
