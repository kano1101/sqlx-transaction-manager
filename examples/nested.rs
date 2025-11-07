use sqlx::MySqlPool;
use sqlx_transaction_manager::{with_nested_transaction, with_transaction};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Connect to database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://localhost/test".to_string());
    let pool = MySqlPool::connect(&database_url).await?;

    println!("=== Nested Transaction (Savepoint) Example ===\n");

    // Example 1: Successful nested transaction
    println!("1. Nested transaction - both succeed...");
    with_transaction(&pool, |tx| {
        Box::pin(async move {
            // Outer transaction: create user
            let result = sqlx::query("INSERT INTO users (name, email) VALUES (?, ?)")
                .bind("David")
                .bind("david@example.com")
                .execute(tx.as_executor())
                .await
                .map_err(|e| e.into())?;

            let user_id = result.last_insert_id() as i64;
            println!("   Outer: Created user with ID {}", user_id);

            // Nested transaction: create audit log
            with_nested_transaction(tx, |nested_tx| {
                Box::pin(async move {
                    sqlx::query("INSERT INTO audit_log (user_id, action) VALUES (?, ?)")
                        .bind(user_id)
                        .bind("User created")
                        .execute(nested_tx.as_executor())
                        .await
                        .map_err(|e| e.into())?;
                    println!("   Nested: Created audit log");
                    Ok(())
                })
            })
            .await?;

            println!("   ✓ Both transactions committed\n");
            Ok(())
        })
    })
    .await?;

    // Example 2: Nested transaction fails, outer succeeds
    println!("2. Nested transaction fails, outer succeeds...");
    with_transaction(&pool, |tx| {
        Box::pin(async move {
            // Outer transaction: create user
            let result = sqlx::query("INSERT INTO users (name, email) VALUES (?, ?)")
                .bind("Eve")
                .bind("eve@example.com")
                .execute(tx.as_executor())
                .await
                .map_err(|e| e.into())?;

            let user_id = result.last_insert_id() as i64;
            println!("   Outer: Created user with ID {}", user_id);

            // Nested transaction: try to create invalid audit log
            let nested_result = with_nested_transaction(tx, |nested_tx| {
                Box::pin(async move {
                    // This will fail
                    sqlx::query("INSERT INTO non_existent_table VALUES (?)")
                        .bind(user_id)
                        .execute(nested_tx.as_executor())
                        .await
                        .map_err(|e| e.into())?;
                    Ok(())
                })
            })
            .await;

            match nested_result {
                Ok(_) => println!("   ✗ Nested should have failed!"),
                Err(e) => println!("   Nested: Failed ({})", e),
            }

            println!("   Outer: Continuing despite nested failure...");
            println!("   ✓ Outer transaction committed (user created)\n");
            Ok(())
        })
    })
    .await?;

    // Example 3: Multiple nested transactions
    println!("3. Multiple nested transactions...");
    with_transaction(&pool, |tx| {
        Box::pin(async move {
            let result = sqlx::query("INSERT INTO users (name, email) VALUES (?, ?)")
                .bind("Frank")
                .bind("frank@example.com")
                .execute(tx.as_executor())
                .await
                .map_err(|e| e.into())?;

            let user_id = result.last_insert_id() as i64;
            println!("   Outer: Created user with ID {}", user_id);

            // First nested transaction: profile
            with_nested_transaction(tx, |nested_tx1| {
                Box::pin(async move {
                    sqlx::query("INSERT INTO profiles (user_id, bio) VALUES (?, ?)")
                        .bind(user_id)
                        .bind("Data Scientist")
                        .execute(nested_tx1.as_executor())
                        .await
                        .map_err(|e| e.into())?;
                    println!("   Nested 1: Created profile");
                    Ok(())
                })
            })
            .await?;

            // Note: MySQL savepoints with the same name overwrite previous ones
            // So we can reuse the same savepoint name for sequential nested transactions
            with_nested_transaction(tx, |nested_tx2| {
                Box::pin(async move {
                    sqlx::query("INSERT INTO audit_log (user_id, action) VALUES (?, ?)")
                        .bind(user_id)
                        .bind("Profile created")
                        .execute(nested_tx2.as_executor())
                        .await
                        .map_err(|e| e.into())?;
                    println!("   Nested 2: Created audit log");
                    Ok(())
                })
            })
            .await?;

            println!("   ✓ All transactions committed\n");
            Ok(())
        })
    })
    .await?;

    println!("=== All nested transaction examples completed ===");

    pool.close().await;
    Ok(())
}
