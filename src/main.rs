#[macro_use]
extern crate tracing;

use crate::sanitize::{sanitize_record_for_lx, sanitize_record_for_xcsoar};
use crate::serde::SerializableRecord;
use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
use reqwest_middleware::ClientBuilder;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use reqwest_tracing::TracingMiddleware;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use tokio::try_join;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::EnvFilter;

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
    let client = ClientBuilder::new(client)
        .with(TracingMiddleware::default())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .with(Cache(HttpCache {
            mode: CacheMode::Default,
            manager: CACacheManager::default(),
            options: HttpCacheOptions::default(),
        }))
        .build();

    let flarmnet_fut = flarmnet::get_flarmnet_file(&client);
    let ogn_fut = ogn::get_ddb(&client);
    let weglide_fut = weglide::get_devices(&client);

    let (flarmnet_file, ogn_ddb_records) = try_join!(flarmnet_fut, ogn_fut)?;

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
    merged.sort_unstable_by_key(|a| u32::from_str_radix(&a.flarm_id, 16).unwrap());

    merged.iter_mut().for_each(|record| {
        if record.airfield == record.registration {
            record.airfield = "".to_string();
        }
    });

    info!("writing united.fln…");
    let path = PathBuf::from("united.fln");
    let file = File::create(path)?;
    let mut writer = ::flarmnet::xcsoar::Writer::new(BufWriter::new(file));

    let xcsoar_records = merged.iter().map(sanitize_record_for_xcsoar).collect();
    let xcsoar_file = ::flarmnet::File {
        version: flarmnet_file.version,
        records: xcsoar_records,
    };
    writer.write(&xcsoar_file)?;

    info!("writing united-lx.fln…");
    let lx_path = PathBuf::from("united-lx.fln");
    let lx_file = File::create(lx_path)?;
    let mut lx_writer = ::flarmnet::lx::Writer::new(BufWriter::new(lx_file));

    let lx_records = merged.iter().map(sanitize_record_for_lx).collect();
    let lx_file = ::flarmnet::File {
        version: flarmnet_file.version,
        records: lx_records,
    };
    lx_writer.write(&lx_file)?;

    info!("writing united.json…");
    let json_path = PathBuf::from("united.json");
    let json_file = File::create(json_path)?;
    let json_records: Vec<_> = merged
        .iter()
        .filter_map(SerializableRecord::from_record)
        .collect();
    serde_json::to_writer(BufWriter::new(json_file), &json_records)?;

    Ok(())
}

fn merge(
    flarmnet_record: Option<::flarmnet::Record>,
    ogn_device: Option<ogn::Device>,
    weglide_device: Option<weglide::Device>,
) -> Option<::flarmnet::Record> {
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
        (Some(merged), None) => Some(merged),
        (None, Some(weglide_device)) => Some(weglide_device.into_flarmnet_record()),
        (Some(mut merged), Some(device)) => {
            if merged.call_sign == device.competition_id.unwrap_or_default() {
                merged.pilot_name = device.user.name;

                if merged.registration.is_empty() {
                    merged.registration = device.name.unwrap_or_default();
                }

                if merged.plane_type.is_empty() {
                    merged.plane_type = device.aircraft.map(|it| it.name).unwrap_or_default();
                }
            }
            Some(merged)
        }
    }
}
