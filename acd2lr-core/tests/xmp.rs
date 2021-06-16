use std::{convert::TryFrom, io::prelude::*, path::Path};

use acd2lr_core::{
    file::XPacketFile,
    xmp::{rules, XmpData},
    xpacket::XPacket,
};
use async_std::{fs::File, task::block_on};
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

async fn test_rewrite(p: impl AsRef<Path>) {
    let packet = XPacketFile::open(File::open(p.as_ref()).await.unwrap())
        .await
        .unwrap()
        .read_packet_bytes()
        .await
        .unwrap()
        .unwrap();
    let packet = XPacket::try_from(&packet[..]).unwrap();

    eprint!("before: ");
    std::io::stderr().write_all(&packet.body).unwrap();
    eprintln!();

    let xmp = XmpData::parse(packet.body).unwrap();

    let mut rules = vec![rules::xmp_metadata_date()];
    rules.extend(xmp.acdsee_data().unwrap().to_ruleset());
    let events = xmp.write_events(rules);

    eprintln!("after: ");

    let events = events.unwrap();

    let mut out = Vec::with_capacity(packet.body.len());
    let mut writer = xml::writer::EventWriter::new_with_config(
        &mut out,
        xml::writer::EmitterConfig::new()
            .perform_indent(true)
            .indent_string(" ")
            .write_document_declaration(false),
    );

    for event in events {
        if let Some(evt) = event.as_writer_event() {
            writer.write(evt).unwrap();
        }
    }

    std::io::stderr().write_all(&out[..]).unwrap();
    eprintln!();

    let trimmed_body = unsafe { String::from_utf8_unchecked(packet.body.to_vec()) };
    let trimmed_body = trimmed_body.trim_end();
    let padding = packet.body.len() - trimmed_body.len();

    if out.len() > trimmed_body.len() {
        let diff = out.len() - trimmed_body.len();
        eprintln!("space lost: {} bytes", diff);

        if diff < padding {
            eprintln!("fits in padding, leftover: {} bytes", padding - diff);
        } else {
            eprintln!("does not fit in padding, extra: {} bytes", diff - padding);
        }

        assert!(diff < padding);
    } else {
        let diff = trimmed_body.len() - out.len();
        eprintln!("space saved: {} bytes", diff);
        eprintln!("padding left: {} bytes", padding + diff);
    }
}

#[test]
fn test_rewrite_single() {
    block_on(async {
        test_rewrite("tests/data/test_cat.jpg").await;
    });
}

#[test]
fn test_rewrite_multi() {
    block_on(async {
        test_rewrite("tests/data/test_cat_multi.jpg").await;
    });
}
