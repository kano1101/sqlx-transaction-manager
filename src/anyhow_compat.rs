use super::context::TransactionContext;
use sqlx::MySqlPool;
use std::future::Future;
use std::pin::Pin;

/// Executes a function within a database transaction, using anyhow::Error for error handling.
///
/// This is a convenience wrapper around the main `with_transaction` function that accepts
/// closures returning `anyhow::Result<T>` instead of `crate::Result<T>`.
///
/// # Type Parameters
///
/// * `F` - A function that takes a mutable `TransactionContext` and returns a pinned future
/// * `T` - The return type of the function (must be `Send`)
///
/// # Arguments
///
/// * `pool` - The MySQL connection pool
/// * `f` - The function to execute within the transaction
///
/// # Returns
///
/// Returns the result of the function execution as `anyhow::Result<T>`.
///
/// # Examples
///
/// ```rust,no_run
/// use sqlx::MySqlPool;
/// use sqlx_transaction_manager::with_transaction_anyhow;
/// use sqlx_named_bind::PreparedQuery;
///
/// # async fn example() -> anyhow::Result<()> {
/// # let pool = MySqlPool::connect("mysql://localhost/test").await?;
/// with_transaction_anyhow(&pool, |tx| {
///     Box::pin(async move {
///         let mut query = PreparedQuery::new(
///             "INSERT INTO users (name) VALUES (:name)",
///             |q, key| match key {
///                 ":name" => q.bind("Alice"),
///                 _ => q,
///             }
///         )?;
///         query.execute(tx.as_executor()).await?;
///         Ok(())
///     })
/// }).await?;
/// # Ok(())
/// # }
/// ```
pub async fn with_transaction_anyhow<F, T>(pool: &MySqlPool, f: F) -> anyhow::Result<T>
where
    F: for<'a> FnOnce(
        &'a mut TransactionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send + 'a>>,
    T: Send,
{
    let mut tx_ctx = TransactionContext::begin(pool).await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    match f(&mut tx_ctx).await {
        Ok(result) => {
            tx_ctx.commit().await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            Ok(result)
        }
        Err(e) => {
            let _ = tx_ctx.rollback().await;
            Err(e)
        }
    }
}

/// Executes a nested transaction using savepoints, with anyhow::Error for error handling.
///
/// This is a convenience wrapper for nested transactions that accepts closures
/// returning `anyhow::Result<T>`.
///
/// # Examples
///
/// ```rust,no_run
/// use sqlx::MySqlPool;
/// use sqlx_transaction_manager::{with_transaction_anyhow, with_nested_transaction_anyhow};
///
/// # async fn example() -> anyhow::Result<()> {
/// # let pool = MySqlPool::connect("mysql://localhost/test").await?;
/// with_transaction_anyhow(&pool, |tx| {
///     Box::pin(async move {
///         sqlx::query("INSERT INTO users (name) VALUES (?)")
///             .bind("Alice")
///             .execute(tx.as_executor())
///             .await?;
///
///         with_nested_transaction_anyhow(tx, |nested_tx| {
///             Box::pin(async move {
///                 sqlx::query("INSERT INTO audit_log (action) VALUES (?)")
///                     .bind("User created")
///                     .execute(nested_tx.as_executor())
///                     .await?;
///                 Ok(())
///             })
///         }).await?;
///
///         Ok(())
///     })
/// }).await?;
/// # Ok(())
/// # }
/// ```
pub async fn with_nested_transaction_anyhow<F, T>(
    tx_ctx: &mut TransactionContext<'_>,
    f: F,
) -> anyhow::Result<T>
where
    F: for<'a> FnOnce(&'a mut TransactionContext<'_>) -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send + 'a>>,
    T: Send,
{
    // Create a savepoint
    sqlx::query("SAVEPOINT nested_tx")
        .execute(tx_ctx.as_executor())
        .await?;

    match f(tx_ctx).await {
        Ok(result) => {
            // Release savepoint (equivalent to commit)
            sqlx::query("RELEASE SAVEPOINT nested_tx")
                .execute(tx_ctx.as_executor())
                .await?;
            Ok(result)
        }
        Err(e) => {
            // Rollback to savepoint
            let _ = sqlx::query("ROLLBACK TO SAVEPOINT nested_tx")
                .execute(tx_ctx.as_executor())
                .await;
            Err(e)
        }
    }
}
