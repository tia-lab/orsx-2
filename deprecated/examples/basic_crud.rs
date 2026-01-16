use anyhow::Result;
use orsx::migrations::Migrations;
use orsx::prelude::*;
use sqlx::PgPool;

#[derive(OrsxMigrate, sqlx::FromRow, Debug, Clone, serde::Serialize)]
#[orsx_table("users")]
struct User {
    #[orsx_column(primary_key)]
    id: String,
    name: String,
    email: String,
    age: i32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== orso-postgres V2: Basic CRUD Example ===\n");

    // Connect to database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/orso_example".to_string());

    println!("Connecting to database...");
    let pool = PgPool::connect(&database_url).await?;

    // Create table using migrations
    println!("Running migrations...");
    let dummy = User {
        id: String::new(),
        name: String::new(),
        email: String::new(),
        age: 0,
    };

    Migrations::init(&pool, &[(dummy, None)]).await?;
    println!("✓ Table 'users' created\n");

    // CREATE
    println!("1. CREATE - Inserting user...");
    let user = User {
        id: "user_1".to_string(),
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        age: 30,
    };

    user.insert_into_table(&pool, User::table_name()).await?;
    println!("✓ Inserted: {:?}\n", user);

    // READ
    println!("2. READ - Fetching user by ID...");
    let retrieved = User::find_by_id_in_table(&pool, User::table_name(), "user_1").await?;
    println!("✓ Retrieved: {:?}\n", retrieved);

    // READ ALL
    println!("3. READ ALL - Fetching all users...");
    let all_users = User::fetch_all_from_table(&pool, User::table_name()).await?;
    println!("✓ Found {} users\n", all_users.len());

    // UPDATE
    println!("4. UPDATE - Updating user age...");
    if let Some(mut user) = retrieved {
        user.age = 31;
        user.update_in_table(&pool, User::table_name()).await?;
        println!("✓ Updated user age to 31\n");

        // Verify update
        let updated = User::find_by_id_in_table(&pool, User::table_name(), "user_1").await?;
        println!("✓ Verified: {:?}\n", updated);
    }

    // COUNT
    println!("5. COUNT - Counting users...");
    let count = User::count_in_table(&pool, User::table_name()).await?;
    println!("✓ Total users: {}\n", count);

    // DELETE
    println!("6. DELETE - Deleting user...");
    let deleted = User::delete_from_table(&pool, User::table_name(), "user_1").await?;
    println!("✓ Deleted {} rows\n", deleted);

    // Verify deletion
    let count_after = User::count_in_table(&pool, User::table_name()).await?;
    println!("✓ Users remaining: {}\n", count_after);

    println!("=== Example completed successfully! ===");

    Ok(())
}
