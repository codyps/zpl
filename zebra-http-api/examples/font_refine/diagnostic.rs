//! Adaptive, predeclared probes to distinguish the surviving spacing hypotheses.
use super::campaign;
use eyre::{ensure, eyre, Result};
use image_diff::{compare, Raster};
use serde_json::json;
use std::{fs, path::Path, time::Duration};
pub fn pages() -> Vec<campaign::Page> {
    let mut pages = vec![];
    let strings: Vec<_> = ["||", "|l|", "|ll|", "|llll|", "|H|", "|HH|"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for h in [11, 12] {
        campaign::pages(
            &mut pages,
            "advance-discriminator",
            h,
            0,
            0,
            "0",
            &strings,
            "FT",
            "",
        );
    }
    for font in ["0", "R:ZZNOFNT.TTF"] {
        campaign::pages(
            &mut pages,
            "missing-font-control",
            32,
            0,
            0,
            font,
            &"HIl|OonmgjpAV_."
                .chars()
                .map(|c| c.to_string())
                .collect::<Vec<_>>(),
            "FT",
            "",
        );
    }
    pages
}
pub async fn run(host: &reqwest::Url, root: &Path, offline: bool, cap: usize) -> Result<()> {
    let dir = root.join("diagnostic");
    fs::create_dir_all(&dir)?;
    let pages = pages();
    super::super::immutable(
        &dir.join("manifest.json"),
        &serde_json::to_vec_pretty(
            &json!({"schema":"font-refine-diagnostic-v1","requests":pages.len(),"predictions_before_capture":{"l_at_11":{"floor":2,"nearest":3},"H_at_12":{"nearest":7,"ceil":8}},"pages":pages.iter().map(|p|p.metadata()).collect::<Vec<_>>()}),
        )?,
    )?;
    let client = reqwest::Client::builder()
        .http1_title_case_headers()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    if !offline {
        super::transport::check_host(root, host)?;
        let listing = client
            .get(host.join("dir?dev=R&otype=TTF")?)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        ensure!(
            listing.contains("Directory of: R:*.TTF") && !listing.contains("R:ZZNOFNT.TTF"),
            "missing-font control object exists or listing failed"
        );
    }
    let user = std::env::var("ZPL_USERNAME").unwrap_or_default();
    let password = std::env::var("ZPL_PASSWORD").unwrap_or_default();
    let mut records = vec![];
    let mut rasters = vec![];
    for page in &pages {
        let z = page.zpl();
        super::super::immutable(&dir.join(format!("{}.zpl", page.name)), z.as_bytes())?;
        let png = dir.join(format!("{}.png", page.name));
        let bytes = if png.exists() {
            super::super::read(&png)?
        } else {
            ensure!(!offline, "missing diagnostic capture");
            super::transport::reserve(root, &format!("diagnostic/{}", page.name), cap)?;
            tokio::time::sleep(Duration::from_millis(500)).await;
            let bytes = zebra_http_api::zpl_to_png_with_credentials(
                client.clone(),
                host.clone(),
                &z,
                &user,
                &password,
            )
            .await?;
            campaign::validate(
                &Raster::decode_png_with_threshold(&bytes, None).map_err(|e| eyre!(e))?,
                page,
            )?;
            super::super::immutable(&png, &bytes)?;
            bytes
        };
        super::super::immutable(
            &png.with_extension("sha256"),
            super::super::font_support::sha256(&bytes).as_bytes(),
        )?;
        let r = Raster::decode_png_with_threshold(&bytes, None).map_err(|e| eyre!(e))?;
        campaign::validate(&r, page)?;
        records.push(json!({"page":page.name,"measurements":page.probes.iter().map(|p|campaign::measurement(&campaign::normalize(&r,p))).collect::<Vec<_>>()}));
        rasters.push(r);
    }
    let d = compare(&rasters[2], &rasters[3], false).map_err(|e| eyre!(e))?;
    let distance = |page: usize, index: usize| {
        let runs = records[page]["measurements"][index]["column_runs"]
            .as_array()
            .unwrap();
        runs.last().unwrap()[0].as_i64().unwrap() - runs[0][0].as_i64().unwrap()
    };
    fs::write(
        dir.join("report.json"),
        serde_json::to_vec_pretty(
            &json!({"pages":records,"missing_font_vs_font0_xor":d.different_pixels(),"l_advance_at_11":distance(0,1)-distance(0,0),"H_advance_at_12":distance(1,4)-distance(1,0),"sealed_pixels_read":false}),
        )?,
    )?;
    Ok(())
}
