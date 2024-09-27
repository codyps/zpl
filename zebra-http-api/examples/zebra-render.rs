use clap::Parser;
use reqwest::header::HeaderName;

#[derive(Parser, Debug)]
struct Args {
    /// The URL of a zebra printer to use to preview render the zpl. `http://foo.local/`, etc.
    #[clap(long)]
    host: reqwest::Url,

    #[clap(long, short = 'H')]
    header: Vec<String>,

    zpl_input_file: std::path::PathBuf,
    png_output_file: std::path::PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if args.header.len() % 2 != 0 {
        eprintln!("Headers must be key-value pairs");
        std::process::exit(1);
    }

    let zpl = std::fs::read_to_string(&args.zpl_input_file).unwrap();
    let client = reqwest::Client::builder()
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            for kv in args.header.chunks(2) {
                let key = &kv[0];
                let value = &kv[1];
                let key: HeaderName = key.parse().unwrap();
                headers.insert(key, value.parse().unwrap());
            }
            headers
        })
        .build()
        .unwrap();
    let png = zebra_http_api::zpl_to_png(client, args.host, &zpl)
        .await
        .unwrap();
    std::fs::write(&args.png_output_file, &png).unwrap();
}
