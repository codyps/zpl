use clap::Parser;
use eyre::Result;
use std::net::SocketAddr;
use tokio::{net::TcpListener, sync::watch};
use zebra_simulator::{serve_tcp, Config, Simulator};

#[derive(Parser)]
#[command(about = "Simulate a network Zebra printer and browse its live request history")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:9100")]
    tcp_addr: SocketAddr,
    #[arg(long, default_value = "127.0.0.1:9200")]
    json_addr: SocketAddr,
    #[arg(long, default_value = "127.0.0.1:9170")]
    http_addr: SocketAddr,
    #[arg(long, default_value = "_simulator.sqlite")]
    database: String,
    #[arg(long, default_value_t = 812)]
    width: u32,
    #[arg(long, default_value_t = 1218)]
    height: u32,
    #[arg(long, default_value = "ZD621 simulator")]
    model: String,
    #[arg(long, default_value = "SIM000000001")]
    serial: String,
    #[arg(long, default_value = "SIM.0.1.0")]
    firmware: String,
}
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let app = Simulator::open(
        &args.database,
        Config {
            width: args.width,
            height: args.height,
            model: args.model,
            serial: args.serial,
            firmware: args.firmware,
        },
    )?;
    let tcp = TcpListener::bind(args.tcp_addr).await?;
    let json = TcpListener::bind(args.json_addr).await?;
    let http = TcpListener::bind(args.http_addr).await?;
    eprintln!(
        "Printer TCP: {}; SGD JSON: {}; request log: http://{}/",
        tcp.local_addr()?,
        json.local_addr()?,
        http.local_addr()?
    );
    let (stop, receiver) = watch::channel(false);
    // Drop the HTTP serve future on shutdown to also close long-lived SSE
    // streams. Drain TCP handlers so partial buffered requests are persisted.
    let tcp_server = serve_tcp(tcp, app.clone(), false, receiver.clone());
    let json_server = serve_tcp(json, app.clone(), true, receiver);
    let http_server = async {
        tokio::select! {
            result = axum::serve(http, app.router()) => result?,
            result = tokio::signal::ctrl_c() => { result?; stop.send_replace(true); }
        }
        Ok::<_, eyre::Report>(())
    };
    tokio::try_join!(tcp_server, json_server, http_server)?;
    Ok(())
}
