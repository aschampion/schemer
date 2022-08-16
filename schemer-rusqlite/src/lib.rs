//! An adapter enabling use of the schemer schema migration library with
//! SQLite3.
//!
//! # Examples:
//!
//! ```rust
//! extern crate rusqlite;
//! #[macro_use]
//! extern crate schemer;
//! extern crate schemer_rusqlite;
//! extern crate uuid;
//!
//! use std::collections::HashSet;
//!
//! use rusqlite::{params, Connection, Transaction};
//! use schemer::{Migration, Migrator};
//! use schemer_rusqlite::{RusqliteAdapter, RusqliteAdapterError, RusqliteMigration};
//! use uuid::Uuid;
//!
//! struct MyExampleMigration;
//! migration!(
//!     MyExampleMigration,
//!     "4885e8ab-dafa-4d76-a565-2dee8b04ef60",
//!     [],
//!     "An example migration without dependencies.");
//!
//! impl RusqliteMigration for MyExampleMigration {
//!     fn up(&self, transaction: &Transaction) -> Result<(), RusqliteAdapterError> {
//!         transaction.execute("CREATE TABLE my_example (id integer PRIMARY KEY);", params![])?;
//!         Ok(())
//!     }
//!
//!     fn down(&self, transaction: &Transaction) -> Result<(), RusqliteAdapterError> {
//!         transaction.execute("DROP TABLE my_example;", params![])?;
//!         Ok(())
//!     }
//! }
//!
//! fn main() {
//!     let mut conn = Connection::open_in_memory().unwrap();
//!     let adapter = RusqliteAdapter::new(&mut conn, None);
//!
//!     let mut migrator = Migrator::new(adapter);
//!
//!     let migration = Box::new(MyExampleMigration {});
//!     migrator.register(migration);
//!     migrator.up(None);
//! }
//! ```
#![warn(clippy::all)]

use std::collections::HashSet;

use rusqlite::{params, Connection, Error as RusqliteError, Transaction};
use uuid::Uuid;

use schemer::{Adapter, Migration};

/// SQlite-specific trait for schema migrations.
pub trait RusqliteMigration: Migration {
    /// Apply a migration to the database using a transaction.
    fn up(&self, _transaction: &Transaction<'_>) -> Result<(), RusqliteError> {
        Ok(())
    }

    /// Revert a migration to the database using a transaction.
    fn down(&self, _transaction: &Transaction<'_>) -> Result<(), RusqliteError> {
        Ok(())
    }
}

pub type RusqliteAdapterError = RusqliteError;

struct WrappedUuid(Uuid);

impl rusqlite::types::FromSql for WrappedUuid {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        Ok(WrappedUuid(Uuid::from_slice(value.as_blob()?).map_err(
            |e| rusqlite::types::FromSqlError::Other(Box::new(e)),
        )?))
    }
}

/// Adapter between schemer and SQLite.
pub struct RusqliteAdapter<'a> {
    conn: &'a mut Connection,
    migration_metadata_table: String,
}

impl<'a> RusqliteAdapter<'a> {
    /// Construct a SQLite schemer adapter.
    ///
    /// `table_name` specifies the name of the table that schemer will use
    /// for storing metadata about applied migrations. If `None`, a default
    /// will be used.
    ///
    /// ```rust
    /// # extern crate rusqlite;
    /// # extern crate schemer_rusqlite;
    /// #
    /// # fn main() {
    /// let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    /// let adapter = schemer_rusqlite::RusqliteAdapter::new(&mut conn, None);
    /// # }
    /// ```
    pub fn new(conn: &'a mut Connection, table_name: Option<String>) -> RusqliteAdapter<'a> {
        RusqliteAdapter {
            conn,
            migration_metadata_table: table_name.unwrap_or_else(|| "_schemer".into()),
        }
    }

    /// Initialize the schemer metadata schema. This must be called before
    /// using `Migrator` with this adapter. This is safe to call multiple times.
    pub fn init(&self) -> Result<(), RusqliteError> {
        self.conn.execute(
            &format!(
                r#"
                    CREATE TABLE IF NOT EXISTS {} (
                        id blob PRIMARY KEY
                    )
                "#,
                self.migration_metadata_table
            ),
            params![],
        )?;
        Ok(())
    }
}

impl<'a> Adapter for RusqliteAdapter<'a> {
    type MigrationType = dyn RusqliteMigration;

    type Error = RusqliteAdapterError;

    fn applied_migrations(&mut self) -> Result<HashSet<Uuid>, Self::Error> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT id FROM {};",
            self.migration_metadata_table
        ))?;
        // TODO: have to do this rather than `collect` because Rusqlite has an
        // interface that goes against map conventions.
        let rows = stmt.query_map(params![], |row| row.get::<_, WrappedUuid>(0))?;
        let mut ids = HashSet::new();
        for row in rows {
            ids.insert(row?.0);
        }
        Ok(ids)
    }

    fn apply_migration(&mut self, migration: &Self::MigrationType) -> Result<(), Self::Error> {
        let trans = self.conn.transaction()?;
        migration.up(&trans)?;
        let uuid = migration.id();
        let uuid_bytes = &uuid.as_bytes()[..];
        trans.execute(
            &format!(
                "INSERT INTO {} (id) VALUES (?1);",
                self.migration_metadata_table
            ),
            &[&uuid_bytes],
        )?;
        trans.commit()
    }

    fn revert_migration(&mut self, migration: &Self::MigrationType) -> Result<(), Self::Error> {
        let trans = self.conn.transaction()?;
        migration.down(&trans)?;
        let uuid = migration.id();
        let uuid_bytes = &uuid.as_bytes()[..];
        trans.execute(
            &format!(
                "DELETE FROM {} WHERE id = ?1;",
                self.migration_metadata_table
            ),
            &[&uuid_bytes],
        )?;
        trans.commit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemer::test_schemer_adapter;
    use schemer::testing::*;

    impl RusqliteMigration for TestMigration {}

    impl<'a> TestAdapter for RusqliteAdapter<'a> {
        fn mock(id: Uuid, dependencies: HashSet<Uuid>) -> Box<Self::MigrationType> {
            Box::new(TestMigration::new(id, dependencies))
        }
    }

    fn build_test_connection() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    fn build_test_adapter(conn: &mut Connection) -> RusqliteAdapter<'_> {
        let adapter = RusqliteAdapter::new(conn, None);
        adapter.init().unwrap();
        adapter
    }

    test_schemer_adapter!(
        let mut conn = build_test_connection(),
        build_test_adapter(&mut conn));
}
