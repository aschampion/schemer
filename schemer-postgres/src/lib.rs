//! An adapter enabling use of the schemer schema migration library with
//! PostgreSQL.
//!
//! # Examples:
//!
//! ```rust
//! extern crate postgres;
//! #[macro_use]
//! extern crate schemer;
//! extern crate schemer_postgres;
//! extern crate uuid;
//!
//! use std::collections::HashSet;
//!
//! use postgres::Connection;
//! use postgres::transaction::Transaction;
//! use schemer::{Migration, Migrator};
//! use schemer_postgres::{PostgresAdapter, PostgresAdapterError, PostgresMigration};
//! use uuid::Uuid;
//!
//! struct MyExampleMigration;
//! migration!(
//!     MyExampleMigration,
//!     "4885e8ab-dafa-4d76-a565-2dee8b04ef60",
//!     [],
//!     "An example migration without dependencies.");
//!
//! impl PostgresMigration for MyExampleMigration {
//!     fn up(&self, transaction: &Transaction) -> Result<(), PostgresAdapterError> {
//!         transaction.execute("CREATE TABLE my_example (id integer PRIMARY KEY);", &[])?;
//!         Ok(())
//!     }
//!
//!     fn down(&self, transaction: &Transaction) -> Result<(), PostgresAdapterError> {
//!         transaction.execute("DROP TABLE my_example;", &[])?;
//!         Ok(())
//!     }
//! }
//!
//! fn main() {
//!     let conn = Connection::connect(
//!         "postgresql://postgres@localhost/?search_path=pg_temp",
//!         postgres::TlsMode::None).unwrap();
//!     let adapter = PostgresAdapter::new(&conn, None);
//!
//!     let mut migrator = Migrator::new(adapter);
//!
//!     let migration = Box::new(MyExampleMigration {});
//!     migrator.register(migration);
//!     migrator.up(None);
//! }
//! ```
#![warn(clippy::all)]

extern crate postgres;
#[cfg(test)]
#[macro_use]
extern crate schemer;
#[cfg(not(test))]
extern crate schemer;
extern crate uuid;


use std::collections::HashSet;

use postgres::{Connection, Error as PostgresError};
use postgres::transaction::Transaction;
use uuid::Uuid;

use schemer::{Adapter, Migration};


/// PostgreSQL-specific trait for schema migrations.
pub trait PostgresMigration: Migration {
    /// Apply a migration to the database using a transaction.
    fn up(&self, _transaction: &Transaction) -> Result<(), PostgresError> {
        Ok(())
    }

    /// Revert a migration to the database using a transaction.
    fn down(&self, _transaction: &Transaction) -> Result<(), PostgresError> {
        Ok(())
    }
}

pub type PostgresAdapterError = PostgresError;

/// Adapter between schemer and PostgreSQL.
pub struct PostgresAdapter<'a> {
    conn: &'a Connection,
    migration_metadata_table: String,
}

impl<'a> PostgresAdapter<'a> {
    /// Construct a PostgreSQL schemer adapter.
    ///
    /// `table_name` specifies the name of the table that schemer will use
    /// for storing metadata about applied migrations. If `None`, a default
    /// will be used.
    ///
    /// ```rust
    /// # extern crate postgres;
    /// # extern crate schemer_postgres;
    /// #
    /// # fn main() {
    /// let conn = postgres::Connection::connect(
    ///     "postgresql://postgres@localhost/?search_path=pg_temp",
    ///     postgres::TlsMode::None).unwrap();
    /// let adapter = schemer_postgres::PostgresAdapter::new(&conn, None);
    /// # }
    /// ```
    pub fn new(
        conn: &'a Connection,
        table_name: Option<String>,
    ) -> PostgresAdapter<'a> {
        PostgresAdapter {
            conn,
            migration_metadata_table: table_name.unwrap_or_else(|| "_schemer".into()),
        }
    }

    /// Initialize the schemer metadata schema. This must be called before
    /// using `Migrator` with this adapter. This is safe to call multiple times.
    pub fn init(&self) -> Result<(), PostgresError> {
        self.conn.execute(
            &format!(
                r#"
                    CREATE TABLE IF NOT EXISTS {} (
                        id uuid PRIMARY KEY
                    ) WITH (
                        OIDS=FALSE
                    )
                "#,
                self.migration_metadata_table
            ),
            &[],
        )?;
        Ok(())
    }
}

impl<'a> Adapter for PostgresAdapter<'a> {
    type MigrationType = dyn PostgresMigration;

    type Error = PostgresAdapterError;

    fn applied_migrations(&self) -> Result<HashSet<Uuid>, Self::Error> {
        let rows = self.conn.query(
            &format!(
                "SELECT id FROM {};",
                self.migration_metadata_table
            ),
            &[],
        )?;
        Ok(rows.iter().map(|row| row.get(0)).collect())
    }

    fn apply_migration(&mut self, migration: &Self::MigrationType) -> Result<(), Self::Error> {
        let trans = self.conn.transaction()?;
        migration.up(&trans)?;
        trans.execute(
            &format!(
                "INSERT INTO {} (id) VALUES ($1::uuid);",
                self.migration_metadata_table
            ),
            &[&migration.id()],
        )?;
        Ok(trans.commit()?)
    }

    fn revert_migration(&mut self, migration: &Self::MigrationType) -> Result<(), Self::Error> {
        let trans = self.conn.transaction()?;
        migration.down(&trans)?;
        trans.execute(
            &format!(
                "DELETE FROM {} WHERE id = $1::uuid;",
                self.migration_metadata_table
            ),
            &[&migration.id()],
        )?;
        Ok(trans.commit()?)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use postgres::TlsMode;
    use schemer::testing::*;

    impl PostgresMigration for TestMigration {}

    impl<'a> TestAdapter for PostgresAdapter<'a> {
        fn mock(id: Uuid, dependencies: HashSet<Uuid>) -> Box<Self::MigrationType> {
            Box::new(TestMigration::new(id, dependencies))
        }
    }

    fn build_test_connection () -> Connection {
        Connection::connect("postgresql://postgres@localhost/?search_path=pg_temp", TlsMode::None)
            .unwrap()
    }

    fn build_test_adapter(conn: &Connection) -> PostgresAdapter {
        let adapter = PostgresAdapter::new(conn, None);
        adapter.init().unwrap();
        adapter
    }

    test_schemer_adapter!(
        let conn = build_test_connection(),
        build_test_adapter(&conn));
}
