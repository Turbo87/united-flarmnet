#[macro_use]
extern crate tracing;

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::Duration;

use deunicode::deunicode;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::EnvFilter;

use cache::Cache;

mod cache;
mod ogn;
mod weglide;

static FLARMNET_CACHE_DURATION: Duration = Duration::from_secs(60 * 60);

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

    let ogn_ddb_records: HashMap<_, _> = ogn::get_ddb()?
        .into_iter()
        .map(|record| (record.device_id.to_lowercase(), record))
        .collect();

    debug!(ogn_count = ogn_ddb_records.len());

    let weglide_users: HashMap<_, _> = weglide::get_users()?
        .into_iter()
        .map(|record| (record.device.as_ref().unwrap().id.to_lowercase(), record))
        .collect();

    debug!(weglide_count = weglide_users.len());

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
fn get_flarmnet_file() -> anyhow::Result<Vec<flarmnet::Record>> {
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
