use crate::cache::Cache;
use serde::{Deserialize, Serialize};
use std::time::Duration;

static WEGLIDE_CACHE_DURATION: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    pub id: u32,
    pub name: String,
    pub home_airport: Option<Airport>,
    pub device: Option<Device>,
}

impl User {
    pub fn into_flarmnet_record(self) -> Option<flarmnet::Record> {
        let Self {
            device,
            name,
            home_airport,
            ..
        } = self;

        device.map(|device| flarmnet::Record {
            flarm_id: device.id,
            pilot_name: name,
            airfield: home_airport.map(|it| it.name).unwrap_or_default(),
            plane_type: device.aircraft.map(|it| it.name).unwrap_or_default(),
            registration: device.name.unwrap_or_default(),
            call_sign: device.competition_id.unwrap_or_default(),
            frequency: "".to_string(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Device {
    pub id: String,
    pub name: Option<String>,
    pub competition_id: Option<String>,
    pub aircraft: Option<Aircraft>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Aircraft {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Airport {
    pub id: u32,
    pub name: String,
}

#[instrument]
pub fn get_users() -> anyhow::Result<Vec<User>> {
    let cache = Cache::new("weglide-users.json", WEGLIDE_CACHE_DURATION);
    if cache.needs_update() {
        let all_users = download_all_users()?;
        debug!(all_users = all_users.len());

        let filtered_users: Vec<_> = all_users
            .into_iter()
            .filter(|it| it.device.is_some())
            .collect();
        debug!(filtered_users = filtered_users.len());

        let content = serde_json::to_string_pretty(&filtered_users)?;
        cache.save(&content)?;
    }

    info!("reading WeGlide user data…");
    let content = cache.read()?;
    let users: Vec<User> = serde_json::from_str(&content)?;
    Ok(users)
}

#[instrument]
fn download_all_users() -> anyhow::Result<Vec<User>> {
    info!("downloading WeGlide user data…");
    let mut start = 1u32;
    let limit = 100u32;
    let mut all = Vec::new();
    loop {
        let ids: Vec<u32> = (start..start + limit).collect();

        let page = download_users(&ids)?;
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
fn download_users(ids: &[u32]) -> anyhow::Result<Vec<User>> {
    let ids: Vec<_> = ids.iter().map(|id| id.to_string()).collect();
    let ids = ids.join(",");

    info!("downloading WeGlide user data page…");
    let url = format!("https://api.weglide.org/v1/user?id_in={}&limit=100", ids);
    let response = ureq::get(&url).call()?;
    Ok(response.into_json()?)
}
