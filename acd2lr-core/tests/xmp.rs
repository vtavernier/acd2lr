use acd2lr_core::{xmp::XmpData, xpacket::XPacket};
use std::convert::TryFrom;
use test_env_log::test;

#[test]
fn test_xpacket() {
    let val = &include_bytes!("data/acdsee_data.xpacket")[..];
    let parsed = XPacket::try_from(val).expect("failed to parse xpacket");

    assert_eq!(
        b"<?xpacket begin=\"\xFE\xFF\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>",
        parsed.header
    );

    eprintln!("{:?}", val);
}

#[test]
fn test_xmp() {
    let val = &include_bytes!("data/acdsee_data.xpacket")[..];
    let xpacket = XPacket::try_from(val).expect("failed to parse xpacket");
    let parsed = XmpData::parse(xpacket.body).expect("failed to parse xmp");

    eprintln!("{:#?}", parsed);

    let acdsee = parsed.acdsee_data().expect("failed to parse acdsee data");

    eprintln!("{:#?}", acdsee);
}
