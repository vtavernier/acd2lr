use std::convert::TryFrom;
use thiserror::Error;

#[derive(Debug, Clone, Copy)]
pub struct XPacket<'p> {
    pub header: &'p [u8],
    pub body: &'p [u8],
    pub footer: &'p [u8],
}

#[derive(Debug)]
pub struct XPacketMut<'p> {
    pub header: &'p mut [u8],
    pub body: &'p mut [u8],
    pub footer: &'p mut [u8],
}

#[derive(Debug, Error)]
pub enum XPacketParseError {
    #[error("missing xpacket header")]
    MissingHeader,
    #[error("missing xpacket footer")]
    MissingFooter,
    #[error("missing xpacket header boundary")]
    MissingHeaderBoundary,
    #[error("missing xpacket footer boundary")]
    MissingFooterBoundary,
}

fn get_offsets(value: &[u8]) -> Result<(usize, usize), XPacketParseError> {
    // Check we actually have an xpacket header
    if !value.starts_with(b"<?xpacket begin=") {
        return Err(XPacketParseError::MissingHeader);
    }

    // Check we have a footer at the end
    if !value.ends_with(b"<?xpacket end=\"w\"?>") {
        return Err(XPacketParseError::MissingFooter);
    }

    // Now find the first newline: this is the header boundary
    let body_start = value
        .iter()
        .enumerate()
        .find(|(_, x)| **x == b'\n')
        .map(|(i, _)| i + 1)
        .ok_or_else(|| XPacketParseError::MissingHeaderBoundary)?;

    // The last newline is the footer boundary
    let body_end = value
        .iter()
        .rev()
        .enumerate()
        .find(|(_, x)| **x == b'\n')
        .map(|(i, _)| i + 1)
        .ok_or_else(|| XPacketParseError::MissingFooterBoundary)?;

    Ok((body_start, body_end))
}

impl<'p> TryFrom<&'p [u8]> for XPacket<'p> {
    type Error = XPacketParseError;

    fn try_from(value: &'p [u8]) -> Result<Self, Self::Error> {
        let (body_start, body_end) = get_offsets(value)?;
        let (header, body_footer) = value.split_at(body_start);
        let (body, footer) = body_footer.split_at(body_end - body_start);

        Ok(Self {
            header,
            body,
            footer,
        })
    }
}

impl<'p> TryFrom<&'p mut [u8]> for XPacketMut<'p> {
    type Error = XPacketParseError;

    fn try_from(value: &'p mut [u8]) -> Result<Self, Self::Error> {
        let (body_start, body_end) = get_offsets(value)?;
        let (header, body_footer) = value.split_at_mut(body_start);
        let (body, footer) = body_footer.split_at_mut(body_end - body_start);

        Ok(Self {
            header,
            body,
            footer,
        })
    }
}
