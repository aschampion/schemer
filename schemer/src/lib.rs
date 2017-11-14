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

    fn applied_migration_sinks(&self) -> Result<HashSet<Uuid>, Self::Error>;

    fn apply_migration(&self, &Self::MigrationType) -> Result<(), Self::Error>;

    fn revert_migration(&self, &Self::MigrationType) -> Result<(), Self::Error>;
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
            let parent_idx = self.id_map.get(&d)?;
            self.dependencies.add_edge(*parent_idx, migration_idx, ());
        }

        self.id_map.insert(id, migration_idx);

        Ok(())
    }

    fn up(&self, to: Option<Uuid>) -> Result<(), T::Error> {
        let mut target_ids = HashSet::new();
        match to {
            Some(sink_id) => target_ids.insert(sink_id),
            None => target_ids.extend(
                self.dependencies
                    .graph()
                    .externals(EdgeDirection::Outgoing)
                    .map(|idx| {
                        self.dependencies.node_weight(idx).expect("Impossible").id()
                    }),
            ),
        }

        let applied_migrations = self.adapter.applied_migrations()?;
        let mut to_visit: VecDeque<_> = target_ids
            .clone()
            .iter()
            .map(|id| self.id_map.get(id).expect("ID map is malformed"))
            .collect();
        while !to_visit.is_empty() {
            let idx = to_visit.pop_front().expect("Not empty");
            let id = self.dependencies
                .node_weight(*idx)
                .expect("Impossible")
                .id();
            target_ids.insert(id);
            to_visit.extend(
                self.dependencies
                    .parents(*idx)
                    .iter(&self.dependencies)
                    .map(|(_, p_idx)| p_idx),
            );
        }

        // TODO: This is assuming the applied_migrations state is consistent
        // with the dependency graph.
        for idx in &daggy::petgraph::algo::toposort(self.dependencies.graph().clone(), None)? {
            let migration = self.dependencies.node_weight(*idx).expect("Impossible");
            let id = migration.id();
            if applied_migrations.contains(&id) || !target_ids.contains(&id) {
                continue;
            }

            self.adapter.apply_migration(&migration)?;
        }

        Ok(())
    }

    fn down(&self, to: Option<Uuid>) -> Result<(), T::Error> {
        unimplemented!();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
