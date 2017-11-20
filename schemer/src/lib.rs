extern crate daggy;
extern crate uuid;


use std::collections::{HashMap, HashSet, VecDeque};

use daggy::Dag;
use daggy::petgraph::EdgeDirection;
use daggy::Walker;
use uuid::Uuid;


pub trait Migration {
    fn id(&self) -> Uuid;

    fn dependencies(&self) -> HashSet<Uuid>;

    fn description(&self) -> &'static str;
}

pub trait Adapter {
    type MigrationType: Migration + ?Sized;

    type Error; // TODO: How does this work with error_chain?

    fn applied_migrations(&self) -> Result<HashSet<Uuid>, Self::Error>;

    // fn applied_migration_sinks(&self) -> Result<HashSet<Uuid>, Self::Error>;

    fn apply_migration(&mut self, &Self::MigrationType) -> Result<(), Self::Error>;

    fn revert_migration(&mut self, &Self::MigrationType) -> Result<(), Self::Error>;
}

pub struct Migrator<T: Adapter> {
    adapter: T,
    dependencies: Dag<Box<T::MigrationType>, ()>,
    id_map: HashMap<Uuid, daggy::NodeIndex>,
}

impl<T: Adapter> Migrator<T> {
    fn new(adapter: T) -> Migrator<T> {
        Migrator {
            adapter: adapter,
            dependencies: Dag::new(),
            id_map: HashMap::new(),
        }
    }

    fn register(&mut self, migration: Box<T::MigrationType>) -> Result<(), T::Error> {
        // TODO: check that this Id doesn't already exist in the graph.
        let id = migration.id();
        let depends = migration.dependencies();
        let migration_idx = self.dependencies.add_node(migration);

        for d in depends {
            let parent_idx = self.id_map.get(&d).expect("TODO");
            self.dependencies.add_edge(*parent_idx, migration_idx, ());
        }

        self.id_map.insert(id, migration_idx);

        Ok(())
    }

    fn up(&mut self, to: Option<Uuid>) -> Result<(), T::Error> {
        let mut target_ids = HashSet::new();
        match to {
            Some(sink_id) => {
                target_ids.insert(sink_id);
            }
            None => {
                target_ids.extend(
                    self.dependencies
                        .graph()
                        .externals(EdgeDirection::Outgoing)
                        .map(|idx| {
                            self.dependencies.node_weight(idx).expect("Impossible").id()
                        }),
                )
            }
        }

        let applied_migrations = self.adapter.applied_migrations()?;
        let mut to_visit: VecDeque<_> = target_ids
            .clone()
            .iter()
            .map(|id| *self.id_map.get(id).expect("ID map is malformed"))
            .collect();
        while !to_visit.is_empty() {
            let idx = to_visit.pop_front().expect("Not empty");
            let id = self.dependencies.node_weight(idx).expect("Impossible").id();
            target_ids.insert(id);
            to_visit.extend(
                self.dependencies
                    .parents(idx)
                    .iter(&self.dependencies)
                    .map(|(_, p_idx)| p_idx),
            );
        }

        // TODO: This is assuming the applied_migrations state is consistent
        // with the dependency graph.
        for idx in &daggy::petgraph::algo::toposort(self.dependencies.graph().clone(), None)
            .expect("Impossible because dependencies are a DAG")
        {
            let migration = self.dependencies.node_weight(*idx).expect("Impossible");
            let id = migration.id();
            if applied_migrations.contains(&id) || !target_ids.contains(&id) {
                continue;
            }

            self.adapter.apply_migration(&migration)?;
        }

        Ok(())
    }

    fn down(&mut self, to: Option<Uuid>) -> Result<(), T::Error> {
        unimplemented!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestAdapter {
        applied_migrations: HashSet<Uuid>,
    }

    impl TestAdapter {
        fn new() -> TestAdapter {
            TestAdapter { applied_migrations: HashSet::new() }
        }
    }

    #[derive(Debug)]
    struct TestAdapterError;

    impl Adapter for TestAdapter {
        type MigrationType = Migration;

        type Error = TestAdapterError;

        fn applied_migrations(&self) -> Result<HashSet<Uuid>, Self::Error> {
            Ok(self.applied_migrations.clone())
        }

        fn apply_migration(&mut self, migration: &Self::MigrationType) -> Result<(), Self::Error> {
            self.applied_migrations.insert(migration.id());
            Ok(())
        }

        fn revert_migration(&mut self, migration: &Self::MigrationType) -> Result<(), Self::Error> {
            self.applied_migrations.remove(&migration.id());
            Ok(())
        }
    }

    struct TestMigration {
        id: Uuid,
        dependencies: HashSet<Uuid>,
    }

    impl Migration for TestMigration {
        fn id(&self) -> Uuid {
            self.id.clone()
        }

        fn dependencies(&self) -> HashSet<Uuid> {
            self.dependencies.clone()
        }

        fn description(&self) -> &'static str {
            "Test Migration"
        }
    }

    #[test]
    fn test_single_migration() {
        let migration1 = Box::new(TestMigration {
            id: Uuid::parse_str("bc960dc8-0e4a-4182-a62a-8e776d1e2b30").unwrap(),
            dependencies: HashSet::new(),
        });
        let uuid1 = migration1.id();

        let adapter = TestAdapter::new();

        let mut migrator = Migrator::new(adapter);

        migrator.register(migration1);
        migrator.up(None);

        assert!(migrator.adapter.applied_migrations().unwrap().contains(
            &uuid1,
        ));
    }

    #[test]
    fn test_migration_chain() {
        let migration1 = Box::new(TestMigration {
            id: Uuid::parse_str("bc960dc8-0e4a-4182-a62a-8e776d1e2b30").unwrap(),
            dependencies: HashSet::new(),
        });
        let migration2 = Box::new(TestMigration {
            id: Uuid::parse_str("4885e8ab-dafa-4d76-a565-2dee8b04ef60").unwrap(),
            dependencies: vec![migration1.id().clone()].into_iter().collect(),
        });
        let migration3 = Box::new(TestMigration {
            id: Uuid::parse_str("c5d07448-851f-45e8-8fa7-4823d5250609").unwrap(),
            dependencies: vec![migration2.id().clone()].into_iter().collect(),
        });


        let uuid1 = migration1.id();
        let uuid2 = migration2.id();
        let uuid3 = migration3.id();

        let adapter = TestAdapter::new();

        let mut migrator = Migrator::new(adapter);

        migrator.register(migration1);
        migrator.register(migration2);
        migrator.register(migration3);

        migrator.up(Some(uuid2));

        assert!(migrator.adapter.applied_migrations().unwrap().contains(
            &uuid1,
        ));
        assert!(migrator.adapter.applied_migrations().unwrap().contains(
            &uuid2,
        ));
        assert!(!migrator.adapter.applied_migrations().unwrap().contains(
            &uuid3,
        ));
    }
}
