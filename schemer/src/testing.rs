//! Test harness for applying a generic test suite to any backend-specific
//! schemer adapter.

use super::*;

/// A trait required for running the generic test suite on an `Adapter`.
pub trait TestAdapter: Adapter {
    /// Construct a mock, no-op migration of the adapter's `MigrationType`.
    ///
    /// For convenience adapters can implement their migration traits on
    /// `TestMigration` and construct those here.
    fn mock(id: Uuid, dependencies: HashSet<Uuid>) -> Box<Self::MigrationType>;
}

/// A trivial struct implementing `Migration` on which adapters can build their
/// mock migrations.
pub struct TestMigration {
    id: Uuid,
    dependencies: HashSet<Uuid>,
}

impl TestMigration {
    pub fn new(id: Uuid, dependencies: HashSet<Uuid>) -> TestMigration {
        TestMigration { id, dependencies }
    }
}

impl Migration for TestMigration {
    fn id(&self) -> Uuid {
        self.id
    }

    fn dependencies(&self) -> HashSet<Uuid> {
        self.dependencies.clone()
    }

    fn description(&self) -> &'static str {
        "Test Migration"
    }
}

/// Test an `Adapter` with the generic test suite.
///
/// Note that the adapter must also implement the `TestAdapter` trait. This
/// should be done only for the testing configuration of the adapter's crate,
/// as it is not necessary for normal behavior.
///
/// # Examples
///
/// ```rust,ignore
/// #[macro_use] extern crate schemer;
///
/// fn construct_my_adapter_test_fixture() -> MyAdapterType {
///     MyAdapterType {}
/// }
///
/// test_schemer_adapter!(construct_my_adapter_test_fixture());
/// ```
#[macro_export]
macro_rules! test_schemer_adapter {
    ($constructor:expr) => {
        test_schemer_adapter!({}, $constructor);
    };
    ($setup:stmt, $constructor:expr) => {
        test_schemer_adapter!($setup, $constructor,
            test_single_migration,
            test_migration_chain,
            test_multi_component_dag,
            test_branching_dag,
        );
    };
    ($setup:stmt, $constructor:expr, $($test_fn:ident),* $(,)*) => {
        $(
            #[test]
            fn $test_fn() {
                $setup;
                let adapter = $constructor;
                $crate::testing::$test_fn(adapter);
            }
        )*
    }
}

/// Test the application and reversion of a singleton migration.
pub fn test_single_migration<A: TestAdapter>(adapter: A) {
    let migration1 = A::mock(
        Uuid::parse_str("bc960dc8-0e4a-4182-a62a-8e776d1e2b30").unwrap(),
        HashSet::new(),
    );
    let uuid1 = migration1.id();

    let mut migrator: Migrator<A> = Migrator::new(adapter);

    migrator.register(migration1).expect("Migration 1 registration failed");
    migrator.up(None).expect("Up migration failed");

    assert!(migrator.adapter.applied_migrations().unwrap().contains(
        &uuid1,
    ));

    migrator.down(None).expect("Down migration failed");

    assert!(!migrator.adapter.applied_migrations().unwrap().contains(
        &uuid1,
    ));
}

/// Test the partial application and reversion of a chain of three dependent
/// migrations.
pub fn test_migration_chain<A: TestAdapter>(adapter: A) {
    let migration1 = A::mock(
        Uuid::parse_str("bc960dc8-0e4a-4182-a62a-8e776d1e2b30").unwrap(),
        HashSet::new(),
    );
    let migration2 = A::mock(
        Uuid::parse_str("4885e8ab-dafa-4d76-a565-2dee8b04ef60").unwrap(),
        vec![migration1.id()].into_iter().collect(),
    );
    let migration3 = A::mock(
        Uuid::parse_str("c5d07448-851f-45e8-8fa7-4823d5250609").unwrap(),
        vec![migration2.id()].into_iter().collect(),
    );

    let uuid1 = migration1.id();
    let uuid2 = migration2.id();
    let uuid3 = migration3.id();

    let mut migrator = Migrator::new(adapter);

    migrator.register(migration1).expect("Migration 1 registration failed");
    migrator.register(migration2).expect("Migration 2 registration failed");
    migrator.register(migration3).expect("Migration 3 registration failed");

    migrator.up(Some(uuid2)).expect("Up migration failed");

    {
        let applied = migrator.adapter.applied_migrations().unwrap();
        assert!(applied.contains(&uuid1));
        assert!(applied.contains(&uuid2));
        assert!(!applied.contains(&uuid3));
    }

    migrator.down(Some(uuid1)).expect("Down migration failed");

    {
        let applied = migrator.adapter.applied_migrations().unwrap();
        assert!(applied.contains(&uuid1));
        assert!(!applied.contains(&uuid2));
        assert!(!applied.contains(&uuid3));
    }
}

