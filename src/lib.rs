//! PostgreSQL MCP Server
//!
//! A Model Context Protocol (MCP) server for PostgreSQL databases, built with rmcp.
//! Provides tools for querying, inserting, updating, deleting data, and inspecting schemas.
//!
//! # Example
//!
//! ```no_run
//! use rmcp_postgres::PostgresServer;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let server = PostgresServer::new("host=localhost user=postgres dbname=mydb");
//!     // Use with rmcp ServiceExt trait
//!     Ok(())
//! }
//! ```

use anyhow::Result;
use rmcp::{
    handler::server::{router::tool::ToolRouter, ServerHandler, wrapper::Parameters},
    model::*,
    ErrorData as McpError,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio_postgres::{NoTls, Row};

// ============================================================================
// Parameter Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct QueryParams {
    #[schemars(description = "SQL SELECT query to execute")]
    pub query: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SchemaParams {
    #[schemars(description = "Optional table name to filter schema")]
    pub table_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsertParams {
    #[schemars(description = "Table name to insert into")]
    pub table_name: String,
    #[schemars(description = "Data to insert as JSON object")]
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TableNameParams {
    #[schemars(description = "Name of the table")]
    pub table_name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CountRowsParams {
    #[schemars(description = "Name of the table to count rows from")]
    pub table_name: String,
    #[schemars(description = "Optional WHERE conditions as JSON object")]
    pub where_conditions: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ColumnExistsParams {
    #[schemars(description = "Name of the table")]
    pub table_name: String,
    #[schemars(description = "Name of the column to check")]
    pub column_name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TableSampleParams {
    #[schemars(description = "Name of the table to sample")]
    pub table_name: String,
    #[schemars(description = "Number of rows to return (default: 10, max: 100)")]
    pub limit: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateDataParams {
    #[schemars(description = "Name of the table to update")]
    pub table_name: String,
    #[schemars(description = "Object with column names as keys and new values")]
    pub values: serde_json::Value,
    #[schemars(description = "Object with column names as keys and values to match for WHERE clause")]
    pub where_conditions: serde_json::Value,
    #[schemars(description = "Maximum number of rows to update (safety limit, default: 1000)")]
    pub limit: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteDataParams {
    #[schemars(description = "Name of the table to delete from")]
    pub table_name: String,
    #[schemars(description = "Object with column names as keys and values to match for WHERE clause")]
    pub where_conditions: serde_json::Value,
    #[schemars(description = "Maximum number of rows to delete (safety limit, default: 1000)")]
    pub limit: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ExecuteRawQueryParams {
    #[schemars(description = "SQL query to execute (use with caution)")]
    pub query: String,
    #[schemars(description = "Optional array of parameters for parameterized queries")]
    pub params: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RelationshipsParams {
    #[schemars(description = "Optional table name to filter relationships")]
    pub table_name: Option<String>,
}

// ============================================================================
// PostgreSQL MCP Server
// ============================================================================

/// PostgreSQL MCP Server
///
/// Provides MCP tools for interacting with a PostgreSQL database.
pub struct PostgresServer {
    db_config: String,
    pub tool_router: ToolRouter<Self>,
}

impl PostgresServer {
    /// Create a new PostgreSQL MCP server
    ///
    /// # Arguments
    ///
    /// * `db_config` - PostgreSQL connection string (e.g., "host=localhost user=postgres dbname=mydb")
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rmcp_postgres::PostgresServer;
    ///
    /// let server = PostgresServer::new("host=localhost user=postgres dbname=mydb");
    /// ```
    pub fn new(db_config: impl Into<String>) -> Self {
        Self {
            db_config: db_config.into(),
            tool_router: Self::tool_router(),
        }
    }

    async fn get_client(&self) -> Result<tokio_postgres::Client> {
        let (client, connection) = tokio_postgres::connect(&self.db_config, NoTls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Connection error: {}", e);
            }
        });

        Ok(client)
    }

    fn row_to_json(&self, row: &Row) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        for (idx, column) in row.columns().iter().enumerate() {
            let value: serde_json::Value = match column.type_().name() {
                "int4" | "int8" => {
                    row.try_get::<_, i64>(idx)
                        .map(|v| serde_json::json!(v))
                        .unwrap_or(serde_json::Value::Null)
                }
                "float4" | "float8" => {
                    row.try_get::<_, f64>(idx)
                        .map(|v| serde_json::json!(v))
                        .unwrap_or(serde_json::Value::Null)
                }
                "bool" => {
                    row.try_get::<_, bool>(idx)
                        .map(|v| serde_json::json!(v))
                        .unwrap_or(serde_json::Value::Null)
                }
                "text" | "varchar" => {
                    row.try_get::<_, String>(idx)
                        .map(|v| serde_json::json!(v))
                        .unwrap_or(serde_json::Value::Null)
                }
                _ => {
                    row.try_get::<_, String>(idx)
                        .map(|v| serde_json::json!(v))
                        .unwrap_or(serde_json::Value::Null)
                }
            };

            map.insert(column.name().to_string(), value);
        }

        serde_json::Value::Object(map)
    }
}

// ============================================================================
// MCP Tools
// ============================================================================

#[rmcp::tool_router]
impl PostgresServer {
    /// Execute a SELECT query on the database
    #[rmcp::tool(description = "Execute a SELECT query and return results as JSON")]
    pub async fn query_data(
        &self,
        Parameters(params): Parameters<QueryParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self
            .get_client()
            .await
            .map_err(|e| McpError::internal_error(format!("DB connection failed: {}", e), None))?;

        let rows = client
            .query(&params.query, &[])
            .await
            .map_err(|e| McpError::internal_error(format!("Query failed: {}", e), None))?;

        let json_rows: Vec<serde_json::Value> = rows.iter().map(|row| self.row_to_json(row)).collect();

        let result = serde_json::json!({
            "rows": json_rows,
            "row_count": json_rows.len()
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }

    /// Get schema information for database tables
    #[rmcp::tool(description = "Get column information for database tables")]
    pub async fn get_schema(
        &self,
        Parameters(params): Parameters<SchemaParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self
            .get_client()
            .await
            .map_err(|e| McpError::internal_error(format!("DB connection failed: {}", e), None))?;

        let query = if let Some(table) = params.table_name {
            format!(
                "SELECT table_name, column_name, data_type, is_nullable
                 FROM information_schema.columns
                 WHERE table_name = '{}'
                 ORDER BY ordinal_position",
                table
            )
        } else {
            "SELECT table_name, column_name, data_type, is_nullable
             FROM information_schema.columns
             WHERE table_schema = 'public'
             ORDER BY table_name, ordinal_position"
                .to_string()
        };

        let rows = client
            .query(&query, &[])
            .await
            .map_err(|e| McpError::internal_error(format!("Schema query failed: {}", e), None))?;

        let schema: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "table_name": row.get::<_, String>(0),
                    "column_name": row.get::<_, String>(1),
                    "data_type": row.get::<_, String>(2),
                    "is_nullable": row.get::<_, String>(3),
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&schema).unwrap(),
        )]))
    }

    /// Insert data into a table
    #[rmcp::tool(description = "Insert a row into a database table")]
    pub async fn insert_data(
        &self,
        Parameters(params): Parameters<InsertParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self
            .get_client()
            .await
            .map_err(|e| McpError::internal_error(format!("DB connection failed: {}", e), None))?;

        let obj = params
            .data
            .as_object()
            .ok_or_else(|| McpError::invalid_params("Data must be a JSON object", None))?;

        let columns: Vec<String> = obj.keys().cloned().collect();
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("${}", i)).collect();

        let query = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            params.table_name,
            columns.join(", "),
            placeholders.join(", ")
        );

        // For now, convert all values to strings (we can improve this later)
        let values: Vec<String> = obj
            .values()
            .map(|v| v.to_string().trim_matches('"').to_string())
            .collect();

        let value_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            values.iter().map(|v| v as &(dyn tokio_postgres::types::ToSql + Sync)).collect();

        client
            .execute(&query, &value_refs[..])
            .await
            .map_err(|e| McpError::internal_error(format!("Insert failed: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Successfully inserted into {}",
            params.table_name
        ))]))
    }

    /// List all tables in the database
    #[rmcp::tool(description = "List all tables in the database")]
    pub async fn list_tables(&self) -> Result<CallToolResult, McpError> {
        let client = self
            .get_client()
            .await
            .map_err(|e| McpError::internal_error(format!("DB connection failed: {}", e), None))?;

        let rows = client
            .query(
                "SELECT tablename FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename",
                &[],
            )
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to list tables: {}", e), None))?;

        let tables: Vec<String> = rows.iter().map(|row| row.get(0)).collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&tables).unwrap(),
        )]))
    }

    /// Get detailed information about a table
    #[rmcp::tool(description = "Get detailed information about a table including indexes and constraints")]
    pub async fn describe_table(
        &self,
        Parameters(params): Parameters<TableNameParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self
            .get_client()
            .await
            .map_err(|e| McpError::internal_error(format!("DB connection failed: {}", e), None))?;

        // Get columns
        let columns = client
            .query(
                "SELECT column_name, data_type, is_nullable, column_default
                 FROM information_schema.columns
                 WHERE table_schema = 'public' AND table_name = $1
                 ORDER BY ordinal_position",
                &[&params.table_name],
            )
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to get columns: {}", e), None))?;

        let column_info: Vec<serde_json::Value> = columns
            .iter()
            .map(|row| {
                serde_json::json!({
                    "column_name": row.get::<_, String>(0),
                    "data_type": row.get::<_, String>(1),
                    "is_nullable": row.get::<_, String>(2),
                    "column_default": row.get::<_, Option<String>>(3),
                })
            })
            .collect();

        // Get indexes
        let indexes = client
            .query(
                "SELECT indexname, indexdef
                 FROM pg_indexes
                 WHERE schemaname = 'public' AND tablename = $1",
                &[&params.table_name],
            )
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to get indexes: {}", e), None))?;

        let index_info: Vec<serde_json::Value> = indexes
            .iter()
            .map(|row| {
                serde_json::json!({
                    "index_name": row.get::<_, String>(0),
                    "definition": row.get::<_, String>(1),
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "table_name": params.table_name,
                "columns": column_info,
                "indexes": index_info
            }))
            .unwrap(),
        )]))
    }

    /// Count rows in a table
    #[rmcp::tool(description = "Count rows in a table with optional WHERE conditions")]
    pub async fn count_rows(
        &self,
        Parameters(params): Parameters<CountRowsParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self
            .get_client()
            .await
            .map_err(|e| McpError::internal_error(format!("DB connection failed: {}", e), None))?;

        let query = if let Some(where_obj) = params.where_conditions {
            let conditions: Vec<String> = where_obj
                .as_object()
                .ok_or_else(|| McpError::invalid_params("WHERE conditions must be a JSON object", None))?
                .iter()
                .map(|(k, v)| format!("{} = '{}'", k, v.as_str().unwrap_or("")))
                .collect();

            format!("SELECT COUNT(*) FROM {} WHERE {}", params.table_name, conditions.join(" AND "))
        } else {
            format!("SELECT COUNT(*) FROM {}", params.table_name)
        };

        let row = client
            .query_one(&query, &[])
            .await
            .map_err(|e| McpError::internal_error(format!("Count query failed: {}", e), None))?;

        let count: i64 = row.get(0);

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "table_name": params.table_name,
                "count": count
            }))
            .unwrap(),
        )]))
    }

    /// Check if a table exists
    #[rmcp::tool(description = "Check if a table exists in the database")]
    pub async fn table_exists(
        &self,
        Parameters(params): Parameters<TableNameParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self
            .get_client()
            .await
            .map_err(|e| McpError::internal_error(format!("DB connection failed: {}", e), None))?;

        let row = client
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM pg_tables WHERE schemaname = 'public' AND tablename = $1)",
                &[&params.table_name],
            )
            .await
            .map_err(|e| McpError::internal_error(format!("Table exists query failed: {}", e), None))?;

        let exists: bool = row.get(0);

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "table_name": params.table_name,
                "exists": exists
            }))
            .unwrap(),
        )]))
    }

    /// Check if a column exists in a table
    #[rmcp::tool(description = "Check if a column exists in a table")]
    pub async fn column_exists(
        &self,
        Parameters(params): Parameters<ColumnExistsParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self
            .get_client()
            .await
            .map_err(|e| McpError::internal_error(format!("DB connection failed: {}", e), None))?;

        let row = client
            .query_one(
                "SELECT EXISTS (
                    SELECT 1 FROM information_schema.columns
                    WHERE table_schema = 'public'
                      AND table_name = $1
                      AND column_name = $2
                )",
                &[&params.table_name, &params.column_name],
            )
            .await
            .map_err(|e| McpError::internal_error(format!("Column exists query failed: {}", e), None))?;

        let exists: bool = row.get(0);

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "table_name": params.table_name,
                "column_name": params.column_name,
                "exists": exists
            }))
            .unwrap(),
        )]))
    }

    /// Get a sample of rows from a table
    #[rmcp::tool(description = "Get a sample of rows from a table")]
    pub async fn get_table_sample(
        &self,
        Parameters(params): Parameters<TableSampleParams>,
    ) -> Result<CallToolResult, McpError> {
        let limit = params.limit.unwrap_or(10).min(100);

        let client = self
            .get_client()
            .await
            .map_err(|e| McpError::internal_error(format!("DB connection failed: {}", e), None))?;

        let query = format!("SELECT * FROM {} LIMIT {}", params.table_name, limit);

        let rows = client
            .query(&query, &[])
            .await
            .map_err(|e| McpError::internal_error(format!("Sample query failed: {}", e), None))?;

        let json_rows: Vec<serde_json::Value> = rows.iter().map(|row| self.row_to_json(row)).collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "table_name": params.table_name,
                "rows": json_rows,
                "count": json_rows.len()
            }))
            .unwrap(),
        )]))
    }

    /// Update rows in a table
    #[rmcp::tool(description = "Update rows in a table with specified values and conditions")]
    pub async fn update_data(
        &self,
        Parameters(params): Parameters<UpdateDataParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self
            .get_client()
            .await
            .map_err(|e| McpError::internal_error(format!("DB connection failed: {}", e), None))?;

        let limit = params.limit.unwrap_or(1000);

        let values_obj = params
            .values
            .as_object()
            .ok_or_else(|| McpError::invalid_params("Values must be a JSON object", None))?;
        let where_obj = params
            .where_conditions
            .as_object()
            .ok_or_else(|| McpError::invalid_params("WHERE conditions must be a JSON object", None))?;

        let set_clauses: Vec<String> = values_obj
            .iter()
            .map(|(k, v)| format!("{} = '{}'", k, v.as_str().unwrap_or("")))
            .collect();

        let where_clauses: Vec<String> = where_obj
            .iter()
            .map(|(k, v)| format!("{} = '{}'", k, v.as_str().unwrap_or("")))
            .collect();

        let query = format!(
            "UPDATE {} SET {} WHERE {} LIMIT {}",
            params.table_name,
            set_clauses.join(", "),
            where_clauses.join(" AND "),
            limit
        );

        let rows_affected = client
            .execute(&query, &[])
            .await
            .map_err(|e| McpError::internal_error(format!("Update failed: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "table_name": params.table_name,
                "rows_affected": rows_affected
            }))
            .unwrap(),
        )]))
    }

    /// Delete rows from a table
    #[rmcp::tool(description = "Delete rows from a table based on specified conditions")]
    pub async fn delete_data(
        &self,
        Parameters(params): Parameters<DeleteDataParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self
            .get_client()
            .await
            .map_err(|e| McpError::internal_error(format!("DB connection failed: {}", e), None))?;

        let limit = params.limit.unwrap_or(1000);

        let where_obj = params
            .where_conditions
            .as_object()
            .ok_or_else(|| McpError::invalid_params("WHERE conditions must be a JSON object", None))?;

        let where_clauses: Vec<String> = where_obj
            .iter()
            .map(|(k, v)| format!("{} = '{}'", k, v.as_str().unwrap_or("")))
            .collect();

        let query = format!(
            "DELETE FROM {} WHERE {} LIMIT {}",
            params.table_name,
            where_clauses.join(" AND "),
            limit
        );

        let rows_affected = client
            .execute(&query, &[])
            .await
            .map_err(|e| McpError::internal_error(format!("Delete failed: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "table_name": params.table_name,
                "rows_affected": rows_affected
            }))
            .unwrap(),
        )]))
    }

    /// Execute a raw SQL query
    #[rmcp::tool(description = "Execute any SQL query including INSERT, UPDATE, DELETE (use with caution)")]
    pub async fn execute_raw_query(
        &self,
        Parameters(params): Parameters<ExecuteRawQueryParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self
            .get_client()
            .await
            .map_err(|e| McpError::internal_error(format!("DB connection failed: {}", e), None))?;

        // For SELECT queries, return results
        if params.query.trim().to_uppercase().starts_with("SELECT") {
            let rows = client
                .query(&params.query, &[])
                .await
                .map_err(|e| McpError::internal_error(format!("Query failed: {}", e), None))?;

            let json_rows: Vec<serde_json::Value> = rows.iter().map(|row| self.row_to_json(row)).collect();

            Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "rows": json_rows,
                    "count": json_rows.len()
                }))
                .unwrap(),
            )]))
        } else {
            // For other queries, return rows affected
            let rows_affected = client
                .execute(&params.query, &[])
                .await
                .map_err(|e| McpError::internal_error(format!("Query execution failed: {}", e), None))?;

            Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "rows_affected": rows_affected
                }))
                .unwrap(),
            )]))
        }
    }

    /// Get foreign key relationships for tables
    #[rmcp::tool(description = "Get foreign key relationships for tables")]
    pub async fn get_relationships(
        &self,
        Parameters(params): Parameters<RelationshipsParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = self
            .get_client()
            .await
            .map_err(|e| McpError::internal_error(format!("DB connection failed: {}", e), None))?;

        let query = if let Some(table) = params.table_name {
            format!(
                "SELECT
                    tc.table_name,
                    kcu.column_name,
                    ccu.table_name AS foreign_table_name,
                    ccu.column_name AS foreign_column_name
                FROM information_schema.table_constraints AS tc
                JOIN information_schema.key_column_usage AS kcu
                  ON tc.constraint_name = kcu.constraint_name
                  AND tc.table_schema = kcu.table_schema
                JOIN information_schema.constraint_column_usage AS ccu
                  ON ccu.constraint_name = tc.constraint_name
                  AND ccu.table_schema = tc.table_schema
                WHERE tc.constraint_type = 'FOREIGN KEY'
                  AND tc.table_schema = 'public'
                  AND tc.table_name = '{}'",
                table
            )
        } else {
            "SELECT
                tc.table_name,
                kcu.column_name,
                ccu.table_name AS foreign_table_name,
                ccu.column_name AS foreign_column_name
            FROM information_schema.table_constraints AS tc
            JOIN information_schema.key_column_usage AS kcu
              ON tc.constraint_name = kcu.constraint_name
              AND tc.table_schema = kcu.table_schema
            JOIN information_schema.constraint_column_usage AS ccu
              ON ccu.constraint_name = tc.constraint_name
              AND ccu.table_schema = tc.table_schema
            WHERE tc.constraint_type = 'FOREIGN KEY'
              AND tc.table_schema = 'public'"
                .to_string()
        };

        let rows = client
            .query(&query, &[])
            .await
            .map_err(|e| McpError::internal_error(format!("Relationships query failed: {}", e), None))?;

        let relationships: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "table_name": row.get::<_, String>(0),
                    "column_name": row.get::<_, String>(1),
                    "foreign_table_name": row.get::<_, String>(2),
                    "foreign_column_name": row.get::<_, String>(3),
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&relationships).unwrap(),
        )]))
    }

    /// Get database connection status
    #[rmcp::tool(description = "Get database connection status and basic info")]
    pub async fn get_connection_status(&self) -> Result<CallToolResult, McpError> {
        let client = self
            .get_client()
            .await
            .map_err(|e| McpError::internal_error(format!("DB connection failed: {}", e), None))?;

        let version_row = client
            .query_one("SELECT version()", &[])
            .await
            .map_err(|e| McpError::internal_error(format!("Version query failed: {}", e), None))?;

        let version: String = version_row.get(0);

        // Parse connection string to get database name
        let db_name = self
            .db_config
            .split_whitespace()
            .find(|s| s.starts_with("dbname="))
            .and_then(|s| s.strip_prefix("dbname="))
            .unwrap_or("unknown");

        let user = self
            .db_config
            .split_whitespace()
            .find(|s| s.starts_with("user="))
            .and_then(|s| s.strip_prefix("user="))
            .unwrap_or("unknown");

        let host = self
            .db_config
            .split_whitespace()
            .find(|s| s.starts_with("host="))
            .and_then(|s| s.strip_prefix("host="))
            .unwrap_or("localhost");

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "connected": true,
                "database": db_name,
                "user": user,
                "host": host,
                "version": version
            }))
            .unwrap(),
        )]))
    }
}

// ============================================================================
// Server Handler Implementation
// ============================================================================

#[rmcp::tool_handler]
impl ServerHandler for PostgresServer {
    fn get_info(&self) -> ServerInfo {
        InitializeResult {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability { list_changed: None }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "rmcp-postgres".to_string(),
                title: Some("PostgreSQL MCP Server".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                website_url: None,
            },
            instructions: Some("MCP server for PostgreSQL databases with full CRUD and schema inspection capabilities".to_string()),
        }
    }
}
