//! Language server for Iris reader specs, shared by stdio and in-process clients.
mod diagnostic;
mod document;
mod server;

use lsp_server::Connection;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Serve one LSP connection until an orderly shutdown and exit.
pub fn run(connection: Connection) -> Result<()> {
    server::run(connection)
}

/// Serve framed LSP messages on stdin/stdout. Callers report errors on stderr.
pub fn run_stdio() -> Result<()> {
    let (connection, threads) = Connection::stdio();
    // On failure, stdin may still be blocked. Let the CLI terminate instead of
    // joining that reader; an orderly exit notification stops it normally.
    run(connection)?;
    threads.join()?;
    Ok(())
}
