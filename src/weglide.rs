use crate::cache::Cache;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use time::serde::rfc3339;
use time::OffsetDateTime;

static WEGLIDE_CACHE_DURATION: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Deserialize, Serialize)]
pub struct Device {
    pub id: String,
    pub name: Option<String>,
    pub competition_id: Option<String>,
    pub aircraft: Option<AircraftRef>,
    #[serde(with = "rfc3339::option")]
    pub until: Option<OffsetDateTime>,
    pub user: UserRef,
}

impl Device {
    pub fn is_current(&self) -> bool {
        self.until.is_none() || self.until.unwrap() >= OffsetDateTime::now_utc()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AircraftRef {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserRef {
    pub id: u32,
    pub name: String,
}

impl Device {
    pub fn into_flarmnet_record(self) -> flarmnet::Record {
        flarmnet::Record {
            flarm_id: self.id,
            pilot_name: self.user.name,
            airfield: "".to_string(),
            plane_type: self.aircraft.map(|it| it.name).unwrap_or_default(),
            registration: self.name.unwrap_or_default(),
            call_sign: self.competition_id.unwrap_or_default(),
            frequency: "".to_string(),
        }
    }
}

#[instrument]
pub async fn get_devices() -> anyhow::Result<Vec<Device>> {
    let cache = Cache::new("weglide-devices.json", WEGLIDE_CACHE_DURATION);
    if cache.needs_update() {
        let devices = download_devices().await?;
        debug!(devices = devices.len());

        let current_devices: Vec<_> = devices.into_iter().filter(|it| it.is_current()).collect();
        debug!(current_devices = current_devices.len());

        let content = serde_json::to_string_pretty(&current_devices)?;
        cache.save(&content)?;
    }

    info!("Reading WeGlide device data…");
    let content = cache.read()?;
    let devices: Vec<Device> = serde_json::from_str(&content)?;
    Ok(devices)
}

#[instrument]
async fn download_devices() -> anyhow::Result<Vec<Device>> {
    info!("Downloading WeGlide device data…");
    let response = reqwest::get("https://api.weglide.org/v1/user/device").await?;
    Ok(response.json().await?)
}
