//! A database schema migration library that supports directed acyclic graph
//! (DAG) dependencies between migrations.
//!
//! To use with a specific database, an adapter is required. Known adapter
//! crates:
//!
//! - PostgreSQL: [`schemer-postgres`](https://crates.io/crates/schemer-postgres)
#![cfg_attr(feature = "cargo-clippy", allow(doc_markdown))]

extern crate daggy;
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate uuid;


use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::{Debug, Display};

use daggy::Dag;
use daggy::petgraph::EdgeDirection;
use failure::Fail;
use uuid::Uuid;


#[macro_use]
pub mod testing;


/// Metadata for defining the identity and dependence relations of migrations.
/// Specific adapters require additional traits for actual application and
/// reversion of migrations.
pub trait Migration {
    /// Unique identifier for this migration.
    fn id(&self) -> Uuid;

    /// Set of IDs of all direct dependencies of this migration.
    fn dependencies(&self) -> HashSet<Uuid>;

    /// User-targeted description of this migration.
    fn description(&self) -> &'static str;
}

/// Create a trivial implementation of `Migration` for a type.
///
/// ## Example
///
/// ```rust
/// #[macro_use]
/// extern crate schemer;
/// extern crate uuid;
///
/// use schemer::Migration;
///
/// struct ParentMigration;
/// migration!(
///     ParentMigration,
///     "bc960dc8-0e4a-4182-a62a-8e776d1e2b30",
///     [],
///     "Parent migration in a DAG");
///
/// struct ChildMigration;
/// migration!(
///     ChildMigration,
///     "4885e8ab-dafa-4d76-a565-2dee8b04ef60",
///     ["bc960dc8-0e4a-4182-a62a-8e776d1e2b30",],
///     "Child migration in a DAG");
///
/// fn main() {
///     let parent = ParentMigration;
///     let child = ChildMigration;
///
///     assert!(child.dependencies().contains(&parent.id()));
/// }
/// ```
#[macro_export]
macro_rules! migration {
    ($name:ident, $id:expr, [ $( $dependency_id:expr ),* $(,)* ], $description:expr) => {
        impl $crate::Migration for $name {
            fn id(&self) -> uuid::Uuid {
                uuid::Uuid::parse_str($id).unwrap()
            }

            fn dependencies(&self) -> std::collections::HashSet<uuid::Uuid> {
                vec![
                    $(
                        uuid::Uuid::parse_str($dependency_id).unwrap(),
                    )*
                ].into_iter().collect()
            }

            fn description(&self) -> &'static str {
                $description
            }
        }
    }
}

/// Direction in which a migration is applied (`Up`) or reverted (`Down`).
#[derive(Debug)]
pub enum MigrationDirection {
    Up,
    Down,
}

impl Display for MigrationDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let printable = match *self {
            MigrationDirection::Up => "up",
            MigrationDirection::Down => "Down",
        };
        write!(f, "{}", printable)
    }
}

/// Trait necessary to adapt schemer's migration management to a stateful
/// backend.
pub trait Adapter {
    /// Type migrations must implement for this adapter.
    type MigrationType: Migration + ?Sized;

    /// Type of errors returned by this adapter.
    type Error: Debug + Fail;

    /// Returns the set of IDs for migrations that have been applied.
    fn applied_migrations(&self) -> Result<HashSet<Uuid>, Self::Error>;

    /// Apply a single migration.
    fn apply_migration(&mut self, &Self::MigrationType) -> Result<(), Self::Error>;

    /// Revert a single migration.
    fn revert_migration(&mut self, &Self::MigrationType) -> Result<(), Self::Error>;
}

/// Error resulting from the definition of migration identity and dependency.
#[derive(Debug, Fail)]
pub enum DependencyError {
    #[fail(display = "Duplicate migration ID {}", _0)]
    DuplicateId(Uuid),
    #[fail(display = "Unknown migration ID {}", _0)]
    UnknownId(Uuid),
    #[fail(display = "Cyclic dependency cased by edge from migration IDs {} to {}", from, to)]
    Cycle {
        from: Uuid,
        to: Uuid,
    }
}

