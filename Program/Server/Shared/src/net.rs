//! Shared socket layer for the daemons (`Docs/Architecture/scale-rules.md`, rules H2 to H7).
//!
//! - [`FrameReader`]: one buffer for each connection, length-checked frames, safe to cancel.
//! - [`spawn_writer`]: one writer task for each connection. It drains its queue and writes one time.
//! - [`Outbox`]: bounded queue to the writer task.
//!
//! Frame form ("trusted" frame of the daemon links): a little-endian `u16` at offset 0 gives the
//! full frame size, the 2 size bytes included. The layer does not change bytes or frame order.

use bytes::{Bytes, BytesMut};
use std::io;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc::{self, error::TrySendError},
    task::JoinHandle,
};

/// Smallest frame: size field and a 2-byte command.
pub const MIN_FRAME: usize = 4;
/// Largest frame that the original trusted links accept.
pub const MAX_TRUSTED_FRAME: usize = 4000;
/// Default queue length of a daemon link (frames).
pub const LINK_QUEUE: usize = 4096;
/// Default queue length of a game client connection (frames).
pub const CLIENT_QUEUE: usize = 1024;
/// The writer stops a batch at this size and writes.
const BATCH_BYTES: usize = 64 * 1024;

/// Socket options for each connection of a daemon (rule H5).
pub fn tune(stream: &TcpStream) -> io::Result<()> {
    stream.set_nodelay(true)
}

/// Reads size-prefixed frames from one connection.
pub struct FrameReader<R> {
    reader: R,
    buffer: BytesMut,
    max_frame: usize,
}

impl<R: AsyncRead + Unpin> FrameReader<R> {
    /// `max_frame`: largest full frame size that the peer can send (at most 65,535).
    pub fn new(reader: R, max_frame: usize) -> Self {
        Self {
            reader,
            buffer: BytesMut::with_capacity(8 * 1024),
            max_frame: max_frame.min(usize::from(u16::MAX)),
        }
    }

    /// The next full frame, size field included. `None`: the peer closed between frames.
    ///
    /// Safe to cancel: bytes that were read stay in the buffer.
    pub async fn next(&mut self) -> io::Result<Option<Bytes>> {
        loop {
            if let Some(frame) = self.take()? {
                return Ok(Some(frame));
            }
            if self.reader.read_buf(&mut self.buffer).await? == 0 {
                return if self.buffer.is_empty() {
                    Ok(None)
                } else {
                    Err(io::ErrorKind::UnexpectedEof.into())
                };
            }
        }
    }

    fn take(&mut self) -> io::Result<Option<Bytes>> {
        let Some(&[low, high]) = self.buffer.first_chunk::<2>() else {
            return Ok(None);
        };
        let size = usize::from(u16::from_le_bytes([low, high]));
        if !(MIN_FRAME..=self.max_frame).contains(&size) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("frame size {size}"),
            ));
        }
        if self.buffer.len() < size {
            self.buffer.reserve(size - self.buffer.len());
            return Ok(None);
        }
        Ok(Some(self.buffer.split_to(size).freeze()))
    }

    /// The reader half and the bytes that were read but not used.
    pub fn into_parts(self) -> (R, BytesMut) {
        (self.reader, self.buffer)
    }
}

/// Why a frame did not go into an [`Outbox`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendError {
    /// The queue is full. Policy of the caller: disconnect a client, fail the request on a link.
    Full,
    /// The writer task stopped (the connection is closed).
    Closed,
}

/// Bounded queue to the writer task of one connection. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Outbox(mpsc::Sender<Bytes>);

impl Outbox {
    /// Put a frame in the queue. Does not wait.
    pub fn try_send(&self, frame: impl Into<Bytes>) -> Result<(), SendError> {
        self.0.try_send(frame.into()).map_err(|error| match error {
            TrySendError::Full(_) => SendError::Full,
            TrySendError::Closed(_) => SendError::Closed,
        })
    }

    /// Put a frame in the queue. Waits while the queue is full (backpressure on a daemon link).
    pub async fn send(&self, frame: impl Into<Bytes>) -> Result<(), SendError> {
        self.0
            .send(frame.into())
            .await
            .map_err(|_| SendError::Closed)
    }

    /// True if the two values are clones of one queue.
    pub fn same_channel(&self, other: &Self) -> bool {
        self.0.same_channel(&other.0)
    }

    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }
}

