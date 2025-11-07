//! # sqlx-transaction-manager
//!
//! A type-safe transaction management wrapper for SQLx with automatic commit/rollback.
//!
//! ## Features
//!
//! - **Automatic Rollback**: Transactions automatically roll back on drop if not explicitly committed
//! - **Type-Safe**: Transaction boundaries are enforced at compile time
//! - **Ergonomic API**: Simple `with_transaction` function for common use cases
//! - **Nested Transactions**: Support for savepoints to simulate nested transactions
//! - **Zero Runtime Overhead**: Thin wrapper around SQLx's native transaction support
//!
//! ## Quick Start
//!
//! Add to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! sqlx = { version = "0.8", features = ["mysql", "runtime-tokio"] }
//! sqlx-transaction-manager = "0.1"
//! ```
//!
//! ## Examples
//!
//! ### Basic Transaction
//!
//! ```rust,no_run
//! use sqlx::MySqlPool;
//! use sqlx_transaction_manager::with_transaction;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let pool = MySqlPool::connect("mysql://localhost/test").await?;
//!
//! with_transaction(&pool, |tx| {
//!     Box::pin(async move {
//!         sqlx::query("INSERT INTO users (name) VALUES (?)")
//!             .bind("Alice")
//!             .execute(tx.as_executor())
//!             .await?;
//!         Ok::<_, sqlx::Error>(())
//!     })
//! }).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Multiple Operations
//!
//! ```rust,no_run
//! use sqlx::MySqlPool;
//! use sqlx_transaction_manager::with_transaction;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let pool = MySqlPool::connect("mysql://localhost/test").await?;
//! let user_id = with_transaction(&pool, |tx| {
//!     Box::pin(async move {
//!         // Insert user
//!         let result = sqlx::query("INSERT INTO users (name) VALUES (?)")
//!             .bind("Bob")
//!             .execute(tx.as_executor())
//!             .await?;
//!
//!         let user_id = result.last_insert_id() as i64;
//!
//!         // Insert profile (same transaction)
//!         sqlx::query("INSERT INTO profiles (user_id, bio) VALUES (?, ?)")
//!             .bind(user_id)
//!             .bind("Software Developer")
//!             .execute(tx.as_executor())
//!             .await?;
//!
//!         // Both operations commit together
//!         Ok::<_, sqlx::Error>(user_id)
//!     })
//! }).await?;
//!
//! println!("Created user with ID: {}", user_id);
//! # Ok(())
//! # }
//! ```
//!
//! ### Using with sqlx-named-bind
//!
//! This library works seamlessly with `sqlx-named-bind`:
//!
//! ```rust,no_run
//! use sqlx::MySqlPool;
//! use sqlx_transaction_manager::with_transaction;
//! use sqlx_named_bind::PreparedQuery;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let pool = MySqlPool::connect("mysql://localhost/test").await?;
//! with_transaction(&pool, |tx| {
//!     Box::pin(async move {
//!         let name = "Charlie".to_string();
//!         let age = 30;
//!
//!         let mut query = PreparedQuery::new(
//!             "INSERT INTO users (name, age) VALUES (:name, :age)",
//!             |q, key| match key {
//!                 ":name" => q.bind(name.clone()),
//!                 ":age" => q.bind(age),
//!                 _ => q,
//!             }
//!         )?;
//!
//!         query.execute(tx.as_executor()).await?;
//!         Ok::<_, sqlx_named_bind::Error>(())
//!     })
//! }).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Nested Transactions (Savepoints)
//!
//! ```rust,no_run
//! use sqlx::MySqlPool;
//! use sqlx_transaction_manager::{with_transaction, with_nested_transaction};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let pool = MySqlPool::connect("mysql://localhost/test").await?;
//! with_transaction(&pool, |tx| {
//!     Box::pin(async move {
//!         // Main transaction operations
//!         sqlx::query("INSERT INTO users (name) VALUES (?)")
//!             .bind("David")
//!             .execute(tx.as_executor())
//!             .await?;
//!
//!         // Nested transaction with savepoint
//!         let _ = with_nested_transaction(tx, |nested_tx| {
//!             Box::pin(async move {
//!                 sqlx::query("INSERT INTO audit_log (action) VALUES (?)")
//!                     .bind("User created")
//!                     .execute(nested_tx.as_executor())
//!                     .await?;
//!                 Ok::<_, sqlx::Error>(())
//!             })
//!         }).await; // If this fails, only the audit log is rolled back
//!
//!         Ok::<_, sqlx::Error>(())
//!     })
//! }).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Manual Transaction Control
//!
//! For more control, use `TransactionContext` directly:
//!
//! ```rust,no_run
//! use sqlx::MySqlPool;
//! use sqlx_transaction_manager::TransactionContext;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let pool = MySqlPool::connect("mysql://localhost/test").await?;
//! let mut tx = TransactionContext::begin(&pool).await?;
//!
//! sqlx::query("INSERT INTO users (name) VALUES (?)")
//!     .bind("Eve")
//!     .execute(tx.as_executor())
//!     .await?;
//!
//! // Explicitly commit
//! tx.commit().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Error Handling
//!
//! When an error occurs inside a transaction, it automatically rolls back:
//!
//! ```rust,no_run
//! use sqlx::MySqlPool;
//! use sqlx_transaction_manager::with_transaction;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let pool = MySqlPool::connect("mysql://localhost/test").await?;
//! let result = with_transaction(&pool, |tx| {
//!     Box::pin(async move {
//!         sqlx::query("INSERT INTO users (name) VALUES (?)")
//!             .bind("Frank")
//!             .execute(tx.as_executor())
//!             .await?;
//!
//!         // This will cause a rollback
//!         return Err(sqlx::Error::RowNotFound);
//!
//!         #[allow(unreachable_code)]
//!         Ok::<_, sqlx::Error>(())
//!     })
//! }).await;
//!
//! assert!(result.is_err());
//! // The INSERT was rolled back, "Frank" is not in the database
//! # Ok(())
//! # }
//! ```
//!
//! ## How It Works
//!
//! 1. **TransactionContext**: Wraps SQLx's `Transaction` and tracks its state
//! 2. **Automatic Cleanup**: Uncommitted transactions are rolled back on drop
//! 3. **Type Safety**: Consumed transactions can't be reused (enforced at compile time)
//! 4. **Executor Access**: Provides `&mut MySqlConnection` for use with SQLx queries
//!
//! ## Limitations
//!
//! - Currently only supports MySQL (PostgreSQL and SQLite support planned)
//! - Nested transactions use savepoints (MySQL limitation)
//! - Error type is `sqlx_transaction_manager::Error` (wraps `sqlx::Error`)
//!
//! ## License
//!
//! Licensed under either of Apache License, Version 2.0 or MIT license at your option.

pub mod context;
pub mod error;
pub mod executor;

pub use context::TransactionContext;
pub use error::{Error, Result};
pub use executor::{with_nested_transaction, with_transaction};

/// Convenience re-exports for common use cases
pub mod prelude {
    pub use crate::context::TransactionContext;
    pub use crate::error::{Error, Result};
    pub use crate::executor::{with_nested_transaction, with_transaction};
}
