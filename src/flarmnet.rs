use flarmnet::xcsoar::DecodedFile;
use reqwest_middleware::ClientWithMiddleware;

#[instrument(skip(client))]
pub async fn get_flarmnet_file(client: &ClientWithMiddleware) -> anyhow::Result<flarmnet::File> {
    info!("Downloading FlarmNet file…");
    let response = client
        .get("https://www.flarmnet.org/static/files/wfn/data.fln")
        .send()
        .await?;
    let response = response.error_for_status()?;
    let content = response.text().await?;
    let decoded_file = flarmnet::xcsoar::decode_file(&content)?;
    Ok(to_file(decoded_file))
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
