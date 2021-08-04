#[macro_use]
extern crate tracing;

use anyhow::Context;
use deunicode::deunicode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::EnvFilter;

static FLARMNET_CACHE_DURATION: Duration = Duration::from_secs(60 * 60);
static OGN_DDB_CACHE_DURATION: Duration = Duration::from_secs(60 * 60);
static WEGLIDE_CACHE_DURATION: Duration = Duration::from_secs(60 * 60);

fn main() -> anyhow::Result<()> {
    Subscriber::builder()
        .pretty()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let flarmnet_records: HashMap<_, _> = get_flarmnet_file()?
        .into_iter()
        .map(|record| (record.flarm_id.to_lowercase(), record))
        .collect();

    debug!(flarmnet_count = flarmnet_records.len());

    let ogn_ddb_records: HashMap<_, _> = get_ogn_ddb_data()?
        .into_iter()
        .map(|record| (record.device_id.to_lowercase(), record))
        .collect();

    debug!(ogn_count = ogn_ddb_records.len());

    let weglide_users: HashMap<_, _> = get_weglide_users()?
        .into_iter()
        .map(|record| (record.device.as_ref().unwrap().id.to_lowercase(), record))
        .collect();

    debug!(weglide_count = weglide_users.len());

    warn!(weglide = ?get_weglide_users()?.len());

    info!("merging datasets…");
    let mut merged: HashMap<_, _> = ogn_ddb_records
        .into_iter()
        .map(|(id, it)| {
            (
                id,
                flarmnet::Record {
                    flarm_id: it.device_id,
                    pilot_name: "".to_string(),
                    airfield: "".to_string(),
                    plane_type: it.aircraft_model,
                    registration: it.registration,
                    call_sign: it.cn,
                    frequency: "".to_string(),
                },
            )
        })
        .collect();

    for (id, record) in flarmnet_records {
        let existing_record = merged.get_mut(&id);
        if let Some(existing_record) = existing_record {
            if existing_record.call_sign == record.call_sign {
                existing_record.pilot_name = record.pilot_name;
                existing_record.airfield = record.airfield;
                existing_record.frequency = record.frequency;

                if existing_record.registration.is_empty() {
                    existing_record.registration = record.registration;
                }

                if existing_record.plane_type.is_empty() {
                    existing_record.plane_type = record.plane_type;
                }
            }
        } else {
            merged.insert(id, record);
        }
    }

    for (id, user) in weglide_users {
        let device = user.device.unwrap();

        let existing_record = merged.get_mut(&id);
        if let Some(existing_record) = existing_record {
            if existing_record.call_sign == device.competition_id.unwrap_or_default() {
                existing_record.pilot_name = deunicode(&user.name);

                if existing_record.registration.is_empty() {
                    existing_record.registration =
                        device.name.map(|it| deunicode(&it)).unwrap_or_default();
                }

                if existing_record.airfield.is_empty()
                    || existing_record.airfield == existing_record.registration
                {
                    existing_record.airfield = user
                        .home_airport
                        .map(|it| deunicode(&it.name))
                        .unwrap_or_default();
                }

                if existing_record.plane_type.is_empty() {
                    existing_record.plane_type = device
                        .aircraft
                        .map(|it| deunicode(&it.name))
                        .unwrap_or_default();
                }
            }
        } else {
            merged.insert(
                id,
                flarmnet::Record {
                    flarm_id: device.id,
                    pilot_name: user.name,
                    airfield: user.home_airport.map(|it| it.name).unwrap_or_default(),
                    plane_type: device.aircraft.map(|it| it.name).unwrap_or_default(),
                    registration: device.name.unwrap_or_default(),
                    call_sign: device.competition_id.unwrap_or_default(),
                    frequency: "".to_string(),
                },
            );
        }
    }

    info!("sorting result…");
    let mut merged: Vec<_> = merged.into_iter().map(|(_, record)| record).collect();
    merged.sort_unstable_by(|a, b| a.flarm_id.cmp(&b.flarm_id));

    merged.iter_mut().for_each(|record| {
        if record.airfield == record.registration {
            record.airfield = "".to_string();
        }
    });

    let merged_file = flarmnet::File {
        version: 1,
        records: merged,
    };

    info!("writing united.fln…");
    let path = PathBuf::from("united.fln");
    let file = File::create(path)?;
    let mut writer = flarmnet::xcsoar::Writer::new(BufWriter::new(file));
    writer.write(&merged_file)?;

    info!("writing united-lx.fln…");
    let lx_path = PathBuf::from("united-lx.fln");
    let lx_file = File::create(lx_path)?;
    let mut lx_writer = flarmnet::lx::Writer::new(BufWriter::new(lx_file));
    lx_writer.write(&merged_file)?;

    Ok(())
}

#[instrument]
fn ensure_cache_folder() -> anyhow::Result<PathBuf> {
    let path = PathBuf::from(".cache");
    if !path.exists() {
        info!("creating cache folder…");
        fs::create_dir(&path).context("Failed to create cache folder")?;
    }
    Ok(path)
}

