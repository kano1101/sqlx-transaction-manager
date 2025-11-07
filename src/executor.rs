use super::context::TransactionContext;
use sqlx::MySqlPool;
use std::future::Future;
use std::pin::Pin;

/// Executes a function within a database transaction.
///
/// This function handles the transaction lifecycle automatically:
/// - Begins a transaction
/// - Executes the provided function
/// - Commits on success
/// - Rolls back on error
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
/// Returns the result of the function execution, or an error if the transaction fails.
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust,no_run
/// use sqlx::MySqlPool;
/// use sqlx_transaction_manager::with_transaction;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let pool = MySqlPool::connect("mysql://localhost/test").await?;
/// with_transaction(&pool, |tx| {
///     Box::pin(async move {
///         sqlx::query("INSERT INTO users (name) VALUES (?)")
///             .bind("Alice")
///             .execute(tx.as_executor())
///             .await?;
///         Ok::<_, sqlx::Error>(())
///     })
/// }).await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Multiple Operations
///
/// ```rust,no_run
/// use sqlx::MySqlPool;
/// use sqlx_transaction_manager::with_transaction;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let pool = MySqlPool::connect("mysql://localhost/test").await?;
/// let user_id = with_transaction(&pool, |tx| {
///     Box::pin(async move {
///         let result = sqlx::query("INSERT INTO users (name) VALUES (?)")
///             .bind("Bob")
///             .execute(tx.as_executor())
///             .await?;
///
///         let user_id = result.last_insert_id() as i64;
///
///         sqlx::query("INSERT INTO profiles (user_id, bio) VALUES (?, ?)")
///             .bind(user_id)
///             .bind("Software Developer")
///             .execute(tx.as_executor())
///             .await?;
///
///         Ok::<_, sqlx::Error>(user_id)
///     })
/// }).await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Error Handling
///
/// ```rust,no_run
/// use sqlx::MySqlPool;
/// use sqlx_transaction_manager::with_transaction;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let pool = MySqlPool::connect("mysql://localhost/test").await?;
/// let result = with_transaction(&pool, |tx| {
///     Box::pin(async move {
///         sqlx::query("INSERT INTO users (name) VALUES (?)")
///             .bind("Charlie")
///             .execute(tx.as_executor())
///             .await?;
///
///         // If this fails, the entire transaction is rolled back
///         sqlx::query("INVALID SQL")
///             .execute(tx.as_executor())
///             .await?;
///
///         Ok::<_, sqlx::Error>(())
///     })
/// }).await;
///
/// assert!(result.is_err()); // Transaction was rolled back
/// # Ok(())
/// # }
/// ```
pub async fn with_transaction<F, T>(pool: &MySqlPool, f: F) -> crate::Result<T>
where
    F: for<'a> FnOnce(
        &'a mut TransactionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = crate::Result<T>> + Send + 'a>>,
    T: Send,
{
    let mut tx_ctx = TransactionContext::begin(pool).await?;

    match f(&mut tx_ctx).await {
        Ok(result) => {
            tx_ctx.commit().await?;
            Ok(result)
        }
        Err(e) => {
            // Explicitly rollback on error
            // (Transaction would auto-rollback on drop anyway, but this makes it clearer)
            let _ = tx_ctx.rollback().await;
            Err(e)
        }
    }
}

/// Executes a nested transaction using savepoints.
///
/// This function allows you to create a transaction within an existing transaction
/// by using MySQL savepoints. If the nested transaction fails, only operations
/// since the savepoint are rolled back.
///
/// # Type Parameters
///
/// * `F` - A function that takes a mutable `TransactionContext` and returns a future
/// * `Fut` - The future type returned by the function
/// * `T` - The return type (must be `Send`)
///
/// # Arguments
///
/// * `tx_ctx` - The existing transaction context
/// * `f` - The function to execute within the nested transaction
///
/// # Returns
///
/// Returns the result of the function execution, or an error if the savepoint operation fails.
///
/// # Examples
///
/// ```rust,no_run
/// use sqlx::MySqlPool;
/// use sqlx_transaction_manager::{with_transaction, with_nested_transaction};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let pool = MySqlPool::connect("mysql://localhost/test").await?;
/// with_transaction(&pool, |tx| {
///     Box::pin(async move {
///         // Outer transaction operations
///         sqlx::query("INSERT INTO users (name) VALUES (?)")
///             .bind("Alice")
///             .execute(tx.as_executor())
///             .await?;
///
///         // Nested transaction with savepoint
///         let nested_result = with_nested_transaction(tx, |nested_tx| {
///             Box::pin(async move {
///                 sqlx::query("INSERT INTO logs (message) VALUES (?)")
///                     .bind("User created")
///                     .execute(nested_tx.as_executor())
///                     .await?;
///                 Ok::<_, sqlx::Error>(())
///             })
///         }).await;
///
///         // If nested transaction fails, outer transaction can still succeed
///         if nested_result.is_err() {
///             println!("Logging failed, but user creation will still commit");
///         }
///
///         Ok::<_, sqlx::Error>(())
///     })
/// }).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Note
///
/// MySQL doesn't support true nested transactions. This function uses SAVEPOINTs
/// to simulate nested transaction behavior. The savepoint name is `nested_tx`.
pub async fn with_nested_transaction<F, T>(
    tx_ctx: &mut TransactionContext<'_>,
    f: F,
) -> crate::Result<T>
where
    F: for<'a> FnOnce(&'a mut TransactionContext<'_>) -> Pin<Box<dyn Future<Output = crate::Result<T>> + Send + 'a>>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_functions_exist() {
        // This test just ensures the functions are properly defined
        // Actual database tests require a connection pool
    }
}
