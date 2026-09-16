//! Shared capture accounting and cleanup of this campaign's temporary objects.
use eyre::{ensure, eyre, Result};
use serde_json::json;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
pub struct Lock(PathBuf);
impl Lock {
    pub fn acquire(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)?;
        let path = root.join("capture.lock");
        fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|_|eyre!("capture.lock exists; another client may be active (remove only after checking)"))?;
        Ok(Self(path))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
pub fn reserve(root: &Path, name: &str, cap: usize) -> Result<()> {
    ensure!(cap <= 200, "request cap exceeds 200");
    let ledger = root.join("requests.jsonl");
    let previous = if ledger.exists() {
        let data = super::super::read(&ledger)?;
        let text = std::str::from_utf8(&data)?;
        let last = text
            .lines()
            .last()
            .ok_or_else(|| eyre!("empty request ledger"))?;
        serde_json::from_str::<serde_json::Value>(last)?["total"]
            .as_u64()
            .ok_or_else(|| eyre!("invalid ledger"))? as usize
    } else {
        let mut count = 0;
        for d in [
            "development",
            "sealed",
            "rejected-preview-download",
            "dropout",
            "dropout-glyph",
            "diagnostic",
        ] {
            let p = root.join(d);
            if p.exists() {
                for f in fs::read_dir(p)? {
                    if f?.path().extension().is_some_and(|s| s == "png") {
                        count += 1;
                    }
                }
            }
        }
        count
    };
    ensure!(previous < cap, "preview budget exhausted: {previous}/{cap}");
    let record = json!({"total":previous+1,"request":name,"unix_seconds":SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),"counting":"reserved before send; initial count includes pre-ledger saved PNGs"});
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(ledger)?;
    writeln!(f, "{record}")?;
    f.sync_all()?;
    Ok(())
}
pub fn check_host(root: &Path, host: &reqwest::Url) -> Result<()> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&super::super::read(&root.join("manifest.json"))?)?;
    ensure!(
        manifest["host"].as_str() == Some(host.as_str()),
        "printer host differs from capture manifest"
    );
    Ok(())
}
pub async fn cleanup(host: &reqwest::Url, root: &Path) -> Result<()> {
    check_host(root, host)?;
    ensure!(
        root.join("calibration-install.json").exists(),
        "no calibration ownership record"
    );
    let mut names = vec!["ZRFN".to_string(), "ZRFH".to_string()];
    for (dir, prefix, modes) in [
        ("dropout", "ZRF", &[0, 1, 2, 4, 5][..]),
        ("dropout-glyph", "ZRG", &[2, 5][..]),
    ] {
        for mode in modes {
            if root
                .join(dir)
                .join(format!("installed-{mode}.json"))
                .exists()
            {
                names.push(format!("{prefix}{mode}"));
            }
        }
    }
    let client = reqwest::Client::builder()
        .http1_title_case_headers()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let password = std::env::var("ZPL_PASSWORD").unwrap_or_default();
    for name in &names {
        let response = client
            .get(host.join("delete")?)
            .query(&[
                ("dev", "R"),
                ("oname", name),
                ("otype", "TTF"),
                ("confirm", "Delete"),
                ("pw", password.as_str()),
            ])
            .send()
            .await
            .map_err(|_| eyre!("font deletion request failed (URL omitted)"))?;
        ensure!(
            response.status().is_success(),
            "delete request failed for {name}"
        );
    }
    let listing = client
        .get(host.join("dir?dev=R&otype=TTF")?)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    ensure!(
        listing.contains("Directory of: R:*.TTF"),
        "invalid directory response"
    );
    for name in &names {
        ensure!(
            !listing.contains(&format!("R:{name}.TTF")),
            "temporary font {name} still exists; deletion was not confirmed"
        );
    }
    super::super::immutable(
        &root.join("cleanup.json"),
        &serde_json::to_vec_pretty(
            &json!({"removed_ram_objects":names,"directory_confirmed_absent":true}),
        )?,
    )?;
    println!(
        "Removed {} temporary fonts; RAM directory confirms absence",
        names.len()
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn request_budget_counts_failed_attempts() {
        let root = std::env::temp_dir().join(format!(
            "zpl-budget-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        reserve(&root, "failed", 1).unwrap();
        assert!(reserve(&root, "retry", 1).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