#[instrument]
fn needs_update(path: &Path, cache_duration: &Duration) -> bool {
    let metadata = match path.metadata() {
        Ok(metadata) => metadata,
        Err(_) => return true,
    };

    let modified = match metadata.modified() {
        Ok(modified) => modified,
        Err(_) => return true,
    };

    let elapsed = match modified.elapsed() {
        Ok(elapsed) => elapsed,
        Err(_) => return false,
    };

    elapsed > *cache_duration
}

#[instrument]
fn get_flarmnet_file() -> anyhow::Result<Vec<flarmnet::Record>> {
    let cache_path = ensure_cache_folder()?;

    let path = cache_path.join("flarmnet.fln");
    let needs_update = needs_update(&path, &FLARMNET_CACHE_DURATION);
    debug!(?path, needs_update);

    if needs_update {
        let content = download_flarmnet_file()?;
        fs::write(&path, content)?;
    }

    info!("reading FlarmNet file…");
    let content = fs::read_to_string(&path)?;
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

#[instrument]
fn get_ogn_ddb_data() -> anyhow::Result<Vec<OgnDdbDevice>> {
    let cache_path = ensure_cache_folder()?;

    let path = cache_path.join("ogn-ddb.json");
    let needs_update = needs_update(&path, &OGN_DDB_CACHE_DURATION);
    debug!(?path, needs_update);

    if needs_update {
        let content = download_ogn_ddb_data()?;
        fs::write(&path, content)?;
    }

    info!("reading OGN DDB…");
    let content = fs::read_to_string(&path)?;
    let ogn_ddb: OgnDdb = serde_json::from_str(&content)?;
    Ok(ogn_ddb.devices)
}

#[derive(Debug, Deserialize)]
struct OgnDdb {
    devices: Vec<OgnDdbDevice>,
}

/// ```json
/// {
///     "device_type": "F",
///     "device_id": "000000",
///     "aircraft_model": "HPH 304CZ-17",
///     "registration": "OK-7777",
///     "cn": "KN",
///     "tracked": "Y",
///     "identified": "Y",
///     "aircraft_type": "1"
/// }
/// ```
#[derive(Debug, Deserialize)]
struct OgnDdbDevice {
    device_type: String,
    device_id: String,
    aircraft_model: String,
    registration: String,
    cn: String,
    tracked: String,
    identified: String,
    aircraft_type: String,
}

#[instrument]
fn download_ogn_ddb_data() -> anyhow::Result<String> {
    info!("downloading OGN DDB…");
    let response = ureq::get("http://ddb.glidernet.org/download/?j=1&t=1").call()?;
    Ok(response.into_string()?)
}

#[derive(Debug, Deserialize, Serialize)]
struct WeglideUser {
    id: u32,
    name: String,
    home_airport: Option<WeglideAirport>,
    device: Option<WeglideDevice>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WeglideDevice {
    id: String,
    name: Option<String>,
    competition_id: Option<String>,
    aircraft: Option<WeglideAircraft>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WeglideAircraft {
    id: u32,
    name: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct WeglideAirport {
    id: u32,
    name: String,
}

#[instrument]
fn get_weglide_users() -> anyhow::Result<Vec<WeglideUser>> {
    let cache_path = ensure_cache_folder()?;

    let path = cache_path.join("weglide-users.json");
    let needs_update = needs_update(&path, &WEGLIDE_CACHE_DURATION);
    debug!(?path, needs_update);

    if needs_update {
        let all_users = download_all_weglide_users()?;
        debug!(all_users = all_users.len());

        let filtered_users: Vec<_> = all_users
            .into_iter()
            .filter(|it| it.device.is_some())
            .collect();
        debug!(filtered_users = filtered_users.len());

        let content = serde_json::to_string_pretty(&filtered_users)?;
        fs::write(&path, content)?;
    }

    info!("reading WeGlide user data…");
    let content = fs::read_to_string(&path)?;
    let users: Vec<WeglideUser> = serde_json::from_str(&content)?;
    Ok(users)
}

#[instrument]
fn download_all_weglide_users() -> anyhow::Result<Vec<WeglideUser>> {
    info!("downloading WeGlide user data…");
    let mut start = 1u32;
    let limit = 100u32;
    let mut all = Vec::new();
    loop {
        let ids: Vec<u32> = (start..start + limit).collect();

        let page = download_weglide_users(&ids)?;
        let page_len = page.len();
        debug!(page_len);

        all.extend(page);

        if page_len == 0 {
            return Ok(all);
        }

        start += limit;
    }
}

#[instrument(skip(ids))]
fn download_weglide_users(ids: &[u32]) -> anyhow::Result<Vec<WeglideUser>> {
    let ids: Vec<_> = ids.iter().map(|id| id.to_string()).collect();
    let ids = ids.join(",");

    info!("downloading WeGlide user data page…");
    let url = format!("https://api.weglide.org/v1/user?id_in={}&limit=100", ids);
    let response = ureq::get(&url).call()?;
    Ok(response.into_json()?)
}
