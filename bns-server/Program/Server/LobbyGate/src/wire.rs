//! Bounded client transport. Spec 03 §§1, 2, 4.
use bloodlib::{frames::payload_len, packet::MAX_PACKET};
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
pub const MAX_WIRE: usize = 4008;
pub fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Read exactly one frame. A clean EOF is distinct from a truncated frame.
/// The header is checked before the payload allocation.
pub async fn read_frame(reader: &mut (impl AsyncRead + Unpin)) -> io::Result<Option<Vec<u8>>> {
    let mut header = [0; 2];
    if reader.read(&mut header[..1]).await? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut header[1..]).await?;
    let h = u16::from_le_bytes(header);
    let len = payload_len(h);
    if h >> 14 == 0 {
        return Err(invalid("plain client frame is not accepted"));
    }
    if len + 2 > MAX_WIRE {
        return Err(invalid("frame exceeds receive buffer"));
    }
    let mut frame = vec![0; len + 2];
    frame[..2].copy_from_slice(&header);
    reader.read_exact(&mut frame[2..]).await?;
    Ok(Some(frame))
}
fn check_output(frame: &[u8]) -> io::Result<()> {
    if frame.len() < 2 || frame.len() > MAX_WIRE {
        return Err(invalid("invalid output frame size"));
    }
    let header = u16::from_le_bytes([frame[0], frame[1]]);
    if header >> 14 == 0 || payload_len(header) + 2 != frame.len() {
        return Err(invalid("invalid output frame header"));
    }
    Ok(())
}
pub async fn write_frame(writer: &mut (impl AsyncWrite + Unpin), frame: &[u8]) -> io::Result<()> {
    check_output(frame)?;
    writer.write_all(frame).await?;
    writer.flush().await
}
/// Write frames in the given order with one write call (`scale-rules.md` rule H4).
/// A bad frame stops the call before any byte goes out.
pub async fn write_frames(
    writer: &mut (impl AsyncWrite + Unpin),
    frames: &[Vec<u8>],
) -> io::Result<()> {
    match frames {
        [] => return Ok(()),
        [frame] => return write_frame(writer, frame).await,
        _ => {}
    }
    for frame in frames {
        check_output(frame)?;
    }
    writer.write_all(&frames.concat()).await?;
    writer.flush().await
}
/// Remove cipher padding only by the inner signed size field.
pub fn inner_packet(plain: &[u8]) -> io::Result<&[u8]> {
    if plain.len() < 2 {
        return Err(invalid("missing inner size"));
    }
    let size = i16::from_le_bytes([plain[0], plain[1]]);
    if size < 4 || size as usize > plain.len() || size as usize > MAX_PACKET {
        return Err(invalid("invalid inner packet size"));
    }
    Ok(&plain[..size as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(words: u16, fill: u8) -> Vec<u8> {
        let mut bytes = vec![fill; 2 + usize::from(words) * 4];
        bytes[..2].copy_from_slice(&(0x5000 | words).to_le_bytes());
        bytes
    }

    #[tokio::test]
    async fn write_frames_keeps_bytes_and_order() {
        let frames = vec![frame(1, 1), frame(3, 2), frame(2, 3)];
        let mut wire = Vec::new();
        write_frames(&mut wire, &frames).await.unwrap();
        assert_eq!(wire, frames.concat());
        let mut reader = wire.as_slice();
        for expected in &frames {
            assert_eq!(&read_frame(&mut reader).await.unwrap().unwrap(), expected);
        }
    }

    #[tokio::test]
    async fn write_frames_sends_nothing_when_one_frame_is_bad() {
        let mut wire = Vec::new();
        let frames = vec![frame(1, 1), vec![0x05, 0x00, 1, 2, 3, 4, 5]];
        assert!(write_frames(&mut wire, &frames).await.is_err());
        assert!(wire.is_empty());
    }
}
