use std::convert::TryFrom;
use std::fs::File;
use std::path::Path;

use acd2lr_core::{file::XPacketFile, xmp::XmpData, xpacket::XPacket};
use test_env_log::test;

fn test_file(path: impl AsRef<Path>) {
    let result = XPacketFile::open(File::open(path.as_ref()).unwrap());
    eprintln!("{:?}", result);
    assert!(result.is_ok());

    let mut result = result.unwrap();
    let packet = result
        .read_packet_bytes()
        .expect("failed to read packet bytes");
    assert!(packet.is_some());

    let packet = packet.unwrap();
    let xpacket = XPacket::try_from(&packet[..]).expect("failed to parse xpacket");
    let xmp = XmpData::parse(xpacket.body).expect("failed to parse xmp");

    eprintln!("{:#?}", xmp);
}

#[test]
fn test_single_description() {
    test_file("tests/data/test_cat.jpg");
}

#[test]
fn test_multi_description() {
    test_file("tests/data/test_cat_multi.jpg");
}
