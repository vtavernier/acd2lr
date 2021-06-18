use std::{convert::TryFrom, io::SeekFrom};

use async_std::{fs::File, io::prelude::*};
use thiserror::Error;
use xml::reader::XmlEvent;

use crate::{
    file::WritePacketError,
    xpacket::{XPacket, XPacketMut},
};

trait WriterExt {
    fn write_all(&mut self, events: &[XmlEvent]) -> Result<(), xml::writer::Error>;
}

impl<W: std::io::Write> WriterExt for xml::writer::EventWriter<W> {
    fn write_all(&mut self, events: &[XmlEvent]) -> Result<(), xml::writer::Error> {
        for event in events {
            if let Some(evt) = event.as_writer_event() {
                self.write(evt)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ContainerError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    XPacketParse(#[from] crate::xpacket::XPacketParseError),
    #[error(transparent)]
    XmpParse(#[from] crate::xmp::XmpParseError),
}

#[derive(Debug, Error)]
pub enum ContainerRewriteError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Emitter(xml::writer::Error),
    #[error("missing xpacket")]
    MissingXPacket,
    #[error(transparent)]
    XPacketParse(#[from] crate::xpacket::XPacketParseError),
    #[error("not enough space for the new xpacket")]
    NotEnoughSpace,
}

impl From<xml::writer::Error> for ContainerRewriteError {
    fn from(error: xml::writer::Error) -> Self {
        match error {
            xml::writer::Error::Io(io) => Self::Io(io),
            other => Self::Emitter(other),
        }
    }
}

#[derive(Debug, Error)]
pub enum ContainerWriteError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("missing xpacket")]
    MissingXPacket,
    #[error("not enough space for the new xpacket")]
    NotEnoughSpace,
}

impl From<WritePacketError> for ContainerWriteError {
    fn from(error: WritePacketError) -> Self {
        match error {
            WritePacketError::Io(io) => Self::Io(io),
            WritePacketError::NoPacket => Self::MissingXPacket,
            WritePacketError::WrongPacketSize => Self::NotEnoughSpace,
        }
    }
}

pub struct Container {
    data: ContainerData,
}

enum ContainerData {
    Xmp(XmpData),
    XPacket(XPacketData),
}

struct XmpData {
    fh: File,
}

impl XmpData {
    pub async fn read_xmp(&mut self) -> Result<Option<crate::xmp::XmpData>, ContainerError> {
        self.fh.seek(SeekFrom::Start(0)).await?;

        let mut bytes = Vec::new();
        self.fh.read_to_end(&mut bytes).await?;
        let xmp = crate::xmp::XmpData::parse(&bytes)?;

        Ok(Some(xmp))
    }

    pub async fn prepare_write(
        &mut self,
        events: &[XmlEvent],
    ) -> Result<Vec<u8>, ContainerRewriteError> {
        // xmp file, we don't really need to do anything special size-wise to fit the data in the file
        let mut out = Vec::with_capacity(8192);

        {
            let mut writer = xml::writer::EventWriter::new_with_config(
                &mut out,
                xml::writer::EmitterConfig::new()
                    .perform_indent(true)
                    .indent_string(" ")
                    .write_document_declaration(false),
            );

            // Write events
            writer.write_all(events)?;
        }

        Ok(out)
    }

    pub async fn write(&mut self, packet: &[u8]) -> Result<(), ContainerWriteError> {
        // Truncate the file
        self.fh.set_len(0).await?;

        // Write the new contents
        self.fh.write_all(packet).await?;

        Ok(())
    }
}

struct XPacketData {
    inner: crate::file::XPacketFile,
}

impl XPacketData {
    pub async fn read_xmp(&mut self) -> Result<Option<crate::xmp::XmpData>, ContainerError> {
        if let Some(packet_bytes) = self.inner.read_packet_bytes().await? {
            let xpacket = XPacket::try_from(&packet_bytes[..])?;
            let xmp = crate::xmp::XmpData::parse(&xpacket.body)?;
            Ok(Some(xmp))
        } else {
            Ok(None)
        }
    }

    fn events_to_vec(
        out: &mut Vec<u8>,
        events: &[XmlEvent],
        config: xml::writer::EmitterConfig,
    ) -> Result<(), ContainerRewriteError> {
        // Start with an empty buffer
        out.clear();

        let mut writer = xml::writer::EventWriter::new_with_config(out, config);
        writer.write_all(events)?;
        Ok(())
    }

    pub async fn prepare_write(
        &mut self,
        events: &[XmlEvent],
    ) -> Result<Vec<u8>, ContainerRewriteError> {
        // xpacket container, we need to fit the result inside the existing packet

        // TODO: Don't reparse the xpacket for this and forward it from previous state?
        let mut xpacket_bytes = self
            .inner
            .read_packet_bytes()
            .await?
            .ok_or_else(|| ContainerRewriteError::MissingXPacket)?;
        let xpacket = XPacketMut::try_from(&mut xpacket_bytes[..])?;

        // Buffer for finding optimal settings
        let mut out = Vec::with_capacity(xpacket.body.len() * 2);

        let emitter_configs = [
            xml::writer::EmitterConfig::new()
                .perform_indent(true)
                .indent_string(" ")
                .write_document_declaration(false),
            xml::writer::EmitterConfig::new()
                .perform_indent(false)
                .write_document_declaration(false),
        ];

        for config in &emitter_configs {
            // If we fail here, it's a XmlWriter error, so we always propagate
            Self::events_to_vec(&mut out, events, config.clone())?;

            if out.len() <= xpacket.body.len() - 2 {
                // There is enough space in the existing packet for this config

                // Overwrite with padding and newlines
                xpacket.body.fill(b' ');
                xpacket.body[0] = b'\n';
                *(xpacket.body.last_mut().unwrap()) = b'\n';

                // Overwrite inner contents
                xpacket.body[1..(1 + out.len())].copy_from_slice(&out);

                // Return the full packet
                return Ok(xpacket_bytes);
            }
        }

        Err(ContainerRewriteError::NotEnoughSpace)
    }

    pub async fn write(&mut self, packet: &[u8]) -> Result<(), ContainerWriteError> {
        self.inner.write_packet_bytes(packet).await?;
        Ok(())
    }
}

impl Container {
    pub async fn open(mut file: async_std::fs::File) -> Result<Self, (std::io::Error, File)> {
        // Read the header
        let mut start_buf: [u8; 16] = [0; 16];
        match file.read_exact(&mut start_buf).await {
            Ok(_) => {
                if start_buf.starts_with(b"<x:xmp") {
                    // A .xmp file
                    Ok(Self {
                        data: ContainerData::Xmp(XmpData { fh: file }),
                    })
                } else {
                    // A file maybe containing an XPacket
                    Ok(Self {
                        data: ContainerData::XPacket(XPacketData {
                            inner: crate::file::XPacketFile::open(file).await?,
                        }),
                    })
                }
            }
            Err(e) => {
                return Err((e, file));
            }
        }
    }

    pub async fn read_xmp(&mut self) -> Result<Option<crate::xmp::XmpData>, ContainerError> {
        match &mut self.data {
            ContainerData::Xmp(inner) => inner.read_xmp().await,
            ContainerData::XPacket(inner) => inner.read_xmp().await,
        }
    }

    pub async fn prepare_write(
        &mut self,
        events: &[XmlEvent],
    ) -> Result<Vec<u8>, ContainerRewriteError> {
        match &mut self.data {
            ContainerData::Xmp(inner) => inner.prepare_write(events).await,
            ContainerData::XPacket(inner) => inner.prepare_write(events).await,
        }
    }

    pub async fn write(&mut self, packet: &[u8]) -> Result<(), ContainerWriteError> {
        match &mut self.data {
            ContainerData::Xmp(inner) => inner.write(packet).await,
            ContainerData::XPacket(inner) => inner.write(packet).await,
        }
    }

    pub fn into_inner(self) -> File {
        match self.data {
            ContainerData::Xmp(inner) => inner.fh,
            ContainerData::XPacket(inner) => inner.inner.into_inner().0,
        }
    }
}
