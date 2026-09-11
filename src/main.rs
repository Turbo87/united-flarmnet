#[macro_use]
extern crate tracing;

use crate::sanitize::{
    sanitize_record_for_lx, sanitize_record_for_tdb, sanitize_record_for_xcsoar,
};
use crate::serde::SerializableRecord;
use http_cache_reqwest::{Cache, CacheMode, HttpCache, RedbManager};
use reqwest_middleware::ClientBuilder;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use reqwest_tracing::TracingMiddleware;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use tokio::join;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::EnvFilter;

mod download;
mod flarmnet;
mod ogn;
mod sanitize;
mod serde;
mod weglide;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Subscriber::builder()
        .pretty()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(5);

    let user_agent = format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    let client = reqwest::Client::builder().user_agent(&user_agent).build()?;
    let cache_manager = RedbManager::new("./http-cache.redb").map_err(anyhow::Error::from_boxed)?;
    let client = ClientBuilder::new(client)
        .with(TracingMiddleware::default())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .with(Cache(HttpCache {
            mode: CacheMode::Default,
            manager: cache_manager,
            options: download::cache_options(),
        }))
        .build();

    let flarmnet_fut = flarmnet::get_flarmnet_file(&client);
    let ogn_fut = ogn::get_ddb(&client);
    let weglide_fut = weglide::get_devices(&client);

    let (flarmnet_file, ogn_ddb_records) = join!(flarmnet_fut, ogn_fut);
    let flarmnet_file = flarmnet_file?;
    let ogn_ddb_records = optional_ogn_records(ogn_ddb_records);

    let weglide_devices = match weglide_fut.await {
        Ok(devices) => devices,
        Err(err) => {
            warn!("failed to fetch weglide devices: {err}");
            Vec::new()
        }
    };

    let mut flarmnet_map: HashMap<_, _> = flarmnet_file
        .records
        .into_iter()
        .map(|record| (record.flarm_id.to_lowercase(), record))
        .collect();

    debug!(flarmnet_count = flarmnet_map.len());

    let mut ogn_map: HashMap<_, _> = ogn_ddb_records
        .into_iter()
        .map(|record| (record.device_id.to_lowercase(), record))
        .collect();

    debug!(ogn_count = ogn_map.len());

    let mut weglide_map: HashMap<_, _> = weglide_devices
        .into_iter()
        .map(|record| (record.id.to_lowercase(), record))
        .collect();

    debug!(weglide_count = weglide_map.len());

    info!("merging datasets…");

    let mut ids: HashSet<_> = flarmnet_map.keys().cloned().collect();
    ids.extend(ogn_map.keys().cloned());
    ids.extend(weglide_map.keys().cloned());

    let mut merged: Vec<_> = ids
        .into_iter()
        .filter_map(|id| {
            let flarmnet_record = flarmnet_map.remove(&id);
            let ogn_device = ogn_map.remove(&id);
            let weglide_device = weglide_map.remove(&id);
            merge(flarmnet_record, ogn_device, weglide_device)
        })
        .collect();

    info!("sorting result…");
    merged.sort_unstable_by_key(|a| u32::from_str_radix(&a.record.flarm_id, 16).unwrap());

    merged.iter_mut().for_each(|merged| {
        let record = &mut merged.record;
        if record.airfield == record.registration {
            record.airfield = "".to_string();
        }
    });

    info!("writing united.fln…");
    let path = PathBuf::from("united.fln");
    let file = File::create(path)?;
    let mut writer = ::flarmnet::xcsoar::Writer::new(BufWriter::new(file));

    let xcsoar_records = merged
        .iter()
        .filter_map(|merged| sanitize_record_for_xcsoar(&merged.record))
        .collect();
    let xcsoar_file = ::flarmnet::File {
        version: flarmnet_file.version,
        records: xcsoar_records,
    };
    writer.write(&xcsoar_file)?;

    info!("writing united-lx.fln…");
    let lx_path = PathBuf::from("united-lx.fln");
    let lx_file = File::create(lx_path)?;
    let mut lx_writer = ::flarmnet::lx::Writer::new(BufWriter::new(lx_file));

    let lx_records = merged
        .iter()
        .filter_map(|merged| sanitize_record_for_lx(&merged.record))
        .collect();
    let lx_file = ::flarmnet::File {
        version: flarmnet_file.version,
        records: lx_records,
    };
    lx_writer.write(&lx_file)?;

    info!("writing flarmnet.tdb…");
    let tdb_path = PathBuf::from("flarmnet.tdb");
    let tdb_file = File::create(tdb_path)?;
    let mut tdb_writer = ::flarmnet::tdb::Writer::new(BufWriter::new(tdb_file));

    let tdb_records = merged
        .iter()
        .filter_map(|merged| sanitize_record_for_tdb(&merged.record))
        .collect();
    let tdb_file = ::flarmnet::File {
        version: flarmnet_file.version,
        records: tdb_records,
    };
    tdb_writer.write(&tdb_file)?;

    info!("writing united.json…");
    let json_path = PathBuf::from("united.json");
    let json_file = File::create(json_path)?;
    let json_records: Vec<_> = merged
        .iter()
        .filter_map(|merged| SerializableRecord::from_record(&merged.record, merged.user.as_ref()))
        .collect();
    serde_json::to_writer(BufWriter::new(json_file), &json_records)?;

    Ok(())
}

