use anyhow::Result;
use std::future::Future;
use std::pin::Pin;

/// Object-safe async trait abstracting the transport layer for MCP client
/// communication.
///
/// Implementations handle the wire protocol details (stdio pipes, HTTP+SSE,
/// etc.) while [`super::McpClient`] remains generic over the transport type.
pub trait McpTransport: Send + Sync {
    /// Send a JSON-RPC message to the MCP server.
    fn send(&mut self, message: String) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Receive the next JSON-RPC message from the MCP server.
    /// Returns `None` when the transport is closed by the peer.
    fn recv(&mut self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + '_>>;

    /// Close the transport and release any associated resources.
    fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}

impl McpTransport for Box<dyn McpTransport> {
    fn send(&mut self, message: String) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        (**self).send(message)
    }

    fn recv(&mut self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + '_>> {
        (**self).recv()
    }

    fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        (**self).close()
    }
}
