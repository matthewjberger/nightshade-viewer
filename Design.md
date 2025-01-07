# Design

(rough notes, wip)

run.rs is entry point, has start() function and step() main loop

window.rs has winit supporting code 

logic for particular domain goes in related module

adds structs that are used either as `Component`s
or grouping return data / input args for `Queries`

structs top of file, followed by systems, queries

private implementation goes in with systems and queries, typically to support systems or help them reuse logic

---

Engine Architecture:

State:
- Components
- Resources

Logic:
- Systems
- Queries
- Commands
- 

----

Engine Context:

- Declared as an entity component system 
- Contains components that any entity can have
- Contains resources that contains all shared state

components:

- Custom plain data struct or a Newtype wrapping a plain data structs
  - Newtype used to wrap types declared outside the crate like u32, nalgebra_glm::Mat4, etc
- Required derives:
    - #[derive(Default, Debug, Copy, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
- Listed in the ecs component list in the `ecs!` macro.
- May have `impl {}` but *only* for a constructor `pub fn new(/*args*/) -> Self` 
  or for read-only operations. These are like immutable queries, but at the struct-level.

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

systems:

stateless free functions that take the `Context`
access / update resources and entity components

- stateless because systems take in the `Context` only, not `self`
- may use ecs query_entities() with a component mask set to find and mutate entities
- may iterate over the ecs Context entity tables and checks component masks
- suffixed with `system_` (arbitrary but helpful)

```rust
/// A basic system
pub fn camera_system(context: &mut crate::Context) {
    query_entities(context, ACTIVE | CAMERA | LOCAL_TRANSFORM)
        .into_iter()
        .for_each(|entity| {
            let (local_transform_matrix, _, right, up) = {
                // Ensure that the type and mask match
                // LOCAL_TRANSFORM is the mask for the LocalTransform type
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

Also, `query_entities` ensures that the entity has the components specified in the mask. 
Our mask was here was `LOCAL_TRANSFORM`, so `.unwrap()` is safe to use for getting the components
rather than early returns.

```rust
/// A basic system
pub fn camera_system(context: &mut crate::Context) {
    query_entities(context, ACTIVE | CAMERA | LOCAL_TRANSFORM)
        .into_iter()
        .for_each(|entity| {
                // Ensure that the type and mask match
                // LOCAL_TRANSFORM is the mask for the LocalTransform type
                let _local_transform = get_component_mut::<LocalTransform>(context, entity, LOCAL_TRANSFORM).unwrap();
                // ...
        })
```

queries:

stateless free functions that take the `Context` and arbitrary input args,
*read* resources and entity components, then return some result.

- Queries do *not* mutate the world
- prefixed with `query_` (arbitrary but helpful)


Example:

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

commands:

stateless free functions that take in a `Context` and arbitrary input args,
*mutate* resources and entity components, and do *not* return a result.

Commands are meant to be queuable, dispatchable single-shot world modifications.

events:

stateless free functions that take in a `Context` and an `winit::event::WindowEvent`
These update world resources in response to events from the main event loop.
This would update the resources for things like the mouse state, keyboard state, etc.

Notes:

- 2 unsafe lines are used for get_component and get_component_mut, and are verifiably safe as long as the component mask and provided type match

`Good` example (the unsafe line in get_component is valid): `get_component_mut::<LocalTransform>(context, entity, LOCAL_TRANSFORM)` because `LOCAL_TRANSFORM` is the mask for `LocalTransform`

`Bad` example (the unsafe line in get_component is valid): `get_component_mut::<Name>(context, entity, LOCAL_TRANSFORM)` because `Name` is not the correct type to use with `LOCAL_TRANSFORM`

## Help:

- I want to X

## Ecs Explanation

TODO

### Philosophy

Data Oriented Design:

- [Mike Acton - Data Oriented Design and C++](https://www.youtube.com/watch?v=rX0ItVEVjHc)
- [Games from Within - Data Oriented Design](https://gamesfromwithin.com/data-oriented-design)
- [The Data Oriented Design Book](https://www.dataorienteddesign.com/dodmain/dodmain.html)

Essentially, we avoid adding abstractions to prevent the software architecture becoming a rube-goldberg machine of discrete logical abstractions. Systems can become difficult to reason about with traditional object oriented programming, even when done correctly.

Instead of classes from object oriented programming, state is instead just plain data structures that may be composed, and stateless free functions that operate on them

Nightshade uses a custom entity component system that allows access to all entities, their components, and all resources (state not owned by entities). It is referred to as `Context`, and it contains all state in Nightshade.

For performance, the ecs is using archetype component tables. The design is a ~500 line rust macro. I made an open-source crate named `[freecs](https://crates.io/crates/freecs)` for a reusable implementation. It is inlined in Nightshade, with not changes made.

All state is declared in components and resources plain data structs
 - Components are state owned by an entity
 - Resources are shared state
 - Both Components and Resources are available to systems, queries, commands, and events

All logic is declared in systems, queries, commands, and events which all have their own helper logic.
 - All logic is in stateless free functions


Examples:
    Ecs:

    Components:

    Resources:

    Systems:
    
    Queries:

    Events:

    Commands:



--

systems don't have to be public, they can be supporting another system