/// Start the writer task of one connection.
///
/// The task takes all frames that wait in the queue, in order, and writes them with one call.
/// It ends when all [`Outbox`] clones are dropped (after it wrote the last frame) or on a write
/// error. It shuts the write side down at its end.
pub fn spawn_writer<W>(mut writer: W, queue: usize) -> (Outbox, JoinHandle<io::Result<()>>)
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (sender, mut receiver) = mpsc::channel::<Bytes>(queue.max(1));
    let task = tokio::spawn(async move {
        let mut batch = BytesMut::with_capacity(8 * 1024);
        while let Some(first) = receiver.recv().await {
            if receiver.is_empty() {
                writer.write_all(&first).await?;
            } else {
                batch.extend_from_slice(&first);
                while batch.len() < BATCH_BYTES {
                    let Ok(frame) = receiver.try_recv() else {
                        break;
                    };
                    batch.extend_from_slice(&frame);
                }
                writer.write_all(&batch).await?;
                batch.clear();
            }
            writer.flush().await?;
        }
        writer.shutdown().await
    });
    (Outbox(sender), task)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    fn frame(size: u16, fill: u8) -> Vec<u8> {
        let mut bytes = vec![fill; usize::from(size)];
        bytes[..2].copy_from_slice(&size.to_le_bytes());
        bytes
    }

    #[tokio::test]
    async fn reader_splits_frames_and_keeps_order() {
        let (mut a, b) = duplex(64);
        let mut reader = FrameReader::new(b, MAX_TRUSTED_FRAME);
        let mut wire = frame(4, 1);
        wire.extend(frame(300, 2));
        wire.extend(frame(6, 3));
        tokio::spawn(async move {
            // Small pieces: a frame comes in more than one read.
            for piece in wire.chunks(7) {
                a.write_all(piece).await.unwrap();
            }
        });
        assert_eq!(reader.next().await.unwrap().unwrap(), frame(4, 1));
        assert_eq!(reader.next().await.unwrap().unwrap(), frame(300, 2));
        assert_eq!(reader.next().await.unwrap().unwrap(), frame(6, 3));
        assert!(reader.next().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn reader_refuses_bad_sizes_and_cut_frames() {
        for size in [0u16, 3, 4001] {
            let (mut a, b) = duplex(64);
            a.write_all(&size.to_le_bytes()).await.unwrap();
            let mut reader = FrameReader::new(b, MAX_TRUSTED_FRAME);
            let error = reader.next().await.unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "size {size}");
        }
        let (mut a, b) = duplex(64);
        a.write_all(&frame(8, 0)[..5]).await.unwrap();
        drop(a);
        let mut reader = FrameReader::new(b, MAX_TRUSTED_FRAME);
        let error = reader.next().await.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn reader_is_safe_to_cancel() {
        let (mut a, b) = duplex(64);
        let mut reader = FrameReader::new(b, MAX_TRUSTED_FRAME);
        a.write_all(&frame(8, 5)[..5]).await.unwrap();
        let wait = tokio::time::timeout(std::time::Duration::from_millis(20), reader.next());
        assert!(wait.await.is_err());
        a.write_all(&frame(8, 5)[5..]).await.unwrap();
        assert_eq!(reader.next().await.unwrap().unwrap(), frame(8, 5));
    }

    #[tokio::test]
    async fn writer_keeps_order_and_ends_after_the_last_frame() {
        let (a, mut b) = duplex(1 << 20);
        let (outbox, task) = spawn_writer(a, 64);
        let mut expected = Vec::new();
        for index in 0..50u8 {
            let bytes = frame(4 + u16::from(index), index);
            expected.extend_from_slice(&bytes);
            outbox.try_send(bytes).unwrap();
        }
        drop(outbox);
        task.await.unwrap().unwrap();
        let mut wire = Vec::new();
        b.read_to_end(&mut wire).await.unwrap();
        assert_eq!(wire, expected);
    }

    #[tokio::test]
    async fn outbox_reports_full_and_closed() {
        // The peer does not read and the pipe holds 1 byte: the writer task stops in its write.
        let (a, b) = duplex(1);
        let (outbox, task) = spawn_writer(a, 2);
        let mut full = false;
        for _ in 0..8 {
            tokio::task::yield_now().await;
            if outbox.try_send(frame(4, 0)) == Err(SendError::Full) {
                full = true;
                break;
            }
        }
        assert!(full);
        drop(b);
        assert!(task.await.unwrap().is_err());
        assert_eq!(outbox.try_send(frame(4, 0)), Err(SendError::Closed));
        assert!(outbox.is_closed());
    }
}
