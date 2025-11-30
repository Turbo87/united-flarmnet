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

impl<'a> From<&'a Record> for SerializableRecord<'a> {
    fn from(record: &'a Record) -> Self {
        Self {
            flarm_id: &record.flarm_id,
            pilot_name: &record.pilot_name,
            airfield: &record.airfield,
            plane_type: &record.plane_type,
            registration: &record.registration,
            call_sign: &record.call_sign,
            frequency: &record.frequency,
        }
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

        insta::assert_json_snapshot!(SerializableRecord::from(&record), @r#"
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

        insta::assert_json_snapshot!(SerializableRecord::from(&record), @r#"
        {
          "flarm_id": "ABCDEF",
          "plane_type": "ASW 28",
          "registration": "D-1234"
        }
        "#);
    }
}
