# Nightshade Engine Design

## Architecture Overview

Nightshade follows Data-Oriented Design principles rather than traditional OOP. The core architecture consists of:

### Entry Points & Core Flow

- `run.rs` is entry point, has `start()` function and `step()` main loop
- `window.rs` has winit supporting code 
- Logic for particular domain goes in related module

### Engine Context
k
- Declared as an entity component system 
- Contains components that any entity can have
- Contains resources that contains all shared state

State:

- Components
- Resources

Logic:

- Systems
- Queries
- Commands
- Events

## Component Design

Components are:

- Custom plain data struct or a Newtype wrapping plain data structs
  - Newtype used to wrap types declared outside the crate like u32, nalgebra_glm::Mat4, etc
- Required derives:
  - `#[derive(Default, Debug, Copy, Clone, serde::Serialize, serde::Deserialize, PartialEq)]`
- Listed in the ecs component list in the `ecs!` macro
- May have `impl {}` but *only* for:
  - Constructor `pub fn new(/*args*/) -> Self`
  - Read-only operations (like immutable queries at struct-level)

Example Components:

```rust
// A newtype wrapping a type we didn't create
#[derive(Debug, Copy, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Name(pub String);

// A plain data struct
#[derive(Debug, Copy, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct LocalTransform {
    pub translation: nalgebra_glm::Vec3,
    pub rotation: nalgebra_glm::Quat,
    pub scale: nalgebra_glm::Vec3,
}
```

## Systems

Systems are stateless free functions that take the `Context` and access/update resources and entity components.

Characteristics:

- Stateless (take `Context` only, not `self`)
- May use ecs query_entities() with component mask
- May iterate over Context entity tables and check component masks
- Suffixed with `_system` (arbitrary but helpful)

Example System:

```rust
/// A basic system
pub fn camera_system(context: &mut crate::Context) {
    query_entities(context, ACTIVE | CAMERA | LOCAL_TRANSFORM)
        .into_iter()
        .for_each(|entity| {
            let (local_transform_matrix, _, right, up) = {
                let Some(local_transform) =
                    get_component_mut::<LocalTransform>(context, entity, LOCAL_TRANSFORM)
                else {
                    return;
                };
                let local_transform_matrix = local_transform.as_matrix();
                // ...
        });
}
```

Note: When using `query_entities`, component access is safe:

```rust
pub fn camera_system(context: &mut crate::Context) {
    query_entities(context, ACTIVE | CAMERA | LOCAL_TRANSFORM)
        .into_iter()
        .for_each(|entity| {
                let _local_transform = get_component_mut::<LocalTransform>(context, entity, LOCAL_TRANSFORM).unwrap();
                // Safe to unwrap as query_entities ensures components exist
        })
```

## Queries

Queries are stateless functions that:

- Take `Context` and input args
- *Read* resources and entity components
- Return some result
- Do *not* mutate the world
- Prefixed with `query_` (arbitrary but helpful)

Example Query:

```rust
/// Queries for root nodes by looking for entities that do not have a Parent component
pub fn query_root_nodes(context: &Context) -> Vec<EntityId> {
    let mut root_entities: Vec<EntityId> = context
        .tables
        .iter()
        .filter_map(|table| {
            if crate::has_components!(table, PARENT) {
                return None;
            }
            Some(table.entity_indices.to_vec())
        })
        .flatten()
        .collect();
    root_entities.dedup();
    root_entities
}
```

## Commands & Events

Commands:

- Stateless functions taking `Context` and args
- *Mutate* resources and components
- Do *not* return results
- Meant to be queuable, dispatchable single-shot world modifications

Events:

- Take `Context` and `winit::event::WindowEvent`
- Update world resources in response to window events
- Handle mouse state, keyboard state, etc.

## Safety Notes

The ECS uses only 2 unsafe lines for get_component and get_component_mut, which are safe when:

- Component mask and provided type match

Good example (safe):

```rust
get_component_mut::<LocalTransform>(context, entity, LOCAL_TRANSFORM)
```

Bad example (unsafe):

```rust
get_component_mut::<Name>(context, entity, LOCAL_TRANSFORM)
```

## Data-Oriented Design Philosophy

Key resources:

- [Mike Acton - Data Oriented Design and C++](https://www.youtube.com/watch?v=rX0ItVEVjHc)
- [Games from Within - Data Oriented Design](https://gamesfromwithin.com/data-oriented-design)
- [The Data Oriented Design Book](https://www.dataorienteddesign.com/dodmain/dodmain.html)

Core principles:

- Avoid unnecessary abstractions
- State as plain data structures
- Logic as stateless functions
- Performance through data layout
- Archetype component tables for ECS
- All state in Components and Resources
- All logic in Systems, Queries, Commands, and Events

The custom ECS implementation is based on the [freecs](https://crates.io/crates/freecs) crate, inlined with no modifications.

---

This is a living document and will be updated as the engine evolves