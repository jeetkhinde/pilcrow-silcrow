pilcrow_web::pilcrow_app!();

mod baked_pages;

#[tokio::main]
async fn main() {
    pilcrow_web::start(baked_pages::router().merge(pilcrow_router())).await
}
