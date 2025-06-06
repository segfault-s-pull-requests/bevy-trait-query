use bevy_ecs::{
    change_detection::{DetectChanges, Ref},
    component::{ComponentId, Tick},
    entity::Entity,
    ptr::UnsafeCellDeref,
    storage::{SparseSets, Table, TableRow},
};

use crate::{zip_exact, TraitImplMeta, TraitImplRegistry, TraitQuery};

/// Read-access to all components implementing a trait for a given entity.
///
/// This supports change detection and detection for added objects via
///
/// - [`ReadTraits::iter_changed`]
/// - [`ReadTraits::iter_added`]
pub struct ReadTraits<'a, Trait: ?Sized + TraitQuery> {
    // Read-only access to the global trait registry.
    // Since no one outside of the crate can name the registry type,
    // we can be confident that no write accesses will conflict with this.
    pub(crate) registry: &'a TraitImplRegistry<Trait>,
    pub(crate) table: &'a Table,
    pub(crate) table_row: TableRow,
    /// This grants shared access to all sparse set components,
    /// but in practice we will only read the components specified in `self.registry`.
    /// The fetch impl registers read-access for all of these components,
    /// so there will be no runtime conflicts.
    pub(crate) sparse_sets: &'a SparseSets,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

#[doc(hidden)]
pub type CombinedReadTraitsIter<'a, Trait> =
    std::iter::Chain<ReadTableTraitsIter<'a, Trait>, ReadSparseTraitsIter<'a, Trait>>;

#[doc(hidden)]
pub struct ReadTableTraitsIter<'a, Trait: ?Sized> {
    // SAFETY: These two iterators must have equal length.
    pub(crate) components: std::slice::Iter<'a, ComponentId>,
    pub(crate) meta: std::slice::Iter<'a, TraitImplMeta<Trait>>,
    pub(crate) table_row: TableRow,
    // Grants shared access to the components corresponding to `components` in this table.
    // Not all components are guaranteed to exist in the table.
    pub(crate) table: &'a Table,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

impl<'a, Trait: ?Sized + TraitQuery> Iterator for ReadTableTraitsIter<'a, Trait> {
    type Item = Ref<'a, Trait>;
    fn next(&mut self) -> Option<Self::Item> {
        // Iterate the remaining table components that are registered,
        // until we find one that exists in the table.
        let (ptr, component, meta) = unsafe { zip_exact(&mut self.components, &mut self.meta) }
            .find_map(|(&component, meta)| {
                // SAFETY: we know that the `table_row` is a valid index.
                let ptr = unsafe { self.table.get_component(component, self.table_row) }?;
                Some((ptr, component, meta))
            })?;
        let trait_object = unsafe { meta.dyn_ctor.cast(ptr) };

        // SAFETY:
        // Read access has been registered, so we can dereference it immutably.
        let added_tick = unsafe {
            self.table
                .get_added_tick(component, self.table_row)?
                .deref()
        };
        let changed_tick = unsafe {
            self.table
                .get_changed_tick(component, self.table_row)?
                .deref()
        };
        let changed_by = unsafe {
            self.table
                .get_changed_by(component, self.table_row)?
                .deref()
        };

        Some(Ref::new(
            trait_object,
            added_tick,
            changed_tick,
            self.last_run,
            self.this_run,

            #[cfg(feature = "track_change_detection")]
            changed_by
        ))
    }
}

#[doc(hidden)]
pub struct ReadSparseTraitsIter<'a, Trait: ?Sized> {
    // SAFETY: These two iterators must have equal length.
    pub(crate) components: std::slice::Iter<'a, ComponentId>,
    pub(crate) meta: std::slice::Iter<'a, TraitImplMeta<Trait>>,
    pub(crate) entity: Entity,
    // Grants shared access to the components corresponding to both `components` and `entity`.
    pub(crate) sparse_sets: &'a SparseSets,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

impl<'a, Trait: ?Sized + TraitQuery> Iterator for ReadSparseTraitsIter<'a, Trait> {
    type Item = Ref<'a, Trait>;
    fn next(&mut self) -> Option<Self::Item> {
        // Iterate the remaining sparse set components that are registered,
        // until we find one that exists in the archetype.
        let (ptr, ticks_ptr, meta, changed_by) = unsafe { zip_exact(&mut self.components, &mut self.meta) }
            .find_map(|(&component, meta)| {
                let set = self.sparse_sets.get(component)?;
                let (ptr, ticks, changed_by) = set.get_with_ticks(self.entity)?;
                Some((ptr, ticks, meta, changed_by))
            })?;
        let trait_object = unsafe { meta.dyn_ctor.cast(ptr) };
        let added_tick = unsafe { ticks_ptr.added.deref() };
        let changed_tick = unsafe { ticks_ptr.changed.deref() };
        let changed_by = unsafe { changed_by.deref() };
        Some(Ref::new(
            trait_object,
            added_tick,
            changed_tick,
            self.last_run,
            self.this_run,
            
            #[cfg(feature = "track_change_detection")]
            changed_by
        ))
    }
}

impl<'w, Trait: ?Sized + TraitQuery> IntoIterator for ReadTraits<'w, Trait> {
    type Item = Ref<'w, Trait>;
    type IntoIter = CombinedReadTraitsIter<'w, Trait>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let table = ReadTableTraitsIter {
            components: self.registry.table_components.iter(),
            meta: self.registry.table_meta.iter(),
            table: self.table,
            table_row: self.table_row,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        let sparse = ReadSparseTraitsIter {
            components: self.registry.sparse_components.iter(),
            meta: self.registry.sparse_meta.iter(),
            entity: self.table.entities()[self.table_row.as_usize()],
            sparse_sets: self.sparse_sets,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        table.chain(sparse)
    }
}

impl<'w, Trait: ?Sized + TraitQuery> IntoIterator for &ReadTraits<'w, Trait> {
    type Item = Ref<'w, Trait>;
    type IntoIter = CombinedReadTraitsIter<'w, Trait>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let table = ReadTableTraitsIter {
            components: self.registry.table_components.iter(),
            meta: self.registry.table_meta.iter(),
            table: self.table,
            table_row: self.table_row,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        let sparse = ReadSparseTraitsIter {
            components: self.registry.sparse_components.iter(),
            meta: self.registry.sparse_meta.iter(),
            entity: self.table.entities()[self.table_row.as_usize()],
            sparse_sets: self.sparse_sets,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        table.chain(sparse)
    }
}

impl<'w, Trait: ?Sized + TraitQuery> ReadTraits<'w, Trait> {
    /// Returns an iterator over the components implementing `Trait` for the current entity.
    pub fn iter(&self) -> CombinedReadTraitsIter<'w, Trait> {
        self.into_iter()
    }

    /// Returns an iterator over the components implementing `Trait` for the current entity
    /// that were added since the last time the system was run.
    pub fn iter_added(&self) -> impl Iterator<Item = Ref<'w, Trait>> {
        self.iter().filter(DetectChanges::is_added)
    }

    /// Returns an iterator over the components implementing `Trait` for the current entity
    /// whose values were changed since the last time the system was run.
    pub fn iter_changed(&self) -> impl Iterator<Item = Ref<'w, Trait>> {
        self.iter().filter(DetectChanges::is_changed)
    }
}
