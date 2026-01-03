//! Standalone PostgreSQL MCP server binary
//!
//! This binary provides a command-line interface for running rmcp-postgres
//! as an MCP server.
//!
//! # Configuration
//!
//! The database connection string can be provided via:
//! - Environment variable: `POSTGRES_CONNECTION_STRING`
//! - Command line argument: `--db-config <connection_string>`
//!
//! # Example
//!
//! ```bash
//! # Using environment variable
//! export POSTGRES_CONNECTION_STRING="host=localhost user=postgres dbname=mydb"
//! rmcp-postgres
//!
//! # Using command line argument
//! rmcp-postgres --db-config "host=localhost user=postgres dbname=mydb password=secret"
//! ```

use anyhow::{Context, Result};
use rmcp::service::ServiceExt;
use rmcp_postgres::PostgresServer;
use std::env;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rmcp_postgres=info,rmcp=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    // Get database connection string from environment or command line
    let db_config = get_db_config()?;

    tracing::info!("Starting PostgreSQL MCP server");
    tracing::debug!("Database config: {}", sanitize_connection_string(&db_config));

    // Create and run the server
    let server = PostgresServer::new(db_config);

    // Get stdio transport
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());

    // Serve over stdio
    server
        .serve((stdin, stdout))
        .await
        .context("Failed to serve MCP server over stdio")?;

    Ok(())
}

/// Get database configuration from environment or command line arguments
fn get_db_config() -> Result<String> {
    // Check command line arguments first
    let args: Vec<String> = env::args().collect();

    if args.len() >= 3 && args[1] == "--db-config" {
        return Ok(args[2].clone());
    }

    // Fall back to environment variable
    env::var("POSTGRES_CONNECTION_STRING").context(
        "Database connection string not provided. Set POSTGRES_CONNECTION_STRING environment variable or use --db-config argument"
    )
}

/// Sanitize connection string for logging (hide password)
fn sanitize_connection_string(conn_str: &str) -> String {
    if let Some(pwd_start) = conn_str.find("password=") {
        let mut sanitized = conn_str[..pwd_start].to_string();
        sanitized.push_str("password=***");

        // Find the end of the password value (next space or end of string)
        let after_pwd = &conn_str[pwd_start + 9..];
        if let Some(space_pos) = after_pwd.find(' ') {
            sanitized.push_str(&after_pwd[space_pos..]);
        }

        sanitized
    } else {
        conn_str.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_connection_string() {
        let input = "host=localhost user=postgres password=secret123 dbname=test";
        let output = sanitize_connection_string(input);
        assert!(output.contains("password=***"));
        assert!(!output.contains("secret123"));
        assert!(output.contains("host=localhost"));
        assert!(output.contains("dbname=test"));
    }

    #[test]
    fn test_sanitize_connection_string_no_password() {
        let input = "host=localhost user=postgres dbname=test";
        let output = sanitize_connection_string(input);
        assert_eq!(input, output);
    }
}
