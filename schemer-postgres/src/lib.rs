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


pub trait PostgresMigration: Migration {
    fn up(&self, _transaction: &Transaction) -> Result<(), PostgresError> {
        Ok(())
    }

    fn down(&self, _transaction: &Transaction) -> Result<(), PostgresError> {
        Ok(())
    }
}

pub struct PostgresAdapter<'a> {
    conn: &'a Connection,
    migration_metadata_table: String,
}

impl<'a> PostgresAdapter<'a> {
    pub fn new(
        conn: &'a Connection,
        table: Option<String>,
    ) -> PostgresAdapter<'a> {
        PostgresAdapter {
            conn: conn,
            migration_metadata_table: table.unwrap_or_else(|| "_schemer".into()),
        }
    }

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
    type MigrationType = PostgresMigration;

    type Error = PostgresError;

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