/// Test that application and reversion of two DAG components are independent.
pub fn test_multi_component_dag<A: TestAdapter>(adapter: A) {
    let migration1 = A::mock(
        Uuid::parse_str("bc960dc8-0e4a-4182-a62a-8e776d1e2b30").unwrap(),
        HashSet::new(),
    );
    let migration2 = A::mock(
        Uuid::parse_str("4885e8ab-dafa-4d76-a565-2dee8b04ef60").unwrap(),
        vec![migration1.id()].into_iter().collect(),
    );
    let migration3 = A::mock(
        Uuid::parse_str("c5d07448-851f-45e8-8fa7-4823d5250609").unwrap(),
        HashSet::new(),
    );
    let migration4 = A::mock(
        Uuid::parse_str("9433a432-386f-467e-a59f-a9fb7e249767").unwrap(),
        vec![migration3.id()].into_iter().collect(),
    );

    let uuid1 = migration1.id();
    let uuid2 = migration2.id();
    let uuid3 = migration3.id();
    let uuid4 = migration4.id();

    let mut migrator = Migrator::new(adapter);

    migrator.register(migration1).expect("Migration 1 registration failed");
    migrator.register(migration2).expect("Migration 2 registration failed");
    migrator.register(migration3).expect("Migration 3 registration failed");
    migrator.register(migration4).expect("Migration 4 registration failed");

    migrator.up(Some(uuid2)).expect("Up migration failed");

    {
        let applied = migrator.adapter.applied_migrations().unwrap();
        assert!(applied.contains(&uuid1));
        assert!(applied.contains(&uuid2));
        assert!(!applied.contains(&uuid3));
        assert!(!applied.contains(&uuid4));
    }

    migrator.down(Some(uuid1)).expect("Down migration failed");

    {
        let applied = migrator.adapter.applied_migrations().unwrap();
        assert!(applied.contains(&uuid1));
        assert!(!applied.contains(&uuid2));
        assert!(!applied.contains(&uuid3));
        assert!(!applied.contains(&uuid4));
    }

    migrator.up(Some(uuid3)).expect("Up migration failed");

    {
        let applied = migrator.adapter.applied_migrations().unwrap();
        assert!(applied.contains(&uuid1));
        assert!(!applied.contains(&uuid2));
        assert!(applied.contains(&uuid3));
        assert!(!applied.contains(&uuid4));
    }

    migrator.up(None).expect("Up migration failed");

    {
        let applied = migrator.adapter.applied_migrations().unwrap();
        assert!(applied.contains(&uuid1));
        assert!(applied.contains(&uuid2));
        assert!(applied.contains(&uuid3));
        assert!(applied.contains(&uuid4));
    }

    migrator.down(None).expect("Down migration failed");

    {
        let applied = migrator.adapter.applied_migrations().unwrap();
        assert!(!applied.contains(&uuid1));
        assert!(!applied.contains(&uuid2));
        assert!(!applied.contains(&uuid3));
        assert!(!applied.contains(&uuid4));
    }
}

/// Test application and reversion on a branching DAG.
pub fn test_branching_dag<A: TestAdapter>(adapter: A) {
    let migration1 = A::mock(
        Uuid::parse_str("bc960dc8-0e4a-4182-a62a-8e776d1e2b30").unwrap(),
        HashSet::new(),
    );
    let migration2 = A::mock(
        Uuid::parse_str("4885e8ab-dafa-4d76-a565-2dee8b04ef60").unwrap(),
        HashSet::new(),
    );
    let migration3 = A::mock(
        Uuid::parse_str("c5d07448-851f-45e8-8fa7-4823d5250609").unwrap(),
        vec![migration1.id(), migration2.id()].into_iter().collect(),
    );
    let migration4 = A::mock(
        Uuid::parse_str("9433a432-386f-467e-a59f-a9fb7e249767").unwrap(),
        vec![migration3.id()].into_iter().collect(),
    );
    let migration5 = A::mock(
        Uuid::parse_str("0940acb1-0e2e-4b99-9d69-2302a9c74524").unwrap(),
        vec![migration3.id()].into_iter().collect(),
    );

    let uuid1 = migration1.id();
    let uuid2 = migration2.id();
    let uuid3 = migration3.id();
    let uuid4 = migration4.id();
    let uuid5 = migration5.id();

    let mut migrator = Migrator::new(adapter);

    migrator.register(migration1).expect("Migration 1 registration failed");
    migrator.register(migration2).expect("Migration 2 registration failed");
    migrator.register(migration3).expect("Migration 3 registration failed");
    migrator.register(migration4).expect("Migration 4 registration failed");
    migrator.register(migration5).expect("Migration 5 registration failed");

    migrator.up(Some(uuid4)).expect("Up migration failed");

    {
        let applied = migrator.adapter.applied_migrations().unwrap();
        assert!(applied.contains(&uuid1));
        assert!(applied.contains(&uuid2));
        assert!(applied.contains(&uuid3));
        assert!(applied.contains(&uuid4));
        assert!(!applied.contains(&uuid5));
    }

    migrator.down(Some(uuid1)).expect("Down migration failed");

    {
        let applied = migrator.adapter.applied_migrations().unwrap();
        assert!(applied.contains(&uuid1));
        assert!(applied.contains(&uuid2));
        assert!(!applied.contains(&uuid3));
        assert!(!applied.contains(&uuid4));
        assert!(!applied.contains(&uuid5));
    }
}
