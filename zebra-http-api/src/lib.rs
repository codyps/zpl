pub async fn zpl_to_png<U: reqwest::IntoUrl>(
    client: reqwest::Client,
    host: U,
    zpl: &str,
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
            ("username", ""),
            ("pw", ""),
            ("data", zpl),
            ("prev", "Preview Label"),
        ])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(eyre::eyre!("request failed: {:?}", response.status()));
    }

    let bytes = response.bytes().await?;

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

    let response = client.get(img_url).send().await?;

    if !response.status().is_success() {
        return Err(eyre::eyre!(
            "request for img failed: {:?}",
            response.status()
        ));
    }

    let bytes = response.bytes().await?;

    Ok(bytes.to_vec())
}
