extern crate postgres;
extern crate schemer;
extern crate uuid;


use std::collections::HashSet;

use postgres::{Connection, Error as PostgresError, TlsMode};
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

pub struct PostgresAdapter {
    conn: Connection,
    migration_metadata_table: String,
}

impl PostgresAdapter {
    pub fn new<T: postgres::params::IntoConnectParams>(
        url: T,
        table: Option<String>,
    ) -> Result<PostgresAdapter, PostgresError> {
        Ok(PostgresAdapter {
            conn: Connection::connect(url, TlsMode::None)?,
            migration_metadata_table: table.unwrap_or("_schemer".into()),
        })
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

impl Adapter for PostgresAdapter {
    type MigrationType = PostgresMigration;

    type Error = PostgresError;

    fn applied_migrations(&self) -> Result<HashSet<Uuid>, Self::Error> {
        let rows = self.conn.query(
            &format!(
                r#"
                    SELECT id FROM {};
                "#,
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
                r#"
                    INSERT INTO {} (id) VALUES ($1::uuid);
                "#,
                self.migration_metadata_table
            ),
            &[&migration.id()],
        )?;
        trans.set_commit();
        Ok(())
    }

    fn revert_migration(&mut self, migration: &Self::MigrationType) -> Result<(), Self::Error> {
        let trans = self.conn.transaction()?;
        migration.down(&trans)?;
        trans.execute(
            &format!(
                r#"
                    DELETE FROM {} WHERE id = $1::uuid;
                "#,
                self.migration_metadata_table
            ),
            &[&migration.id()],
        )?;
        trans.set_commit();
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use schemer::tests::*;

    impl PostgresMigration for TestMigration {}

    impl TestAdapter for PostgresAdapter {
        fn mock(id: Uuid, dependencies: HashSet<Uuid>) -> Box<Self::MigrationType> {
            Box::new(TestMigration::new(id, dependencies))
        }
    }

    fn build_test_adapter() -> PostgresAdapter {
        let adapter =
            PostgresAdapter::new("postgresql://postgres@localhost/?search_path=pg_temp", None)
                .unwrap();
        adapter.init().unwrap();
        adapter
    }

    #[test]
    fn test_single_migration_postgres() {
        let adapter = build_test_adapter();
        test_single_migration(adapter);
    }

    #[test]
    fn test_migration_chain_postgres() {
        let adapter = build_test_adapter();
        test_migration_chain(adapter);
    }
}
