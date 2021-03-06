use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;

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

#[instrument(skip(client))]
pub async fn get_ddb(client: &ClientWithMiddleware) -> anyhow::Result<Vec<Device>> {
    info!("Downloading OGN DDB…");
    let response = client
        .get("http://ddb.glidernet.org/download/?j=1&t=1")
        .send()
        .await?;
    let ogn_ddb: DeviceDatabase = response.json().await?;
    Ok(ogn_ddb.devices)
}
