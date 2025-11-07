# sqlx-transaction-manager

[![Crates.io](https://img.shields.io/crates/v/sqlx-transaction-manager.svg)](https://crates.io/crates/sqlx-transaction-manager)
[![Documentation](https://docs.rs/sqlx-transaction-manager/badge.svg)](https://docs.rs/sqlx-transaction-manager)
[![License](https://img.shields.io/crates/l/sqlx-transaction-manager.svg)](https://github.com/kano1101/sqlx-transaction-manager#license)

A type-safe transaction management wrapper for SQLx with automatic commit/rollback.

## Features

- ✅ **Automatic Rollback**: Transactions automatically roll back on drop if not explicitly committed
- ✅ **Type-Safe**: Transaction boundaries are enforced at compile time
- ✅ **Ergonomic API**: Simple `with_transaction` function for common use cases
- ✅ **Nested Transactions**: Support for savepoints to simulate nested transactions
- ✅ **Zero Runtime Overhead**: Thin wrapper around SQLx's native transaction support
- ✅ **Works with sqlx-named-bind**: Seamless integration with named parameter binding

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
sqlx = { version = "0.8", features = ["mysql", "runtime-tokio"] }
sqlx-transaction-manager = "0.1"
```

## Quick Start

```rust
use sqlx::MySqlPool;
use sqlx_transaction_manager::with_transaction;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = MySqlPool::connect("mysql://localhost/test").await?;

    with_transaction(&pool, |tx| {
        Box::pin(async move {
            sqlx::query("INSERT INTO users (name) VALUES (?)")
                .bind("Alice")
                .execute(tx.as_executor())
                .await?;
            Ok::<_, sqlx::Error>(())
        })
    }).await?;

    Ok(())
}
```

## Examples

### Multiple Operations in One Transaction

```rust
use sqlx::MySqlPool;
use sqlx_transaction_manager::with_transaction;

let user_id = with_transaction(&pool, |tx| {
    Box::pin(async move {
        // Insert user
        let result = sqlx::query("INSERT INTO users (name) VALUES (?)")
            .bind("Bob")
            .execute(tx.as_executor())
            .await?;

        let user_id = result.last_insert_id() as i64;

        // Insert profile (same transaction)
        sqlx::query("INSERT INTO profiles (user_id, bio) VALUES (?, ?)")
            .bind(user_id)
            .bind("Software Developer")
            .execute(tx.as_executor())
            .await?;

        // Both operations commit together
        Ok::<_, sqlx::Error>(user_id)
    })
}).await?;
```

### Using with sqlx-named-bind

```rust
use sqlx::MySqlPool;
use sqlx_transaction_manager::with_transaction;
use sqlx_named_bind::PreparedQuery;

with_transaction(&pool, |tx| {
    Box::pin(async move {
        let name = "Charlie".to_string();
        let age = 30;

        let mut query = PreparedQuery::new(
            "INSERT INTO users (name, age) VALUES (:name, :age)",
            |q, key| match key {
                ":name" => q.bind(name.clone()),
                ":age" => q.bind(age),
                _ => q,
            }
        )?;

        query.execute(tx.as_executor()).await?;
        Ok::<_, sqlx_named_bind::Error>(())
    })
}).await?;
```

### Nested Transactions (Savepoints)

```rust
use sqlx::MySqlPool;
use sqlx_transaction_manager::{with_transaction, with_nested_transaction};

with_transaction(&pool, |tx| {
    Box::pin(async move {
        // Main transaction operations
        sqlx::query("INSERT INTO users (name) VALUES (?)")
            .bind("David")
            .execute(tx.as_executor())
            .await?;

        // Nested transaction with savepoint
        let _ = with_nested_transaction(tx, |nested_tx| {
            Box::pin(async move {
                sqlx::query("INSERT INTO audit_log (action) VALUES (?)")
                    .bind("User created")
                    .execute(nested_tx.as_executor())
                    .await?;
                Ok::<_, sqlx::Error>(())
            })
        }).await; // If this fails, only the audit log is rolled back

        Ok::<_, sqlx::Error>(())
    })
}).await?;
```

### Manual Transaction Control

```rust
use sqlx::MySqlPool;
use sqlx_transaction_manager::TransactionContext;

let mut tx = TransactionContext::begin(&pool).await?;

sqlx::query("INSERT INTO users (name) VALUES (?)")
    .bind("Eve")
    .execute(tx.as_executor())
    .await?;

// Explicitly commit
tx.commit().await?;
```

## Comparison: Before and After

### Before (Raw SQLx)

```rust
let mut tx = pool.begin().await?;

match sqlx::query("INSERT INTO users (name) VALUES (?)")
    .bind("Alice")
    .execute(&mut *tx)
    .await
{
    Ok(_) => {
        tx.commit().await?;
    }
    Err(e) => {
        tx.rollback().await?;
        return Err(e);
    }
}
```

### After (sqlx-transaction-manager)

```rust
with_transaction(&pool, |tx| {
    Box::pin(async move {
        sqlx::query("INSERT INTO users (name) VALUES (?)")
            .bind("Alice")
            .execute(tx.as_executor())
            .await?;
        Ok::<_, sqlx::Error>(())
    })
}).await?;
```

## Error Handling

Transactions automatically roll back on error:

```rust
let result = with_transaction(&pool, |tx| {
    Box::pin(async move {
        sqlx::query("INSERT INTO users (name) VALUES (?)")
            .bind("Frank")
            .execute(tx.as_executor())
            .await?;

        // This will cause a rollback
        return Err(sqlx::Error::RowNotFound);

        Ok::<_, sqlx::Error>(())
    })
}).await;

assert!(result.is_err());
// The INSERT was rolled back
```

## How It Works

1. **TransactionContext**: Wraps SQLx's `Transaction` and tracks its state
2. **Automatic Cleanup**: Uncommitted transactions are rolled back on drop
3. **Type Safety**: Consumed transactions can't be reused (enforced at compile time)
4. **Executor Access**: Provides `&mut MySqlConnection` for use with SQLx queries

## Limitations

- Currently only supports MySQL (PostgreSQL and SQLite support planned)
- Nested transactions use savepoints (MySQL limitation)
- Error type is `sqlx_transaction_manager::Error` (wraps `sqlx::Error`)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
