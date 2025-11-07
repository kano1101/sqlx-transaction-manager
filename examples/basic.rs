use sqlx::MySqlPool;
use sqlx_transaction_manager::with_transaction;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Connect to database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://localhost/test".to_string());
    let pool = MySqlPool::connect(&database_url).await?;

    println!("=== Basic Transaction Example ===\n");

    // Example 1: Simple INSERT
    println!("1. Creating a user...");
    with_transaction(&pool, |tx| {
        Box::pin(async move {
            sqlx::query("INSERT INTO users (name, email) VALUES (?, ?)")
                .bind("Alice")
                .bind("alice@example.com")
                .execute(tx.as_executor())
                .await
                .map_err(|e| e.into())?;
            Ok(())
        })
    })
    .await?;
    println!("   ✓ User created successfully\n");

    // Example 2: Multiple operations in one transaction
    println!("2. Creating user with profile...");
    let user_id = with_transaction(&pool, |tx| {
        Box::pin(async move {
            // Insert user
            let result = sqlx::query("INSERT INTO users (name, email) VALUES (?, ?)")
                .bind("Bob")
                .bind("bob@example.com")
                .execute(tx.as_executor())
                .await
                .map_err(|e| e.into())?;

            let user_id = result.last_insert_id() as i64;

            // Insert profile (same transaction)
            sqlx::query("INSERT INTO profiles (user_id, bio) VALUES (?, ?)")
                .bind(user_id)
                .bind("Software Developer")
                .execute(tx.as_executor())
                .await
                .map_err(|e| e.into())?;

            // Both operations commit together
            Ok(user_id)
        })
    })
    .await?;
    println!("   ✓ User and profile created with ID: {}\n", user_id);

    // Example 3: Error handling and automatic rollback
    println!("3. Testing automatic rollback on error...");
    let result: Result<(), _> = with_transaction(&pool, |tx| {
        Box::pin(async move {
            sqlx::query("INSERT INTO users (name, email) VALUES (?, ?)")
                .bind("Charlie")
                .bind("charlie@example.com")
                .execute(tx.as_executor())
                .await
                .map_err(|e| e.into())?;

            // This will cause an error
            sqlx::query("SELECT * FROM non_existent_table")
                .execute(tx.as_executor())
                .await
                .map_err(|e| e.into())?;

            Ok(())
        })
    })
    .await;

    match result {
        Ok(_) => println!("   ✗ Should have failed!"),
        Err(e) => println!("   ✓ Transaction rolled back: {}\n", e),
    }

    // Example 4: Returning values from transactions
    println!("4. Returning values from transaction...");
    let (user_count, profile_count): (i64, i64) = with_transaction(&pool, |tx| {
        Box::pin(async move {
            let users: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
                .fetch_one(tx.as_executor())
                .await
                .map_err(|e| e.into())?;

            let profiles: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM profiles")
                .fetch_one(tx.as_executor())
                .await
                .map_err(|e| e.into())?;

            Ok((users.0, profiles.0))
        })
    })
    .await?;
    println!("   Users: {}, Profiles: {}\n", user_count, profile_count);

    println!("=== All examples completed successfully ===");

    pool.close().await;
    Ok(())
}
