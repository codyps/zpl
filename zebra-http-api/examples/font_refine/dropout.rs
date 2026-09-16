//! Follow-up designed after the first calibration identified thin-stroke residuals.
use super::{campaign, sfnt};
use eyre::{ensure, eyre, Result};
use raster_diff::{compare, Raster};
use serde_json::json;
use std::{fs, path::Path, time::Duration};
use tokio::io::AsyncWriteExt;
pub async fn run(
    host: &reqwest::Url,
    root: &Path,
    glyph_controls: bool,
    offline: bool,
    cap: usize,
) -> Result<()> {
    let dir = root.join(if glyph_controls {
        "dropout-glyph"
    } else {
        "dropout"
    });
    fs::create_dir_all(&dir)?;
    let client = reqwest::Client::builder()
        .http1_title_case_headers()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let modes: &[u8] = if glyph_controls {
        &[2, 5]
    } else {
        &[0, 1, 2, 4, 5]
    };
    let object = |mode: u8| format!("R:ZR{}{mode}.TTF", if glyph_controls { "G" } else { "F" });
    let font = |mode: u8| {
        if glyph_controls {
            sfnt::font_with_glyph_scan(mode)
        } else {
            sfnt::font_with_scan(false, Some(mode))
        }
    };
    let mut pages = vec![];
    let mut fonts = vec![];
    for &mode in modes {
        let name = object(mode);
        let bytes = font(mode);
        fonts.push(json!({"name":name,"sha256":super::super::font_support::sha256(&bytes),"bytes":bytes.len()}));
        super::super::immutable(&dir.join(format!("scan-{mode}.ttf")), &bytes)?;
        for (h, t) in [(16, 0), (32, 0), (64, 0), (32, 1), (32, 2), (32, 3)] {
            campaign::pages(
                &mut pages,
                &format!("scan-{mode}"),
                h,
                0,
                t,
                &name,
                &"ABCDEFGH"
                    .chars()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>(),
                "FT",
                "",
            );
        }
    }
    super::super::immutable(
        &dir.join("manifest.json"),
        &serde_json::to_vec_pretty(
            &json!({"schema":"font-refine-dropout-v1","parent_manifest_sha256":super::super::font_support::sha256(&fs::read(root.join("manifest.json"))?),"requests":pages.len(),"fonts":fonts,"pages":pages.iter().map(|p|p.metadata()).collect::<Vec<_>>()}),
        )?,
    )?;
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
            listing.contains("Directory of: R:*.TTF"),
            "invalid RAM directory response"
        );
        for &mode in modes {
            let name = object(mode);
            let bytes = font(mode);
            let installed = dir.join(format!("installed-{mode}.json"));
            if !listing.contains(&name) {
                let mut stream = tokio::time::timeout(
                    Duration::from_secs(10),
                    tokio::net::TcpStream::connect((host.host_str().unwrap(), 9100)),
                )
                .await??;
                let mut payload = format!("~DY{name},B,T,{},,", bytes.len()).into_bytes();
                payload.extend(bytes);
                tokio::time::timeout(Duration::from_secs(10), stream.write_all(&payload)).await??;
                stream.shutdown().await?;
                super::super::immutable(
                    &installed,
                    b"{\"transport\":\"TCP 9100 binary font download\"}",
                )?;
            } else {
                ensure!(
                    installed.exists(),
                    "RAM object {name} exists without this campaign's installation record"
                );
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
        let listing = client
            .get(host.join("dir?dev=R&otype=TTF")?)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        for &mode in modes {
            ensure!(
                listing.contains(&object(mode)),
                "font download not confirmed"
            );
        }
    }
    let user = std::env::var("ZPL_USERNAME").unwrap_or_default();
    let password = std::env::var("ZPL_PASSWORD").unwrap_or_default();
    let mut measurements = vec![];
    for page in &pages {
        let z = page.zpl();
        super::super::immutable(&dir.join(format!("{}.zpl", page.name)), z.as_bytes())?;
        let png = dir.join(format!("{}.png", page.name));
        let data = if png.exists() {
            super::super::read(&png)?
        } else {
            ensure!(!offline, "missing cached dropout preview");
            super::transport::reserve(
                root,
                &format!(
                    "{}/{}",
                    if glyph_controls {
                        "dropout-glyph"
                    } else {
                        "dropout"
                    },
                    page.name
                ),
                cap,
            )?;
            tokio::time::sleep(Duration::from_millis(500)).await;
            let data = zebra_http_api::zpl_to_png_with_credentials(
                client.clone(),
                host.clone(),
                &z,
                &user,
                &password,
            )
            .await?;
            let r = Raster::decode_png_with_threshold(&data, None).map_err(|e| eyre!(e))?;
            campaign::validate(&r, page)?;
            super::super::immutable(&png, &data)?;
            println!("captured {}", page.name);
            data
        };
        super::super::immutable(
            &png.with_extension("sha256"),
            super::super::font_support::sha256(&data).as_bytes(),
        )?;
        let r = Raster::decode_png_with_threshold(&data, None).map_err(|e| eyre!(e))?;
        campaign::validate(&r, page)?;
        let original = campaign::plan()
            .into_iter()
            .find(|p| {
                p.group == "calibration"
                    && p.probes[0].font == "R:ZRFN.TTF"
                    && p.probes[0].h == page.probes[0].h
                    && p.probes[0].turns == page.probes[0].turns
                    && p.probes[0].w == 0
            })
            .unwrap();
        let old = Raster::decode_png_with_threshold(
            &super::super::read(
                &root
                    .join("development")
                    .join(format!("{}.png", original.name)),
            )?,
            None,
        )
        .map_err(|e| eyre!(e))?;
        let mut records = vec![];
        for (p, q) in page.probes.iter().zip(&original.probes) {
            let tile = campaign::normalize(&r, p);
            let d = compare(&campaign::normalize(&old, q), &tile, false).map_err(|e| eyre!(e))?;
            records.push(json!({"glyph":p.text,"xor_default":d.different_pixels(),"measurement":campaign::measurement(&tile)}));
        }
        measurements.push(json!({"page":page.metadata(),"measurements":records}));
    }
    fs::write(
        dir.join("report.json"),
        serde_json::to_vec_pretty(&json!({"pages":measurements}))?,
    )?;
    Ok(())
}
