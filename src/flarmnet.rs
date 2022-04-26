use crate::cache::Cache;
use flarmnet::xcsoar::DecodedFile;
use std::time::Duration;

static FLARMNET_CACHE_DURATION: Duration = Duration::from_secs(60 * 60);

#[instrument]
pub async fn get_flarmnet_file() -> anyhow::Result<flarmnet::File> {
    let cache = Cache::new("flarmnet.fln", FLARMNET_CACHE_DURATION);
    if cache.needs_update() {
        let content = download_flarmnet_file().await?;
        cache.save(&content)?;
    }

    info!("reading FlarmNet file…");
    let content = cache.read()?;
    let decoded_file = flarmnet::xcsoar::decode_file(&content)?;
    Ok(to_file(decoded_file))
}

#[instrument]
async fn download_flarmnet_file() -> anyhow::Result<String> {
    info!("downloading FlarmNet file…");
    let response = reqwest::get("https://www.flarmnet.org/static/files/wfn/data.fln").await?;
    Ok(response.text().await?)
}

fn to_file(decoded: DecodedFile) -> flarmnet::File {
    let records = decoded
        .records
        .into_iter()
        .filter_map(|res| res.ok())
        .collect();

    flarmnet::File {
        version: decoded.version,
        records,
    }
}
