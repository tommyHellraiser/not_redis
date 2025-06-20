mod modules;
mod utils;

#[tokio::main]
async fn main() {
    modules::api::start_api().await.unwrap();
}
