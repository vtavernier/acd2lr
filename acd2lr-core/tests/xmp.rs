use acd2lr_core::{xmp::XmpData, xpacket::XPacket};
use std::convert::TryFrom;
use test_env_log::test;

fn test_xpacket(val: &[u8]) -> XPacket {
    let parsed = XPacket::try_from(val).expect("failed to parse xpacket");

    assert!(parsed.header.starts_with(b"<?xpacket begin"));
    assert!(parsed.header.ends_with(b"?>"));

    eprintln!("{:?}", val);

    parsed
}

fn test_xmp(val: &[u8]) {
    let xpacket = test_xpacket(val);
    let parsed = XmpData::parse(xpacket.body).expect("failed to parse xmp");

    eprintln!("{:#?}", parsed);

    let acdsee = parsed.acdsee_data().expect("failed to parse acdsee data");

    eprintln!("{:#?}", acdsee);
}

#[test]
fn test_xpacket_acdsee() {
    test_xpacket(&include_bytes!("data/acdsee_data.xpacket")[..]);
}

#[test]
fn test_xpacket_lightroom() {
    test_xpacket(&include_bytes!("data/lightroom_data.xpacket")[..]);
}

#[test]
fn test_xmp_acdsee() {
    test_xmp(&include_bytes!("data/acdsee_data.xpacket")[..]);
}

#[test]
fn test_xmp_lightroom() {
    test_xmp(&include_bytes!("data/lightroom_data.xpacket")[..]);
}
