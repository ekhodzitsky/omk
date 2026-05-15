use anyhow::{Context, Result};
use tokio_stream::StreamExt;
use tracing::debug;

use crate::wire::client::{ProcessWireClient, WireMessage};

impl ProcessWireClient {
    pub(crate) async fn read_message_from_stdout(&mut self) -> Result<WireMessage> {
        // FramedRead<_, LinesCodec> caps each line at MAX_WIRE_LINE_LENGTH;
        // exceeding the cap surfaces here as a LinesCodecError. Without that
        // cap, an uncooperative peer that omits newlines drives the reader
        // to OOM the host.
        let line = match self.stdout_reader.next().await {
            Some(Ok(line)) => line,
            Some(Err(e)) => {
                return Err(e).context("Failed to read from kimi stdout");
            }
            None => anyhow::bail!("kimi stdout closed"),
        };
        debug!(line = %line, "Received wire message");
        let msg: WireMessage =
            serde_json::from_str(&line).context("Failed to parse wire message")?;
        Ok(msg)
    }
}
