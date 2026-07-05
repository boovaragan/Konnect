//! High-level board operation helpers that wrap IPC calls with
//! transaction management and error handling.

use crate::client::KiCadIpcClient;
use anyhow::Result;

/// Wraps a sequence of IPC mutations in a commit transaction.
/// Rolls back automatically if the closure returns Err.
pub fn with_commit<F, R>(client: &KiCadIpcClient, description: &str, f: F) -> Result<R>
where
    F: FnOnce() -> Result<R>,
{
    let commit_id = client.begin_commit()?;
    match f() {
        Ok(result) => {
            client.push_commit(&commit_id, description)?;
            Ok(result)
        }
        Err(e) => {
            let _ = client.drop_commit(&commit_id);
            Err(e)
        }
    }
}
