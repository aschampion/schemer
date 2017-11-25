use super::*;

pub trait TestAdapter: Adapter {
    fn mock(id: Uuid, dependencies: HashSet<Uuid>) -> Box<Self::MigrationType>;
}

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

#[macro_export]
macro_rules! test_schemer_adapter {
    ($constructor:expr) => {
        test_schemer_adapter!({}, $constructor);
    };
    ($setup:stmt, $constructor:expr) => {
        #[test]
        fn test_single_migration() {
            $setup;
            let adapter = $constructor;
            $crate::testing::test_single_migration(adapter);
        }

        #[test]
        fn test_migration_chain() {
            $setup;
            let adapter = $constructor;
            $crate::testing::test_migration_chain(adapter);
        }

        #[test]
        fn test_multi_component_dag() {
            $setup;
            let adapter = $constructor;
            $crate::testing::test_multi_component_dag(adapter);
        }
    }
}

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