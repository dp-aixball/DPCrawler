#[tokio::main(flavor = "current_thread")]
async fn main() {
    println!("Starting DPCrawler API Server in HEADLESS mode...");
    // Load config to grab port if needed, or stick to 18088
    let port = 18088;
    dpcrawler_lib::server::run_server(port).await;
}
