/// Every fallible API in the workspace returns this error. Expression-level SPARQL
/// "type errors" are NOT `Error`s: they are `None` inside sq-expr and never abort a query.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("sparql: syntax error: {0}")]
    Syntax(String),
    #[error("sparql: unsupported: {0}")]
    Unsupported(String),
    #[error("sparql: SQLite error {code}: {msg}")]
    Sqlite { code: i32, msg: String },
    #[error("sparql: interrupted")]
    Interrupted,
    #[error("sparql: query timeout")]
    Timeout,
    #[error("sparql: memory budget exceeded ({0} MB)")]
    MemoryBudget(u64),
    #[error("sparql: RDF store schema missing or incompatible: {0}")]
    Schema(String),
    #[error("sparql: invalid argument: {0}")]
    Invalid(String),
    #[error("sparql: RDF parse error: {0}")]
    Parse(String),
    #[error("sparql: re-entrant use of connection state")]
    Reentrant,
    #[error("sparql: cannot modify the RDF store while a sparql_rows cursor is open")]
    WriteWhileReading,
    #[error("sparql: internal error: {0}")]
    Internal(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn internal(msg: impl Into<String>) -> Self {
        Error::Internal(msg.into())
    }
}
