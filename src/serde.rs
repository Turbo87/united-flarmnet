use crate::sanitize::has_data;
use crate::weglide::UserRef;
use flarmnet::Record;
use serde::Serialize;

fn is_empty_or_unknown(s: &str) -> bool {
    s.is_empty() || s == "Unknown"
}

#[derive(Serialize)]
pub struct SerializableRecord<'a> {
    flarm_id: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    pilot_name: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    airfield: &'a str,
    #[serde(skip_serializing_if = "is_empty_or_unknown")]
    plane_type: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    registration: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    call_sign: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    frequency: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    weglide_user_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    club_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    weglide_club_id: Option<u32>,
}

impl<'a> SerializableRecord<'a> {
    pub fn from_record(record: &'a Record, user: Option<&'a UserRef>) -> Option<Self> {
        if record.flarm_id.is_empty() || !has_data(record) {
            return None;
        }

        let image = user
            .and_then(|user| user.image.as_deref())
            .filter(|image| !image.is_empty());
        let club = user.and_then(|user| user.club.as_ref());

        Some(Self {
            flarm_id: &record.flarm_id,
            pilot_name: &record.pilot_name,
            airfield: &record.airfield,
            plane_type: &record.plane_type,
            registration: &record.registration,
            call_sign: &record.call_sign,
            frequency: &record.frequency,
            image_url: image.map(|image| format!("https://files.weglide.org/{image}")),
            weglide_user_id: user.map(|user| user.id),
            club_name: club
                .map(|club| club.name.as_str())
                .filter(|name| !name.is_empty()),
            weglide_club_id: club.map(|club| club.id),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_record() {
        let record = Record {
            flarm_id: "ABCDEF".to_string(),
            pilot_name: "John Dö".to_string(),
            airfield: "EDKA".to_string(),
            plane_type: "ASW 28".to_string(),
            registration: "D-1234".to_string(),
            call_sign: "XY".to_string(),
            frequency: "123.456".to_string(),
        };

        insta::assert_json_snapshot!(SerializableRecord::from_record(&record, None), @r#"
        {
          "flarm_id": "ABCDEF",
          "pilot_name": "John Dö",
          "airfield": "EDKA",
          "plane_type": "ASW 28",
          "registration": "D-1234",
          "call_sign": "XY",
          "frequency": "123.456"
        }
        "#);
    }

    #[test]
    fn test_empty_strings_are_skipped() {
        let record = Record {
            flarm_id: "ABCDEF".to_string(),
            pilot_name: "".to_string(),
            airfield: "".to_string(),
            plane_type: "Unknown".to_string(),
            registration: "D-1234".to_string(),
            call_sign: "".to_string(),
            frequency: "".to_string(),
        };

        insta::assert_json_snapshot!(SerializableRecord::from_record(&record, None), @r#"
        {
          "flarm_id": "ABCDEF",
          "registration": "D-1234"
        }
        "#);
    }

    #[test]
    fn test_empty_flarm_id_returns_none() {
        let record = Record {
            flarm_id: "".to_string(),
            pilot_name: "John Dö".to_string(),
            airfield: "EDKA".to_string(),
            plane_type: "ASW 28".to_string(),
            registration: "D-1234".to_string(),
            call_sign: "XY".to_string(),
            frequency: "123.456".to_string(),
        };

        assert!(SerializableRecord::from_record(&record, None).is_none());
    }

    #[test]
    fn test_all_other_fields_empty_returns_none() {
        let record = Record {
            flarm_id: "ABCDEF".to_string(),
            pilot_name: "".to_string(),
            airfield: "".to_string(),
            plane_type: "".to_string(),
            registration: "".to_string(),
            call_sign: "".to_string(),
            frequency: "".to_string(),
        };

        assert!(SerializableRecord::from_record(&record, None).is_none());
    }
}
