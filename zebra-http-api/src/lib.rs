/// Render ZPL using the printer's HTTP preview endpoint.
///
/// Configure `client` with `reqwest::Client::builder().http1_title_case_headers()`:
/// some Zebra firmware rejects lowercase HTTP/1 header names with status 400.
pub async fn zpl_to_png<U: reqwest::IntoUrl>(
    client: reqwest::Client,
    host: U,
    zpl: &str,
) -> Result<Vec<u8>, eyre::Report> {
    zpl_to_png_with_credentials(client, host, zpl, "", "").await
}

/// Preview with optional form credentials. Use a client with a bounded timeout
/// and a same-origin redirect policy when supplying credentials.
pub async fn zpl_to_png_with_credentials<U: reqwest::IntoUrl>(
    client: reqwest::Client,
    host: U,
    zpl: &str,
    username: &str,
    password: &str,
) -> Result<Vec<u8>, eyre::Report> {
    let host = host.into_url()?;
    let host = host.join("zpl")?;
    let response = client
        .post(host.clone())
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("dev", "R"),
            ("oname", "TEST1"),
            ("otype", "ZPL"),
            ("username", username),
            ("pw", password),
            ("data", zpl),
            ("prev", "Preview Label"),
        ])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(eyre::eyre!("request failed: {:?}", response.status()));
    }

    let bytes = bounded_body(response).await?;

    // There's only 1 img tag in the returned html, and it's always all caps, one line, and with
    // src attr first. Skip html parsing.
    let prefix = b"<IMG SRC=\"";
    let img_src_index = memchr::memmem::find(&bytes, prefix)
        .ok_or_else(|| eyre::eyre!("no image found"))?
        + prefix.len();
    let img_src_end = memchr::memchr(b'"', &bytes[img_src_index..])
        .ok_or_else(|| eyre::eyre!("missing end quote"))?;
    let img_src = &bytes[img_src_index..img_src_index + img_src_end];

    let img_src = std::str::from_utf8(img_src)?;

    let img_src = htmlize::unescape_attribute(img_src);

    let img_url = host.join(img_src.as_ref())?;

    if img_url.origin() != host.origin()
        || !img_url.username().is_empty()
        || img_url.password().is_some()
    {
        return Err(eyre::eyre!(
            "cross-origin or credential-bearing preview image rejected"
        ));
    }
    let response = client.get(img_url).send().await?;

    if !response.status().is_success() {
        return Err(eyre::eyre!(
            "request for img failed: {:?}",
            response.status()
        ));
    }

    let bytes = bounded_body(response).await?;

    Ok(bytes)
}

async fn bounded_body(mut response: reqwest::Response) -> Result<Vec<u8>, eyre::Report> {
    const LIMIT: usize = 16 * 1024 * 1024;
    if response.content_length().is_some_and(|n| n > LIMIT as u64) {
        return Err(eyre::eyre!("preview response exceeds 16 MiB"));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if chunk.len() > LIMIT - body.len() {
            return Err(eyre::eyre!("preview response exceeds 16 MiB"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };
    fn server(responses: Vec<Vec<u8>>) -> (String, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = format!("http://{}/", listener.local_addr().unwrap());
        let task = thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut b = [0; 4096];
                    let n = stream.read(&mut b).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&b[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (k, v) = line.split_once(':')?;
                                if k.eq_ignore_ascii_case("content-length") {
                                    v.trim().parse::<usize>().ok()
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8(bytes).unwrap());
                stream.write_all(&response).unwrap();
            }
            requests
        });
        (host, task)
    }
    fn response(body: &[u8]) -> Vec<u8> {
        let mut r = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        r.extend(body);
        r
    }
    fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .http1_title_case_headers()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
    }
    #[tokio::test]
    async fn credentials_form_and_preview_image() {
        let (host, server) = server(vec![
            response(b"<IMG SRC=\"/image?a=1&amp;b=2\">"),
            response(b"png-test"),
        ]);
        assert_eq!(
            zpl_to_png_with_credentials(client(), host, "^XA^XZ", "sample", "test pass")
                .await
                .unwrap(),
            b"png-test"
        );
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("POST /zpl "));
        assert!(requests[0].contains("username=sample&pw=test+pass"));
        assert!(requests[1].starts_with("GET /image?a=1&b=2 "));
        assert!(!requests[1].contains("test+pass"));
    }
    #[tokio::test]
    async fn cross_origin_and_excessive_body_rejected() {
        let (host, t) = server(vec![response(b"<IMG SRC=\"http://other.invalid/image\">")]);
        assert!(zpl_to_png(client(), host, "^XA^XZ")
            .await
            .unwrap_err()
            .to_string()
            .contains("cross-origin"));
        t.join().unwrap();
        let (host, t) = server(vec![
            b"HTTP/1.1 200 OK\r\nContent-Length: 16777217\r\nConnection: close\r\n\r\n".to_vec(),
        ]);
        assert!(zpl_to_png(client(), host, "^XA^XZ")
            .await
            .unwrap_err()
            .to_string()
            .contains("16 MiB"));
        t.join().unwrap();
    }
}
