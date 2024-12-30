//! This module declares a macro that generates
//! an archetypal ecs data structure and free functions for data access.
#[macro_export]
macro_rules! ecs {
    (
        $context:ident {
            $($name:ident: $type:ty => $mask:ident),* $(,)?
        }
        $resources:ident {
            $($(#[$attr:meta])*  $resource_name:ident: $resource_type:ty),* $(,)?
        }
    ) => {

        /// Component masks
        #[repr(u32)]
        #[allow(clippy::upper_case_acronyms)]
        #[allow(non_camel_case_types)]
        #[allow(dead_code)]
        pub enum Component {
            None,
            $($mask,)*
            All,
        }

        #[allow(dead_code)]
        pub const NONE: u32 = Component::None as u32;

        $(
            #[allow(dead_code)]
            pub const $mask: u32 = 1 << (Component::$mask as u32);
        )*

        #[allow(dead_code)]
        pub const ALL: u32 = Component::All as u32;

        pub const COMPONENT_COUNT: usize = { Component::All as usize };

        /// Entity ID, an index into storage and a generation counter to prevent stale references
        #[derive(Default, Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
        pub struct EntityId {
            pub id: u32,
            pub generation: u32,
        }

        impl std::fmt::Display for EntityId {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let Self { id, generation } = self;
                write!(f, "Id: {id} - Generation: {generation}")
            }
        }

        // Handles allocation and reuse of entity IDs
        #[derive(Default, serde::Serialize, serde::Deserialize)]
        pub struct EntityAllocator {
            next_id: u32,
            free_ids: Vec<(u32, u32)>, // (id, next_generation)
        }

        #[derive(Copy, Clone, Default, serde::Serialize, serde::Deserialize)]
        struct EntityLocation {
            generation: u32,
            table_index: u16,
            array_index: u16,
            allocated: bool,
        }

        /// Entity location cache for quick access
        #[derive(Default, serde::Serialize, serde::Deserialize)]
        pub struct EntityLocations {
            locations: Vec<EntityLocation>,
        }

        /// A collection of component tables and resources
        #[derive(Default, serde::Serialize, serde::Deserialize)]
        pub struct $context {
            pub entity_locations: EntityLocations,
            pub tables: Vec<ComponentArrays>,
            pub allocator: EntityAllocator,
            pub resources: $resources,
            table_edges: Vec<TableEdges>,
            pending_despawns: Vec<EntityId>,
        }

        /// Resources
        #[derive(Default, serde::Serialize, serde::Deserialize)]
        pub struct $resources {
            $($(#[$attr])* pub $resource_name: $resource_type,)*
        }

        /// Component Table
        #[derive(Default, serde::Serialize, serde::Deserialize)]
        pub struct ComponentArrays {
            $(pub $name: Vec<$type>,)*
            pub entity_indices: Vec<EntityId>,
            pub mask: u32,
        }

        #[derive(Copy, Clone, Default, serde::Serialize, serde::Deserialize)]
        struct TableEdges {
            add_edges: [Option<usize>; COMPONENT_COUNT],
            remove_edges: [Option<usize>; COMPONENT_COUNT],
        }

        fn get_component_index(mask: u32) -> Option<usize> {
            match mask {
                $($mask => Some(Component::$mask as _),)*
                _ => None,
            }
        }

        #[allow(dead_code)]
        /// Spawn a batch of new entities with the same component mask
        pub fn spawn_entities(context: &mut $context, mask: u32, count: usize) -> Vec<EntityId> {
            let mut entities = Vec::with_capacity(count);
            let table_index = get_or_create_table(context, mask);

            context.tables[table_index].entity_indices.reserve(count);

            // Reserve space in components
            $(
                if mask & $mask != 0 {
                    context.tables[table_index].$name.reserve(count);
                }
            )*

            for _ in 0..count {
                let entity = create_entity(context);
                add_to_table(
                    &mut context.tables[table_index],
                    entity,
                    (
                        $(
                        if mask & $mask != 0 {
                            Some(<$type>::default())
                        } else {
                            None
                        },
                        )*
                    ),
                );
                entities.push(entity);
                insert_location(
                    &mut context.entity_locations,
                    entity,
                    (table_index, context.tables[table_index].entity_indices.len() - 1),
                );
            }

            entities
        }

        #[allow(dead_code)]
        /// Query for all entities that match the component mask
        pub fn query_entities(context: &$context, mask: u32) -> Vec<EntityId> {
            let total_capacity = context
                .tables
                .iter()
                .filter(|table| table.mask & mask == mask)
                .map(|table| table.entity_indices.len())
                .sum();

            let mut result = Vec::with_capacity(total_capacity);
            for table in &context.tables {
                if table.mask & mask == mask {
                    // Only include allocated entities
                    result.extend(
                        table
                            .entity_indices
                            .iter()
                            .copied()
                            .filter(|&e| context.entity_locations.locations[e.id as usize].allocated),
                    );
                }
            }
            result
        }

        #[allow(dead_code)]
        /// Query for the first entity that matches the component mask
        /// Returns as soon as a match is found, instead of running for all entities
        pub fn query_first_entity(context: &$context, mask: u32) -> Option<EntityId> {
            for table in &context.tables {
                if !$crate::has_components!(table, mask) {
                    continue;
                }
                let indices = table
                    .entity_indices
                    .iter()
                    .copied()
                    .filter(|&e| context.entity_locations.locations[e.id as usize].allocated)
                    .collect::<Vec<_>>();
                if let Some(entity) = indices.first() {
                    return Some(*entity);
                }
            }
            None
        }

        #[allow(dead_code)]
        /// Get a specific component for an entity
        pub fn get_component<T: 'static>(context: &$context, entity: EntityId, mask: u32) -> Option<&T> {
           let (table_index, array_index) = get_location(&context.entity_locations, entity)?;

           // Early return if entity is despawned
           if !context.entity_locations.locations[entity.id as usize].allocated {
               return None;
           }

           let table = &context.tables[table_index];

           if table.mask & mask == 0 {
               return None;
           }

           $(
               if mask == $mask && std::any::TypeId::of::<T>() == std::any::TypeId::of::<$type>() {
                   // SAFETY: This operation is safe because:
                   // 1. We verify the component type T exactly matches $type via TypeId
                   // 2. We confirm the table contains this component via mask check
                   // 3. array_index is valid from get_location bounds check
                   // 4. The reference is valid for the lifetime of the return value
                   //    because it's tied to the table reference lifetime
                   // 5. No mutable aliases can exist during the shared borrow
                   // 6. The type cast maintains proper alignment as types are identical
                   return Some(unsafe { &*(&table.$name[array_index] as *const $type as *const T) });
               }
           )*

           None
        }

        #[allow(dead_code)]
        /// Get a mutable reference to a specific component for an entity
        pub fn get_component_mut<T: 'static>(context: &mut $context, entity: EntityId, mask: u32) -> Option<&mut T> {
            let (table_index, array_index) = get_location(&context.entity_locations, entity)?;
            let table = &mut context.tables[table_index];
            if table.mask & mask == 0 {
                return None;
            }

            $(
                if mask == $mask && std::any::TypeId::of::<T>() == std::any::TypeId::of::<$type>() {
                    // SAFETY: This operation is safe because:
                    // 1. We verify the component type T exactly matches $type via TypeId
                    // 2. We confirm the table contains this component via mask check
                    // 3. array_index is valid from get_location bounds check
                    // 4. We have exclusive access through the mutable borrow
                    // 5. The borrow checker ensures no other references exist
                    // 6. The pointer cast is valid as we verified the types are identical
                    // 7. Proper alignment is maintained as the types are the same
                    return Some(unsafe { &mut *(&mut table.$name[array_index] as *mut $type as *mut T) });
                }
            )*

            None
        }

        #[allow(dead_code)]
        /// Despawn a batch of entities
        pub fn despawn_entities(context: &mut $context, entities: &[EntityId]) -> Vec<EntityId> {
            let mut despawned = Vec::with_capacity(entities.len());
            let mut tables_to_update = Vec::new();

            // First pass: mark entities as despawned and collect their table locations
            for &entity in entities {
                let id = entity.id as usize;
                if id < context.entity_locations.locations.len() {
                    let loc = &mut context.entity_locations.locations[id];
                    if loc.allocated && loc.generation == entity.generation {
                        // Get table info before marking as despawned
                        let table_idx = loc.table_index as usize;
                        let array_idx = loc.array_index as usize;

                        // Mark as despawned
                        loc.allocated = false;
                        loc.generation = loc.generation.wrapping_add(1);
                        context.allocator.free_ids.push((entity.id, loc.generation));

                        // Collect table info for updates
                        tables_to_update.push((table_idx, array_idx));
                        despawned.push(entity);
                    }
                }
            }

            // Second pass: remove entities from tables in reverse order to maintain indices
            for (table_idx, array_idx) in tables_to_update.into_iter().rev() {
                if table_idx >= context.tables.len() {
                    continue;
                }

                let table = &mut context.tables[table_idx];
                let last_idx = table.entity_indices.len() - 1;

                // If we're not removing the last element, update the moved entity's location
                if array_idx < last_idx {
                    let moved_entity = table.entity_indices[last_idx];
                    if let Some(loc) = context.entity_locations.locations.get_mut(moved_entity.id as usize) {
                        if loc.allocated {
                            loc.array_index = array_idx as u16;
                        }
                    }
                }

                // Remove the entity's components
                $(
                    if table.mask & $mask != 0 {
                        table.$name.swap_remove(array_idx);
                    }
                )*
                table.entity_indices.swap_remove(array_idx);
            }

            despawned
        }

        #[allow(dead_code)]
        /// Add components to an entity
        pub fn add_components(context: &mut $context, entity: EntityId, mask: u32) -> bool {
            if let Some((table_index, array_index)) = get_location(&context.entity_locations, entity) {
                let current_mask = context.tables[table_index].mask;
                if current_mask & mask == mask {
                    return true;
                }

                let target_table = if mask.count_ones() == 1 {
                    get_component_index(mask).and_then(|idx| context.table_edges[table_index].add_edges[idx])
                } else {
                    None
                };

                let new_table_index =
                    target_table.unwrap_or_else(|| get_or_create_table(context, current_mask | mask));

                move_entity(context, entity, table_index, array_index, new_table_index);
                true
            } else {
                false
            }
        }

        #[allow(dead_code)]
        /// Remove components from an entity
        pub fn remove_components(context: &mut $context, entity: EntityId, mask: u32) -> bool {
            if let Some((table_index, array_index)) = get_location(&context.entity_locations, entity) {
                let current_mask = context.tables[table_index].mask;
                if current_mask & mask == 0 {
                    return true;
                }

                let target_table = if mask.count_ones() == 1 {
                    get_component_index(mask)
                        .and_then(|idx| context.table_edges[table_index].remove_edges[idx])
                } else {
                    None
                };

                let new_table_index =
                    target_table.unwrap_or_else(|| get_or_create_table(context, current_mask & !mask));

                move_entity(context, entity, table_index, array_index, new_table_index);
                true
            } else {
                false
            }
        }

        #[allow(dead_code)]
        /// Get the current component mask for an entity
        pub fn component_mask(context: &$context, entity: EntityId) -> Option<u32> {
            get_location(&context.entity_locations, entity)
                .map(|(table_index, _)| context.tables[table_index].mask)
        }

        fn remove_from_table(arrays: &mut ComponentArrays, index: usize) -> Option<EntityId> {
            let last_index = arrays.entity_indices.len() - 1;
            let mut swapped_entity = None;

            if index < last_index {
                swapped_entity = Some(arrays.entity_indices[last_index]);
            }

            $(
                if arrays.mask & $mask != 0 {
                    arrays.$name.swap_remove(index);
                }
            )*
            arrays.entity_indices.swap_remove(index);

            swapped_entity
        }

        fn move_entity(
            context: &mut $context,
            entity: EntityId,
            from_table: usize,
            from_index: usize,
            to_table: usize,
        ) {
            let components = get_components(&context.tables[from_table], from_index);
            add_to_table(&mut context.tables[to_table], entity, components);
            let new_index = context.tables[to_table].entity_indices.len() - 1;
            insert_location(&mut context.entity_locations, entity, (to_table, new_index));

            if let Some(swapped) = remove_from_table(&mut context.tables[from_table], from_index) {
                insert_location(
                    &mut context.entity_locations,
                    swapped,
                    (from_table, from_index),
                );
            }
        }

        fn get_components(
            arrays: &ComponentArrays,
            index: usize,
        ) -> (  $(Option<$type>,)* ) {
            (
                $(
                    if arrays.mask & $mask != 0 {
                        Some(arrays.$name[index].clone())
                    } else {
                        None
                    },
                )*
            )
        }

        fn get_location(locations: &EntityLocations, entity: EntityId) -> Option<(usize, usize)> {
            let id = entity.id as usize;
            if id >= locations.locations.len() {
                return None;
            }

            let location = &locations.locations[id];
            // Only return location if entity is allocated AND generation matches
            if !location.allocated || location.generation != entity.generation {
                return None;
            }

            Some((location.table_index as usize, location.array_index as usize))        }

        fn insert_location(
            locations: &mut EntityLocations,
            entity: EntityId,
            location: (usize, usize),
        ) {
            let id = entity.id as usize;
            if id >= locations.locations.len() {
                locations
                    .locations
                    .resize(id + 1, EntityLocation::default());
            }

            locations.locations[id] = EntityLocation {
                generation: entity.generation,
                table_index: location.0 as u16,
                array_index: location.1 as u16,
                allocated: true,
            };
        }

        fn create_entity(context: &mut $context) -> EntityId {
            if let Some((id, next_gen)) = context.allocator.free_ids.pop() {
                let id_usize = id as usize;
                if id_usize >= context.entity_locations.locations.len() {
                    context.entity_locations.locations.resize(
                        (context.entity_locations.locations.len() * 2).max(64),
                        EntityLocation::default(),
                    );
                }
                context.entity_locations.locations[id_usize].generation = next_gen;
                EntityId {
                    id,
                    generation: next_gen,
                }
            } else {
                let id = context.allocator.next_id;
                context.allocator.next_id += 1;
                let id_usize = id as usize;
                if id_usize >= context.entity_locations.locations.len() {
                    context.entity_locations.locations.resize(
                        (context.entity_locations.locations.len() * 2).max(64),
                        EntityLocation::default(),
                    );
                }
                EntityId { id, generation: 0 }
            }
        }

        fn add_to_table(
            arrays: &mut ComponentArrays,
            entity: EntityId,
            components: ( $(Option<$type>,)* ),
        ) {
            let ($($name,)*) = components;
            $(
                if arrays.mask & $mask != 0 {
                    arrays
                        .$name
                        .push($name.unwrap_or_default());
                }
            )*
            arrays.entity_indices.push(entity);
        }

        fn get_or_create_table(context: &mut $context, mask: u32) -> usize {
            if let Some((index, _)) = context
                .tables
                .iter()
                .enumerate()
                .find(|(_, t)| t.mask == mask)
            {
                return index;
            }

            let table_index = context.tables.len();
            context.tables.push(ComponentArrays {
                mask,
                ..Default::default()
            });
            context.table_edges.push(TableEdges::default());

            // Remove table registry updates and only update edges
            for comp_mask in [
                $($mask,)*
            ] {
                if let Some(comp_idx) = get_component_index(comp_mask) {
                    for (idx, table) in context.tables.iter().enumerate() {
                        if table.mask | comp_mask == mask {
                            context.table_edges[idx].add_edges[comp_idx] = Some(table_index);
                        }
                        if table.mask & !comp_mask == mask {
                            context.table_edges[idx].remove_edges[comp_idx] = Some(table_index);
                        }
                    }
                }
            }

            table_index
        }
    };
}

#[macro_export]
macro_rules! has_components {
    ($table:expr, $mask:expr) => {
        $table.mask & $mask == $mask
    };
}
