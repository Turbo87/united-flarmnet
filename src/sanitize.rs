use flarmnet::Record;

pub fn has_data(record: &Record) -> bool {
    !(record.pilot_name.is_empty()
        && record.airfield.is_empty()
        && record.plane_type.is_empty()
        && record.registration.is_empty()
        && record.call_sign.is_empty()
        && record.frequency.is_empty())
}

fn sanitize_for_lx(value: &str) -> String {
    sanitize(value)
}

fn sanitize_for_xcsoar(value: &str) -> String {
    if encoding_rs::mem::is_str_latin1(value) {
        value.to_string()
    } else {
        sanitize(value)
    }
}

fn sanitize(value: &str) -> String {
    let string = value
        .replace('ä', "ae")
        .replace('ö', "oe")
        .replace('ü', "ue")
        .replace('Ä', "Ae")
        .replace('Ö', "Oe")
        .replace('Ü', "Ue")
        .replace('ß', "ss");
    deunicode::deunicode(&string)
}

pub fn sanitize_record_for_lx(record: &Record) -> Option<Record> {
    if record.flarm_id.is_empty() || !has_data(record) {
        return None;
    }

    Some(Record {
        flarm_id: sanitize_for_lx(&record.flarm_id),
        pilot_name: sanitize_for_lx(&record.pilot_name),
        airfield: sanitize_for_lx(&record.airfield),
        plane_type: sanitize_for_lx(&record.plane_type),
        registration: sanitize_for_lx(&record.registration),
        call_sign: sanitize_for_lx(&record.call_sign),
        frequency: sanitize_for_lx(&record.frequency),
    })
}

pub fn sanitize_record_for_xcsoar(record: &Record) -> Option<Record> {
    if record.flarm_id.is_empty() || !has_data(record) {
        return None;
    }

    Some(Record {
        flarm_id: sanitize_for_xcsoar(&record.flarm_id),
        pilot_name: sanitize_for_xcsoar(&record.pilot_name),
        airfield: sanitize_for_xcsoar(&record.airfield),
        plane_type: sanitize_for_xcsoar(&record.plane_type),
        registration: sanitize_for_xcsoar(&record.registration),
        call_sign: sanitize_for_xcsoar(&record.call_sign),
        frequency: sanitize_for_xcsoar(&record.frequency),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_for_lx() {
        assert_eq!(sanitize_for_lx("foo"), "foo");
        assert_eq!(sanitize_for_lx("Müller"), "Mueller");
        assert_eq!(sanitize_for_lx("Pociūnai"), "Pociunai");
        assert_eq!(sanitize_for_lx("😂"), "joy");
    }

    #[test]
    fn test_sanitize_for_xcsoar() {
        assert_eq!(sanitize_for_xcsoar("foo"), "foo");
        assert_eq!(sanitize_for_xcsoar("Müller"), "Müller");
        assert_eq!(sanitize_for_xcsoar("Pociūnai"), "Pociunai");
        assert_eq!(sanitize_for_xcsoar("😂"), "joy");
    }
}
