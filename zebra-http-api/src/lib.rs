/// A completed preview response without an image. Distinct from an outage or
/// authentication/transport failure so callers can classify repeated rejection.
#[derive(Debug)]
pub struct NoPreview;
impl std::fmt::Display for NoPreview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("no image found")
    }
}
impl std::error::Error for NoPreview {}
/// RAM object identity for one preview session. Reuse it across that session's
/// requests so the printer does not accumulate a new object for every page.
/// Eight uppercase base32 characters keep the object name within 8.3 limits.
#[derive(Debug)]
pub struct PreviewObject(String);

impl PreviewObject {
    pub fn new() -> Result<Self, eyre::Report> {
        let mut bytes = [0u8; 5];
        getrandom::fill(&mut bytes).map_err(|e| eyre::eyre!("preview object randomness: {e}"))?;
        let value = bytes.iter().fold(0u64, |v, &b| (v << 8) | u64::from(b));
        const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
        let name = (0..8)
            .rev()
            .map(|i| char::from(ALPHABET[((value >> (i * 5)) & 31) as usize]))
            .collect();
        Ok(Self(name))
    }

    pub fn name(&self) -> &str {
        &self.0
    }
}

// Preserve the free-function API while giving its callers one random object per
// process. Callers managing independent sessions can supply their own object.
fn process_object() -> Result<&'static PreviewObject, eyre::Report> {
    static OBJECT: std::sync::OnceLock<Result<PreviewObject, String>> = std::sync::OnceLock::new();
    OBJECT
        .get_or_init(|| PreviewObject::new().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| eyre::eyre!("{e}"))
}

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
    zpl_to_png_with_object(client, host, zpl, username, password, process_object()?).await
}

/// Preview using the RAM object owned by this session. The object name stays out
/// of ZPL content, allowing image caches to survive restarts with a new name.
/// Requests must still be serialized: firmware may share other preview state.
pub async fn zpl_to_png_with_object<U: reqwest::IntoUrl>(
    client: reqwest::Client,
    host: U,
    zpl: &str,
    username: &str,
    password: &str,
    object: &PreviewObject,
) -> Result<Vec<u8>, eyre::Report> {
    let host = host.into_url()?;
    let host = host.join("zpl")?;
    let response = client
        .post(host.clone())
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("dev", "R"),
            ("oname", object.name()),
            ("otype", "ZPL"),
            ("username", username),
            ("pw", password),
            ("data", zpl),
            ("prev", "Preview Label"),
        ])
        .send()
        .await?;

    let response = response.error_for_status()?;
    eyre::ensure!(
        response.status().is_success(),
        "unexpected preview redirect"
    );

    let bytes = bounded_body(response).await?;

    // There's only 1 img tag in the returned html, and it's always all caps, one line, and with
    // src attr first. Skip html parsing.
    let prefix = b"<IMG SRC=\"";
    let img_src_index = memchr::memmem::find(&bytes, prefix).ok_or(NoPreview)? + prefix.len();
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
    async fn objects_are_random_per_session_and_reused_for_each_page() {
        let first = PreviewObject::new().unwrap();
        let second = PreviewObject::new().unwrap();
        assert_ne!(first.name(), second.name());
        for object in [&first, &second] {
            assert_eq!(object.name().len(), 8);
            assert!(object
                .name()
                .bytes()
                .all(|b| b.is_ascii_uppercase() || (b'2'..=b'7').contains(&b)));
        }
        let (host, task) = server(
            (0..3)
                .flat_map(|_| [response(b"<IMG SRC=\"/image\">"), response(b"png-test")])
                .collect(),
        );
        for object in [&first, &first, &second] {
            zpl_to_png_with_object(client(), host.clone(), "^XA^XZ", "", "", object)
                .await
                .unwrap();
        }
        let requests = task.join().unwrap();
        for (index, object) in [(0, &first), (2, &first), (4, &second)] {
            assert!(requests[index].contains(&format!("oname={}&otype=ZPL", object.name())));
            assert!(!requests[index].contains("TEST1"));
        }
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
        assert!(requests[0].contains(&format!(
            "oname={}&otype=ZPL",
            process_object().unwrap().name()
        )));
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
