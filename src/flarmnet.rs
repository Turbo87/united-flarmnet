use crate::cache::Cache;
use std::time::Duration;

static FLARMNET_CACHE_DURATION: Duration = Duration::from_secs(60 * 60);

#[instrument]
pub fn get_flarmnet_file() -> anyhow::Result<Vec<flarmnet::Record>> {
    let cache = Cache::new("flarmnet.fln", FLARMNET_CACHE_DURATION);
    if cache.needs_update() {
        let content = download_flarmnet_file()?;
        cache.save(&content)?;
    }

    info!("reading FlarmNet file…");
    let content = cache.read()?;
    let decoded_file = flarmnet::xcsoar::decode_file(&content)?;
    Ok(decoded_file
        .records
        .into_iter()
        .filter_map(|res| res.ok())
        .collect())
}

#[instrument]
fn download_flarmnet_file() -> anyhow::Result<String> {
    info!("downloading FlarmNet file…");
    let response = ureq::get("https://www.flarmnet.org/static/files/wfn/data.fln").call()?;
    Ok(response.into_string()?)
}
