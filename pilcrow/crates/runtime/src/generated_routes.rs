#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated_routes.rs"));
}

#[allow(dead_code)]
mod generated_api {
    include!(concat!(env!("OUT_DIR"), "/generated_api_routes.rs"));
}

pub use generated::*;
pub use generated_api::*;
