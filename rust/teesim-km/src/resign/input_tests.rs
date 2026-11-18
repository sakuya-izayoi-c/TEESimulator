use super::parse_key_description;

// Complete, unsigned test records; all fixture lengths here fit the DER short form.
fn sequence(body: &[u8]) -> Vec<u8> {
    assert!(body.len() < 128);
    let mut out = vec![0x30, u8::try_from(body.len()).unwrap()];
    out.extend_from_slice(body);
    out
}

fn record(attestation: &[u8], keymaster: &[u8]) -> Vec<u8> {
    sequence(
        &[
            attestation,
            &[10, 1, 1],
            keymaster,
            &[10, 1, 1, 4, 1, 0xab, 4, 0, 48, 0, 48, 0],
        ]
        .concat(),
    )
}

const V3: &[u8] = &[2, 1, 3];

#[test]
fn accepts_canonical_versions_without_guessing() {
    for (field, expected) in [
        (&[2, 1, 0][..], 0),
        (V3, 3),
        (&[2, 1, 4], 4),
        (&[2, 1, 127], 127),
        (&[2, 2, 0, 128], 128),
        (&[2, 2, 1, 44], 300),
        (&[2, 5, 0, 255, 255, 255, 255], u32::MAX),
    ] {
        assert_eq!(
            parse_key_description(&record(V3, field))
                .ok()
                .map(|d| d.keymaster_version),
            Some(expected)
        );
    }
}

#[test]
fn rejects_malformed_versions_in_either_position() {
    for field in [
        &[2, 0][..],
        &[2, 1, 0xff],
        &[2, 5, 1, 0, 0, 0, 0],
        &[2, 2, 0, 4],
        &[2, 6, 0, 128, 0, 0, 0, 0],
        &[4, 1, 4],
        &[2, 0x81, 1, 4],
    ] {
        for der in [record(V3, field), record(field, V3)] {
            assert!(
                parse_key_description(&der).is_err(),
                "accepted invalid version {field:x?}"
            );
        }
    }
}

#[test]
fn exposes_original_bytes_for_valid_long_form_record() {
    // A binary 128-byte challenge forces both inner and outer long-form lengths.
    let mut prefix = vec![2, 1, 3, 10, 1, 1, 2, 1, 4, 10, 1, 1, 4, 0x81, 0x80];
    prefix.extend(0x80..=0xff);
    prefix.extend([4, 1, 0]);
    let sw = [0xa1, 3, 2, 1, 1];
    let hw = [0xbf, 0x81, 0x00, 2, 5, 0]; // high-number tag; opaque to the envelope reader
    let body = [&prefix[..], &sequence(&sw), &sequence(&hw)].concat();
    let mut der = vec![0x30, 0x81, u8::try_from(body.len()).unwrap()];
    der.extend(body);
    let description = parse_key_description(&der).unwrap();
    assert_eq!(description.keymaster_version, 4);
    assert_eq!(description.prefix, prefix);
    assert_eq!(description.software, sw);
    assert_eq!(description.hardware, hw);
}

#[test]
fn rejects_invalid_types_and_enumerated_encodings() {
    for level in [&[4, 1, 1][..], &[10, 0], &[10, 1, 0xff], &[10, 2, 0, 1]] {
        let good_level = &[10, 1, 1][..];
        for (a, b) in [(level, good_level), (good_level, level)] {
            let der = sequence(&[V3, a, V3, b, &[4, 0, 4, 0, 48, 0, 48, 0]].concat());
            assert!(parse_key_description(&der).is_err());
        }
    }
    let mut wrong_challenge = record(V3, V3);
    wrong_challenge[14] = 2; // INTEGER instead of OCTET STRING
    assert!(parse_key_description(&wrong_challenge).is_err());
}

#[test]
fn rejects_incomplete_or_non_der_envelopes() {
    let good = record(V3, V3);
    for cut in 0..good.len() {
        assert!(
            parse_key_description(&good[..cut]).is_err(),
            "accepted prefix of length {cut}"
        );
    }
    for bad in [
        [&good[..], &[5, 0]].concat(),                           // trailing TLV
        [&[0x31], &good[1..]].concat(),                          // wrong outer tag
        [&[0x30, 0x81], &good[1..]].concat(),                    // non-minimal length
        [&[0x30, 0x80], &good[2..], &[0, 0]].concat(),           // indefinite length
        sequence(&good[2..good.len() - 2]),                      // missing list
        sequence(&[&good[2..], &[5, 0]].concat()),               // ninth field
        sequence(&[&good[2..good.len() - 2], &[4, 0]].concat()), // list with wrong tag
    ] {
        assert!(parse_key_description(&bad).is_err());
    }
}
