# rmcp-postgres

A PostgreSQL MCP (Model Context Protocol) server built with [rmcp](https://github.com/JosephCatrambone/rmcp), the Rust MCP SDK.

Provides comprehensive PostgreSQL database access through MCP tools for querying, inserting, updating, deleting, and inspecting database schemas.

## Features

- **Full CRUD Operations**: Query, insert, update, and delete data
- **Schema Inspection**: List tables, describe schemas, check existence
- **Relationship Discovery**: Explore foreign key relationships
- **Safety Limits**: Built-in protections for bulk operations
- **Configurable**: Flexible database connection configuration
- **Library + Binary**: Use as a standalone server or integrate into your own Rust projects

## Installation

### From crates.io (coming soon)

```bash
cargo install rmcp-postgres
```

### From source

```bash
git clone https://github.com/sqrew/rmcp-postgres
cd rmcp-postgres
cargo build --release
```

The binary will be at `target/release/rmcp-postgres`.

## Usage

### As a Standalone Server

Set your PostgreSQL connection string and run:

```bash
# Using environment variable
export POSTGRES_CONNECTION_STRING="host=localhost user=postgres dbname=mydb password=secret"
rmcp-postgres

# Using command line argument
rmcp-postgres --db-config "host=localhost user=postgres dbname=mydb password=secret"
```

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
rmcp-postgres = "0.1"
rmcp = { version = "0.12", features = ["server"] }
tokio = { version = "1", features = ["full"] }
```

Use in your code:

```rust
use rmcp::service::ServiceExt;
use rmcp_postgres::PostgresServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create server with connection string
    let server = PostgresServer::new("host=localhost user=postgres dbname=mydb");

    // Serve over stdio (for MCP)
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    server.serve((stdin, stdout)).await?;

    Ok(())
}
```

### Claude Desktop Configuration

Add to your Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "postgres": {
      "command": "rmcp-postgres",
      "env": {
        "POSTGRES_CONNECTION_STRING": "host=localhost user=postgres dbname=mydb password=secret"
      }
    }
  }
}
```

Or if installed from source:

```json
{
  "mcpServers": {
    "postgres": {
      "command": "/path/to/rmcp-postgres/target/release/rmcp-postgres",
      "env": {
        "POSTGRES_CONNECTION_STRING": "host=localhost user=postgres dbname=mydb"
      }
    }
  }
}
```

## Available Tools

### Data Operations

- **query_data** - Execute SELECT queries and return JSON results
- **insert_data** - Insert rows into tables
- **update_data** - Update rows with WHERE conditions (safety limit: 1000 rows)
- **delete_data** - Delete rows with WHERE conditions (safety limit: 1000 rows)
- **execute_raw_query** - Execute any SQL query (use with caution)

### Schema Inspection

- **list_tables** - List all tables in the database
- **get_schema** - Get column information for tables
- **describe_table** - Get detailed table info including indexes and constraints
- **table_exists** - Check if a table exists
- **column_exists** - Check if a column exists in a table

### Utilities

- **count_rows** - Count rows in a table with optional WHERE conditions
- **get_table_sample** - Get sample rows from a table (default: 10, max: 100)
- **get_relationships** - Get foreign key relationships between tables
- **get_connection_status** - Test connection and get database version info

## Connection String Format

PostgreSQL connection strings support multiple formats:

```bash
# Basic
"host=localhost user=postgres dbname=mydb"

# With password
"host=localhost user=postgres dbname=mydb password=secret"

# With port
"host=localhost port=5433 user=postgres dbname=mydb"

# Full URL format
"postgresql://postgres:secret@localhost:5432/mydb"
```

See the [tokio-postgres documentation](https://docs.rs/tokio-postgres/latest/tokio_postgres/config/struct.Config.html) for all connection options.

## Examples

### Query data

```json
{
  "query": "SELECT * FROM users WHERE active = true LIMIT 10"
}
```

### Insert data

```json
{
  "table_name": "users",
  "data": {
    "username": "alice",
    "email": "alice@example.com",
    "active": true
  }
}
```

### Update data

```json
{
  "table_name": "users",
  "values": {
    "active": false
  },
  "where_conditions": {
    "username": "alice"
  }
}
```

### Get schema for a table

```json
{
  "table_name": "users"
}
```

## Safety Features

- Update and delete operations have default limits (1000 rows)
- WHERE conditions required for updates and deletes
- Raw query execution requires explicit tool call
- Connection string passwords are sanitized in logs

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run with debug logging
RUST_LOG=rmcp_postgres=debug,rmcp=debug rmcp-postgres
```

## Contributing

Contributions welcome! Please feel free to submit issues or pull requests.

## License

MIT

## Related Projects

- [rmcp](https://github.com/JosephCatrambone/rmcp) - Rust MCP SDK
- [Model Context Protocol](https://github.com/anthropics/mcp) - The MCP specification
