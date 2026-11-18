//! Borrow the original field encodings; decoding a version must not re-encode its record.

use x509_cert::der::{AnyRef, Decode, Reader, Result, SliceReader, Tag, Tagged};

pub(super) struct KeyDescription<'a> {
    pub(super) keymaster_version: u32,
    /// Complete TLVs of the first six fields, copied verbatim by the caller.
    pub(super) prefix: &'a [u8],
    /// Contents of the two AuthorizationList SEQUENCEs (including high-number tags).
    pub(super) software: &'a [u8],
    pub(super) hardware: &'a [u8],
}

fn value<'a>(reader: &mut SliceReader<'a>, tag: Tag) -> Result<&'a [u8]> {
    let field = AnyRef::decode(reader)?;
    field.tag().assert_eq(tag)?;
    Ok(field.value())
}

fn security_level(reader: &mut SliceReader<'_>) -> Result<()> {
    // ENUMERATED has a minimal, nonnegative integer encoding. Don't select on its value.
    match value(reader, Tag::Enumerated)? {
        [] | [0x80..=0xff, ..] | [0, 0..=0x7f, ..] => Err(Tag::Enumerated.value_error()),
        _ => Ok(()),
    }
}

pub(super) fn parse_key_description(bytes: &[u8]) -> Result<KeyDescription<'_>> {
    let top = AnyRef::from_der(bytes)?; // includes the end-of-input check
    top.sequence(|r| {
        u32::decode(r)?; // attestationVersion
        security_level(r)?;
        let keymaster_version = u32::decode(r)?;
        security_level(r)?;
        value(r, Tag::OctetString)?; // attestationChallenge
        value(r, Tag::OctetString)?; // uniqueId
        let prefix = &top.value()[..usize::try_from(r.position())?];
        let software = value(r, Tag::Sequence)?;
        let hardware = value(r, Tag::Sequence)?;
        Ok(KeyDescription {
            keymaster_version,
            prefix,
            software,
            hardware,
        })
    }) // sequence also requires that no ninth field remains
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
