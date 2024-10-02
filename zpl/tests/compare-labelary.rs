use reqwest::Client;
use std::io::Cursor;
use std::fs::File;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let zpl = "^xa^cfa,50^fo100,100^fdHello World^fs^xz";

    // adjust print density (8dpmm), label width (4 inches), label height (6 inches), and label index (0) as necessary
    let url = "http://api.labelary.com/v1/printers/8dpmm/labels/4x6/0/";
    let response = Client::new()
        .post(url)
        .body(zpl)
        .header("Accept", "application/pdf") // omit this line to get PNG images back
        .send().await?;

    if response.status().is_success() {
        let mut file = File::create("label.pdf")?; // change file name for PNG images
        let mut content = Cursor::new(response.bytes().await?);
        std::io::copy(&mut content, &mut file)?;
    } else {
        let error_message = response.text().await?;
        eprintln!("{}", error_message);
    }

    Ok(())
}
