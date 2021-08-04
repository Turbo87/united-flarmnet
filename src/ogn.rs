use crate::cache::Cache;
use serde::Deserialize;
use std::time::Duration;

static OGN_DDB_CACHE_DURATION: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Deserialize)]
struct DeviceDatabase {
    pub devices: Vec<Device>,
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
pub struct Device {
    // pub device_type: String,
    pub device_id: String,
    pub aircraft_model: String,
    pub registration: String,
    pub cn: String,
    // pub tracked: String,
    // pub identified: String,
    // pub aircraft_type: String,
}

impl Device {
    pub fn into_flarmnet_record(self) -> flarmnet::Record {
        flarmnet::Record {
            flarm_id: self.device_id,
            pilot_name: "".to_string(),
            airfield: "".to_string(),
            plane_type: self.aircraft_model,
            registration: self.registration,
            call_sign: self.cn,
            frequency: "".to_string(),
        }
    }
}

#[instrument]
pub fn get_ddb() -> anyhow::Result<Vec<Device>> {
    let cache = Cache::new("ogn-ddb.json", OGN_DDB_CACHE_DURATION);
    if cache.needs_update() {
        let content = download_ogn_ddb_data()?;
        cache.save(&content)?;
    }

    info!("reading OGN DDB…");
    let content = cache.read()?;
    let ogn_ddb: DeviceDatabase = serde_json::from_str(&content)?;
    Ok(ogn_ddb.devices)
}

#[instrument]
fn download_ogn_ddb_data() -> anyhow::Result<String> {
    info!("downloading OGN DDB…");
    let response = ureq::get("http://ddb.glidernet.org/download/?j=1&t=1").call()?;
    Ok(response.into_string()?)
}