/// Error resulting either from migration definitions or from migration
/// application with an adapter.
#[derive(Debug, Fail)]
pub enum MigratorError<T: Debug + Fail> {
    #[fail(display = "An error occurred due to migration dependencies")]
    Dependency(#[cause] DependencyError),
    #[fail(display = "An error occurred while interacting with the adapter.")]
    Adapter(#[cause] T),
    #[fail(display = "An error occurred while applying migration {} ({}) {}: {}.", id, description, direction, error)]
    Migration {
        id: Uuid,
        description: &'static str,
        direction: MigrationDirection,
        #[cause] error: T,
    },
}

impl<T: Debug + Fail> From<T> for MigratorError<T> {
    fn from(error: T) -> MigratorError<T> {
        MigratorError::Adapter(error)
    }
}

/// Primary schemer type for defining and applying migrations.
pub struct Migrator<T: Adapter> {
    adapter: T,
    dependencies: Dag<Box<T::MigrationType>, ()>,
    id_map: HashMap<Uuid, daggy::NodeIndex>,
}

impl<T: Adapter> Migrator<T> {
    /// Create a `Migrator` using the given `Adapter`.
    pub fn new(adapter: T) -> Migrator<T> {
        Migrator {
            adapter: adapter,
            dependencies: Dag::new(),
            id_map: HashMap::new(),
        }
    }

    /// Register a migration into the dependency graph.
    pub fn register(&mut self, migration: Box<T::MigrationType>) -> Result<(), MigratorError<T::Error>> {
        let id = migration.id();
        if self.id_map.contains_key(&id) {
            return Err(MigratorError::Dependency(DependencyError::DuplicateId(id)))
        }
        let depends = migration.dependencies();
        let migration_idx = self.dependencies.add_node(migration);

        for d in depends {
            let parent_idx = self.id_map.get(&d)
                .ok_or_else(|| MigratorError::Dependency(DependencyError::UnknownId(d)))?;
            self.dependencies.add_edge(*parent_idx, migration_idx, ())
                .or_else(|_| Err(MigratorError::Dependency(DependencyError::Cycle {from: d, to: id})))?;
        }

        self.id_map.insert(id, migration_idx);

        Ok(())
    }

    /// Collect the ids of recursively dependent migrations in `dir` induced
    /// starting from `id`. If `dir` is `Incoming`, this is all ancestors
    /// (dependencies); if `Outgoing`, this is all descendents (dependents).
    /// If `id` is `None`, this is all migrations starting from the sources or
    /// the sinks, respectively.
    fn induced_stream(&self, id: Option<Uuid>, dir: EdgeDirection) -> Result<HashSet<Uuid>, DependencyError> {
        let mut target_ids = HashSet::new();
        match id {
            Some(id) => {
                if !self.id_map.contains_key(&id) {
                    return Err(DependencyError::UnknownId(id))
                }
                target_ids.insert(id);
            }
            // This will eventually yield all migrations, so could be optimized.
            None => {
                target_ids.extend(
                    self.dependencies
                        .graph()
                        .externals(dir.opposite())
                        .map(|idx| {
                            self.dependencies.node_weight(idx)
                                             .expect("Impossible: indices from this graph")
                                             .id()
                        }),
                )
            }
        }

        let mut to_visit: VecDeque<_> = target_ids
            .clone()
            .iter()
            .map(|id| *self.id_map.get(id).expect("ID map is malformed"))
            .collect();
        while !to_visit.is_empty() {
            let idx = to_visit.pop_front().expect("Impossible: not empty");
            let id = self.dependencies.node_weight(idx)
                                      .expect("Impossible: indices from this graph")
                                      .id();
            target_ids.insert(id);
            to_visit.extend(
                self.dependencies
                    .graph()
                    .neighbors_directed(idx, dir),
            );
        }

        Ok(target_ids)
    }

    /// Apply migrations as necessary to so that the specified migration is
    /// applied (inclusive).
    ///
    /// If `to` is `None`, apply all registered migrations.
    pub fn up(&mut self, to: Option<Uuid>) -> Result<(), MigratorError<T::Error>> {
        let target_ids = self.induced_stream(to, EdgeDirection::Incoming)
                             .map_err(MigratorError::Dependency)?;

        // TODO: This is assuming the applied_migrations state is consistent
        // with the dependency graph.
        let applied_migrations = self.adapter.applied_migrations()?;
        for idx in &daggy::petgraph::algo::toposort(self.dependencies.graph(), None)
            .expect("Impossible: dependencies are a DAG")
        {
            let migration = self.dependencies.node_weight(*idx)
                                             .expect("Impossible: indices from this graph");
            let id = migration.id();
            if applied_migrations.contains(&id) || !target_ids.contains(&id) {
                continue;
            }

            self.adapter.apply_migration(migration)
                        .map_err(|e| MigratorError::Migration {
                            id: id,
                            description: migration.description(),
                            direction: MigrationDirection::Up,
                            error: e
                        })?;
        }

        Ok(())
    }

    /// Revert migrations as necessary so that no migrations dependent on the
    /// specified migration are applied. If the specified migration was already
    /// applied, it will still be applied.
    ///
    /// If `to` is `None`, revert all applied migrations.
    pub fn down(&mut self, to: Option<Uuid>) -> Result<(), MigratorError<T::Error>> {
        let mut target_ids = self.induced_stream(to, EdgeDirection::Outgoing)
                                 .map_err(MigratorError::Dependency)?;
        if let Some(sink_id) = to {
            target_ids.remove(&sink_id);
        }

        let applied_migrations = self.adapter.applied_migrations()?;
        for idx in daggy::petgraph::algo::toposort(self.dependencies.graph(), None)
            .expect("Impossible: dependencies are a DAG")
            .iter()
            .rev()
        {
            let migration = self.dependencies.node_weight(*idx)
                                             .expect("Impossible: indices from this graph");
            let id = migration.id();
            if !applied_migrations.contains(&id) || !target_ids.contains(&id) {
                continue;
            }

            self.adapter.revert_migration(migration)
                        .map_err(|e| MigratorError::Migration {
                            id: id,
                            description: migration.description(),
                            direction: MigrationDirection::Down,
                            error: e
                        })?;
        }

        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use super::testing::*;

    struct DefaultTestAdapter {
        applied_migrations: HashSet<Uuid>,
    }

    impl DefaultTestAdapter {
        fn new() -> DefaultTestAdapter {
            DefaultTestAdapter { applied_migrations: HashSet::new() }
        }
    }

    #[derive(Debug, Fail)]
    #[fail(display = "An error occurred.")]
    struct DefaultTestAdapterError;

    impl Adapter for DefaultTestAdapter {
        type MigrationType = Migration;

        type Error = DefaultTestAdapterError;

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

    impl TestAdapter for DefaultTestAdapter {
        fn mock(id: Uuid, dependencies: HashSet<Uuid>) -> Box<Self::MigrationType> {
            Box::new(TestMigration::new(id, dependencies))
        }
    }

    test_schemer_adapter!(DefaultTestAdapter::new());
}