fn optional_ogn_records(result: anyhow::Result<Vec<ogn::Device>>) -> Vec<ogn::Device> {
    result.unwrap_or_else(|error| {
        warn!("failed to fetch OGN devices: {error}");
        Vec::new()
    })
}

struct MergedRecord {
    record: ::flarmnet::Record,
    user: Option<weglide::UserRef>,
}

fn merge(
    flarmnet_record: Option<::flarmnet::Record>,
    ogn_device: Option<ogn::Device>,
    weglide_device: Option<weglide::Device>,
) -> Option<MergedRecord> {
    let mut merged = ogn_device.map(|it| it.into_flarmnet_record());

    merged = match (merged, flarmnet_record) {
        (None, None) => None,
        (Some(merged), None) => Some(merged),
        (None, Some(flarmnet_record)) => Some(flarmnet_record),
        (Some(mut merged), Some(flarmnet_record)) => {
            if merged.call_sign == flarmnet_record.call_sign {
                merged.pilot_name = flarmnet_record.pilot_name;
                merged.airfield = flarmnet_record.airfield;
                merged.frequency = flarmnet_record.frequency;

                if merged.registration.is_empty() {
                    merged.registration = flarmnet_record.registration;
                }

                if merged.plane_type.is_empty() {
                    merged.plane_type = flarmnet_record.plane_type;
                }
            }
            Some(merged)
        }
    };

    match (merged, weglide_device) {
        (None, None) => None,
        (Some(record), None) => Some(MergedRecord { record, user: None }),
        (None, Some(device)) => {
            let user = Some(device.user.clone());
            Some(MergedRecord {
                record: device.into_flarmnet_record(),
                user,
            })
        }
        (Some(mut merged), Some(device)) => {
            let mut user = None;
            if merged.call_sign == device.competition_id.unwrap_or_default() {
                merged.pilot_name = device.user.name.clone();
                user = Some(device.user);

                if merged.registration.is_empty() {
                    merged.registration = device.name.unwrap_or_default();
                }

                if merged.plane_type.is_empty() {
                    merged.plane_type = device.aircraft.map(|it| it.name).unwrap_or_default();
                }
            }
            Some(MergedRecord {
                record: merged,
                user,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use serde_json::{json, Value};

    #[test]
    fn test_missing_ogn_data_is_ignored() {
        let records = optional_ogn_records(Err(anyhow!("OGN unavailable")));

        assert!(records.is_empty());
    }

    fn export_weglide(user: Value, call_sign: Option<&str>) -> Value {
        let device = serde_json::from_value(json!({
            "id": "ABCDEF",
            "name": "D-1234",
            "competition_id": "XY",
            "until": null,
            "user": user,
        }))
        .unwrap();
        let record = call_sign.map(|call_sign| ::flarmnet::Record {
            flarm_id: "ABCDEF".into(),
            pilot_name: "Existing pilot".into(),
            airfield: String::new(),
            plane_type: String::new(),
            registration: "D-1234".into(),
            call_sign: call_sign.into(),
            frequency: String::new(),
        });
        let merged = merge(record, None, Some(device)).unwrap();
        let record = SerializableRecord::from_record(&merged.record, merged.user.as_ref());
        serde_json::to_value(record).unwrap()
    }

    #[test]
    fn test_weglide_profile_fields() {
        for call_sign in [None, Some("XY")] {
            let exported = export_weglide(
                json!({
                    "id": 123,
                    "name": "Pilot",
                    "image": "123/profile/photo.jpg",
                    "club": { "id": 456, "name": "Gliding Club" },
                }),
                call_sign,
            );
            assert_eq!(
                exported,
                json!({
                    "flarm_id": "ABCDEF",
                    "pilot_name": "Pilot",
                    "registration": "D-1234",
                    "call_sign": "XY",
                    "image_url": "https://files.weglide.org/123/profile/photo.jpg",
                    "weglide_user_id": 123,
                    "club_name": "Gliding Club",
                    "weglide_club_id": 456,
                })
            );
        }
    }

    #[test]
    fn test_missing_weglide_profile_fields() {
        for extra in [
            json!({}),
            json!({"image": null, "club": null}),
            json!({"image": ""}),
        ] {
            let mut user = json!({ "id": 123, "name": "Pilot" });
            user.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            assert_eq!(
                export_weglide(user, None),
                json!({
                    "flarm_id": "ABCDEF",
                    "pilot_name": "Pilot",
                    "registration": "D-1234",
                    "call_sign": "XY",
                    "weglide_user_id": 123,
                })
            );
        }
    }

    #[test]
    fn test_mismatched_weglide_profile_is_omitted() {
        let exported = export_weglide(
            json!({
                "id": 123,
                "name": "Pilot",
                "image": "123/profile/photo.jpg",
                "club": { "id": 456, "name": "Gliding Club" },
            }),
            Some("ZZ"),
        );
        assert_eq!(
            exported,
            json!({
                "flarm_id": "ABCDEF",
                "pilot_name": "Existing pilot",
                "registration": "D-1234",
                "call_sign": "ZZ",
            })
        );
    }
}
