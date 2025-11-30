use flarmnet::Record;
use serde::Serialize;

#[derive(Serialize)]
pub struct SerializableRecord<'a> {
    flarm_id: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    pilot_name: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    airfield: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    plane_type: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    registration: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    call_sign: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    frequency: &'a str,
}

impl<'a> SerializableRecord<'a> {
    pub fn from_record(record: &'a Record) -> Option<Self> {
        if record.flarm_id.is_empty() {
            return None;
        }

        let has_no_data = record.pilot_name.is_empty()
            && record.airfield.is_empty()
            && record.plane_type.is_empty()
            && record.registration.is_empty()
            && record.call_sign.is_empty()
            && record.frequency.is_empty();

        if has_no_data {
            return None;
        }

        Some(Self {
            flarm_id: &record.flarm_id,
            pilot_name: &record.pilot_name,
            airfield: &record.airfield,
            plane_type: &record.plane_type,
            registration: &record.registration,
            call_sign: &record.call_sign,
            frequency: &record.frequency,
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

        insta::assert_json_snapshot!(SerializableRecord::from_record(&record), @r#"
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
            plane_type: "ASW 28".to_string(),
            registration: "D-1234".to_string(),
            call_sign: "".to_string(),
            frequency: "".to_string(),
        };

        insta::assert_json_snapshot!(SerializableRecord::from_record(&record), @r#"
        {
          "flarm_id": "ABCDEF",
          "plane_type": "ASW 28",
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

        assert!(SerializableRecord::from_record(&record).is_none());
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

        assert!(SerializableRecord::from_record(&record).is_none());
    }
}
