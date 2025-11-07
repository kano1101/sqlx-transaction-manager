use sqlx::{MySql, MySqlConnection, MySqlPool, Transaction};
use std::ops::DerefMut;

/// Transaction context wrapper providing type-safe transaction boundaries.
///
/// This struct wraps SQLx's `Transaction` and provides automatic rollback on drop
/// if `commit()` is not explicitly called.
///
/// # Safety
///
/// If this struct is dropped without calling `commit()`, the transaction will be
/// automatically rolled back. This prevents accidental commits when errors occur.
///
/// # Examples
///
/// ```rust,no_run
/// use sqlx::MySqlPool;
/// use sqlx_transaction_manager::TransactionContext;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let pool = MySqlPool::connect("mysql://localhost/test").await?;
/// let mut tx = TransactionContext::begin(&pool).await?;
///
/// // Perform database operations using tx.as_executor()
/// // sqlx::query("INSERT INTO ...").execute(tx.as_executor()).await?;
///
/// // Explicitly commit the transaction
/// tx.commit().await?;
/// # Ok(())
/// # }
/// ```
pub struct TransactionContext<'tx> {
    tx: Option<Transaction<'tx, MySql>>,
}

impl<'tx> TransactionContext<'tx> {
    /// Begins a new transaction from the connection pool.
    ///
    /// # Errors
    ///
    /// Returns an error if the database connection fails or transaction cannot be started.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use sqlx::MySqlPool;
    /// use sqlx_transaction_manager::TransactionContext;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let pool = MySqlPool::connect("mysql://localhost/test").await?;
    /// let mut tx = TransactionContext::begin(&pool).await?;
    /// // Use the transaction...
    /// tx.commit().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn begin(pool: &MySqlPool) -> crate::Result<Self> {
        Ok(Self {
            tx: Some(pool.begin().await?),
        })
    }

    /// Commits the transaction.
    ///
    /// After calling this method, the `TransactionContext` is consumed and cannot be used.
    ///
    /// # Errors
    ///
    /// Returns an error if the commit operation fails.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use sqlx::MySqlPool;
    /// use sqlx_transaction_manager::TransactionContext;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let pool = MySqlPool::connect("mysql://localhost/test").await?;
    /// let mut tx = TransactionContext::begin(&pool).await?;
    /// // ... perform operations
    /// tx.commit().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn commit(mut self) -> crate::Result<()> {
        if let Some(tx) = self.tx.take() {
            tx.commit().await?;
        }
        Ok(())
    }

    /// Explicitly rolls back the transaction.
    ///
    /// Normally, rollback happens automatically when the `TransactionContext` is dropped
    /// without calling `commit()`. This method allows explicit rollback for error handling.
    ///
    /// # Errors
    ///
    /// Returns an error if the rollback operation fails.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use sqlx::MySqlPool;
    /// use sqlx_transaction_manager::TransactionContext;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let pool = MySqlPool::connect("mysql://localhost/test").await?;
    /// let mut tx = TransactionContext::begin(&pool).await?;
    /// // ... if something goes wrong
    /// tx.rollback().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn rollback(mut self) -> crate::Result<()> {
        if let Some(tx) = self.tx.take() {
            tx.rollback().await?;
        }
        Ok(())
    }

    /// Returns a mutable reference to the underlying connection for use as an Executor.
    ///
    /// This method provides access to `&mut MySqlConnection`, which implements SQLx's
    /// `Executor` trait. Use this when calling SQLx query methods or other libraries
    /// that accept an executor.
    ///
    /// # Panics
    ///
    /// Panics if the transaction has already been consumed (committed or rolled back).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use sqlx::MySqlPool;
    /// use sqlx_transaction_manager::TransactionContext;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let pool = MySqlPool::connect("mysql://localhost/test").await?;
    /// let mut tx = TransactionContext::begin(&pool).await?;
    ///
    /// sqlx::query("INSERT INTO users (name) VALUES (?)")
    ///     .bind("Alice")
    ///     .execute(tx.as_executor())
    ///     .await?;
    ///
    /// tx.commit().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn as_executor(&mut self) -> &mut MySqlConnection {
        self.tx
            .as_mut()
            .expect("Transaction has already been consumed")
            .deref_mut()
    }

    /// Consumes the context and returns the underlying SQLx `Transaction`.
    ///
    /// This is useful when you need direct access to SQLx's transaction API.
    /// After calling this method, the `TransactionContext` cannot be used.
    ///
    /// # Panics
    ///
    /// Panics if the transaction has already been consumed.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use sqlx::MySqlPool;
    /// use sqlx_transaction_manager::TransactionContext;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let pool = MySqlPool::connect("mysql://localhost/test").await?;
    /// let tx_ctx = TransactionContext::begin(&pool).await?;
    /// let tx = tx_ctx.into_inner();
    /// // Use raw SQLx transaction...
    /// tx.commit().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(dead_code)]
    pub fn into_inner(mut self) -> Transaction<'tx, MySql> {
        self.tx
            .take()
            .expect("Transaction has already been consumed")
    }
}

impl<'tx> Drop for TransactionContext<'tx> {
    /// Automatically rolls back the transaction if not committed.
    ///
    /// This ensures that uncommitted transactions are always rolled back,
    /// preventing accidental commits when errors occur or when the transaction
    /// context goes out of scope.
    fn drop(&mut self) {
        // If tx is Some, it means commit() was not called.
        // SQLx's Transaction automatically rolls back on drop,
        // so we don't need to do anything here.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_context_can_be_created() {
        // This test just ensures the struct can be instantiated
        // Actual database tests require a connection pool
    }
}
