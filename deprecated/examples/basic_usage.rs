use orsx::OrsxMigrate;

#[derive(OrsxMigrate)]
#[orsx_table("users")]
struct User {
    #[orsx_column(primary_key)]
    id: String,
    name: String,
    email: Option<String>,
    age: i32,
}

fn main() {
    // Verify trait implementation
    println!("Table name: {}", User::table_name());
    println!("Primary key: {}", User::primary_key_field());
    println!("Fields: {:?}", User::field_names());
    println!("Field types: {:?}", User::field_types());
    println!("Nullable: {:?}", User::field_nullable());
    println!("\nMigration SQL:\n{}", User::migration_sql());
}
