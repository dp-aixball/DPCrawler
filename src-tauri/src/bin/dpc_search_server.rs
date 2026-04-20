use std::env;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let mut port: u16 = 18088; // 设定原厂包容性后备端口

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-p" | "--port" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or_else(|_| {
                        eprintln!("错误: 端口必须为纯数字!");
                        std::process::exit(1);
                    });
                    i += 1;
                }
            }
            "-h" | "--help" => {
                println!("DPCrawler Search API / Headless Node");
                println!("Options:");
                println!("  -p, --port <PORT>   指定服务端口 (默认: 18088)");
                println!("  -h, --help          查看帮助");
                std::process::exit(0);
            }
            _ => {
                println!("Ignored unknown argument: {}", args[i]);
            }
        }
        i += 1;
    }

    println!(
        "Starting DPCrawler API Server in HEADLESS mode Binding on :{}...",
        port
    );
    dpcrawler_lib::server::run_server(port).await;
}
