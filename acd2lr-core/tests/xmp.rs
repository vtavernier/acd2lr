use acd2lr_core::xmp::XPacket;
use std::convert::TryFrom;

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
