#[macro_use]
extern crate tracing;

use reqwest_middleware::ClientBuilder;
use reqwest_tracing::TracingMiddleware;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use crate::sanitize::{sanitize_record_for_lx, sanitize_record_for_xcsoar};
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::EnvFilter;

mod cache;
mod flarmnet;
mod ogn;
mod sanitize;
mod weglide;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Subscriber::builder()
        .pretty()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let client = ClientBuilder::new(reqwest::Client::new())
        .with(TracingMiddleware)
        .build();

    let flarmnet_file = flarmnet::get_flarmnet_file(&client).await?;
    let flarmnet_records: HashMap<_, _> = flarmnet_file
        .records
        .into_iter()
        .map(|record| (record.flarm_id.to_lowercase(), record))
        .collect();

    debug!(flarmnet_count = flarmnet_records.len());

    let ogn_ddb_records: HashMap<_, _> = ogn::get_ddb(&client)
        .await?
        .into_iter()
        .map(|record| (record.device_id.to_lowercase(), record))
        .collect();

    debug!(ogn_count = ogn_ddb_records.len());

    let weglide_devices: HashMap<_, _> = weglide::get_devices(&client)
        .await?
        .into_iter()
        .map(|record| (record.id.to_lowercase(), record))
        .collect();

    debug!(weglide_count = weglide_devices.len());

    info!("merging datasets…");
    let mut merged: HashMap<_, _> = ogn_ddb_records
        .into_iter()
        .map(|(id, it)| (id, it.into_flarmnet_record()))
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

    for (id, device) in weglide_devices {
        let existing_record = merged.get_mut(&id);
        if let Some(existing_record) = existing_record {
            if existing_record.call_sign == device.competition_id.unwrap_or_default() {
                existing_record.pilot_name = device.user.name;

                if existing_record.registration.is_empty() {
                    existing_record.registration = device.name.unwrap_or_default();
                }

                if existing_record.plane_type.is_empty() {
                    existing_record.plane_type =
                        device.aircraft.map(|it| it.name).unwrap_or_default();
                }
            }
        } else {
            merged.insert(id, device.into_flarmnet_record());
        }
    }

    info!("sorting result…");
    let mut merged: Vec<_> = merged.into_iter().map(|(_, record)| record).collect();
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

    Ok(())
}
