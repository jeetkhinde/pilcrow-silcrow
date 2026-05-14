pilcrow_web::pilcrow_app!();

mod baked_pages;
mod db;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pilcrow_start(baked_pages::router().merge(pilcrow_router())).await
}
