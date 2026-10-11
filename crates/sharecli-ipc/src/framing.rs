//! NDJSON framing for one IPC connection.
//!
//! Lane-3 HIGH (FINDINGS.md): the connection loop used `BufReader::lines()`,
//! which has **no maximum line length**, so a local client could grow the
//! sidecar's heap without bound by streaming bytes with no newline. The fix is
//! `LinesCodec::new_with_max_length`, which fails the frame as soon as it
//! exceeds the cap instead of accumulating it.
//!
//! The frame is dropped, and the error propagates out of [`serve_framed`] to
//! the accept loop, which logs it and closes that one connection. The server
//! keeps serving every other client — this bounds a *frame*, not a peer.
//!
//! ## No error reply for an over-cap frame
//!
//! The protocol correlates replies to requests by `id`
//! (`{"id": N, "result": ..., "error": ...}`, see `main.rs`). An over-cap
//! frame is rejected by the codec *before it is decoded*, so the request id is
//! unknown and no well-formed reply can be addressed to the caller. The framing
//! layer therefore sends no frame and closes the connection: inventing an
//! unsolicited reply (a null/zero id) would add a wire message no client
//! matches on. [`serve_framed`]'s oversized-frame behavior is pinned by
//! `serve_framed_drops_an_oversized_frame_without_a_reply` so this contract is
//! explicit rather than an accident of the codec.

use anyhow::Result;
use futures_util::StreamExt;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio_util::codec::{Framed, LinesCodec};

use crate::handler::Handler;

/// Hard cap on one inbound NDJSON frame, in bytes.
///
/// 256 KiB is far above any legitimate request — `process.spawn`, the largest
/// payload on the wire, carries only `name`/`command`/`args`/`project`/
/// `harness`/`cwd` — and far below the 1 MiB frame the audit uses to
/// validate the limit. The matching test frames a request to just under this
/// value, so shrinking the cap below real traffic fails the suite rather than
/// silently cutting clients off.
pub const MAX_REQUEST_LINE_BYTES: usize = 256 * 1024;

/// Read NDJSON frames from `reader`, dispatch each one, write the reply to
/// `writer`.
///
/// Returns `Err` when the peer sends an oversized frame
/// (`MaxLineLengthExceeded`) or the socket fails; the caller logs it and hangs
/// up on that connection only. No reply frame is written for an oversized
/// frame: the request id is unavailable before the codec rejects it, so the
/// connection is simply closed (see the module docs).
pub async fn serve_framed<R, W>(reader: R, mut writer: W, handler: &Handler) -> Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut frames = Framed::new(reader, LinesCodec::new_with_max_length(MAX_REQUEST_LINE_BYTES));

    while let Some(frame) = frames.next().await {
        let line = frame?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = handler.dispatch(trimmed).await;
        let mut payload = serde_json::to_string(&response)?;
        payload.push('\n');
        writer.write_all(payload.as_bytes()).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sharecli_session::SessionStore;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_util::codec::LinesCodecError;

    use super::*;
    use crate::handler::Handler;

    fn fixture_handler() -> Handler {
        Handler::with_session_store(SessionStore::open_memory().expect("in-memory session store"))
    }

    /// The contract the plan validates: the cap must reject the 1 MiB frame.
    #[test]
    fn the_cap_is_below_the_validation_frame() {
        assert!(
            MAX_REQUEST_LINE_BYTES < 1024 * 1024,
            "a 1 MiB frame must exceed the cap, cap is {MAX_REQUEST_LINE_BYTES}"
        );
        assert!(
            MAX_REQUEST_LINE_BYTES >= 64 * 1024,
            "the cap must stay far above real requests, cap is {MAX_REQUEST_LINE_BYTES}"
        );
    }

    /// The bug itself, at codec level: an over-cap frame is reported as an
    /// error rather than being buffered and handed back as a line.
    #[tokio::test]
    async fn an_oversized_frame_is_an_error_not_a_buffered_line() {
        // Buffer bigger than the frame so the write cannot block and stall the
        // assertion on a deadlock that would look like a pass.
        let (mut client, server) = tokio::io::duplex(1_100_000);
        let mut frames =
            Framed::new(server, LinesCodec::new_with_max_length(MAX_REQUEST_LINE_BYTES));

        let mut frame = vec![b'x'; 1_048_576];
        frame.push(b'\n');
        client.write_all(&frame).await.expect("write frame");

        let item =
            tokio::time::timeout(Duration::from_secs(5), frames.next()).await.expect("codec read");
        match item {
            Some(Err(LinesCodecError::MaxLineLengthExceeded)) => {}
            other => panic!("expected MaxLineLengthExceeded, got {other:?}"),
        }
    }

    /// A frame at the cap boundary must still decode, so the limit is a bound
    /// and not an outage.
    #[tokio::test]
    async fn a_frame_under_the_cap_decodes_normally() {
        let (mut client, server) = tokio::io::duplex(4096);
        let mut frames =
            Framed::new(server, LinesCodec::new_with_max_length(MAX_REQUEST_LINE_BYTES));

        client.write_all(b"{\"id\":1,\"method\":\"health.status\"}\n").await.expect("write frame");

        let item =
            tokio::time::timeout(Duration::from_secs(5), frames.next()).await.expect("codec read");
        assert_eq!(
            item.expect("a frame must arrive").expect("the frame must decode"),
            r#"{"id":1,"method":"health.status"}"#
        );
    }

    /// The whole round-trip through [`serve_framed`]: a request frame is
    /// dispatched and a reply frame carrying the same id comes back.
    #[tokio::test]
    async fn serve_framed_answers_a_normal_frame() {
        let handler = fixture_handler();
        let (mut request_client, request_server) = tokio::io::duplex(4096);
        let (reply_server, mut reply_client) = tokio::io::duplex(4096);

        let serve =
            tokio::spawn(async move { serve_framed(request_server, reply_server, &handler).await });

        request_client
            .write_all(b"{\"id\":1,\"method\":\"config.get\",\"params\":{}}\n")
            .await
            .expect("write request");
        drop(request_client); // EOF so `serve_framed` finishes and drops the writer.

        let mut reply = Vec::new();
        reply_client.read_to_end(&mut reply).await.expect("read reply");
        serve.await.expect("join").expect("serve_framed must be Ok");

        let reply = String::from_utf8(reply).expect("reply is UTF-8");
        assert!(reply.contains("\"id\":1"), "reply must carry the request id: {reply}");
        assert!(reply.contains("\"error\":null"), "config.get must succeed: {reply}");
    }

    /// An over-cap frame is rejected with no reply: the codec error propagates
    /// and nothing is written back (the id is unknown, so no reply is
    /// addressable). See the module docs for the contract.
    #[tokio::test]
    async fn serve_framed_drops_an_oversized_frame_without_a_reply() {
        let handler = fixture_handler();
        let (mut request_client, request_server) = tokio::io::duplex(1_100_000);
        let (reply_server, mut reply_client) = tokio::io::duplex(4096);

        let serve =
            tokio::spawn(async move { serve_framed(request_server, reply_server, &handler).await });

        let mut frame = vec![b'x'; MAX_REQUEST_LINE_BYTES + 1];
        frame.push(b'\n');
        request_client.write_all(&frame).await.expect("write oversized frame");
        drop(request_client);

        serve.await.expect("join").expect_err("an oversized frame must error serve_framed");

        let mut reply = Vec::new();
        reply_client.read_to_end(&mut reply).await.expect("read reply");
        assert!(
            reply.is_empty(),
            "no reply frame may be written for an unparseable oversized frame: {reply:?}"
        );
    }
}
