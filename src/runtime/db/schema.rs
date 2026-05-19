/// Initial schema DDL, loaded from the migration file at compile time.
pub const INITIAL_SCHEMA: &str = include_str!("migrations/001_initial.sql");
