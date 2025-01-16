# Nightshade Engine Design

## Overview

The engine is organized into these core concepts:

```
Engine
├── State
│   ├── Components (Entity Data)
│   │   ├── Pure data structures
│   │   └── No behavior
│   │
│   └── Resources (Global Data)
│       ├── Shared state
│       └── Engine/game configuration
│
└── Behavior
    ├── Systems (Frame Updates)
    │   ├── Modify world state
    │   └── Run each frame
    │
    ├── Queries (Data Access)
    │   ├── Read-only operations
    │   └── Return world data
    │
    └── Commands (World Mutations)
        ├── Create/destroy entities
        └── Modify component layouts
```

The rest of this document details each of these concepts and how they work together.

## Component Design

Components are pure data structures with no behavior. They come in two forms:

1. **Newtype Components** - Wrapping external types:

```rust
#[derive(Debug, Clone)]
pub struct Name(pub String);
```

2. **Data Structure Components** - Custom data layouts:

```rust
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct LocalTransform {
    pub translation: nalgebra_glm::Vec3,
    pub rotation: nalgebra_glm::Quat,
    pub scale: nalgebra_glm::Vec3,
}
```

## Systems

Systems are just functions that operate on Context:

```rust
pub fn update_global_transforms_system(context: &mut Context) {
    query_entities(context, LOCAL_TRANSFORM | GLOBAL_TRANSFORM)
        .into_iter()
        .for_each(|entity| {
            let new_global_transform = query_global_transform(context, entity);
            let global_transform =
                get_component_mut::<GlobalTransform>(context, entity, GLOBAL_TRANSFORM).unwrap();
            *global_transform = GlobalTransform(new_global_transform);
        });
}
```

Systems can:

- Query for entities with specific components
- Read/write components safely using component masks
- Access shared resources
- Modify world state through the Context

There is no trait-based system or dependency management - just plain functions that take `&mut Context`.

## Commands

Commands come in two forms:

1. **Command Enums** - For queueable operations:

```rust
pub enum Command {
    Entity(EntityCommand),
}

pub enum EntityCommand {
    SpawnCube {
        position: nalgebra_glm::Vec3,
        size: f32,
        name: String,
    },
    SpawnCamera {
        position: nalgebra_glm::Vec3,
        name: String,
    },
}
```

2. **Direct Command Functions** - For immediate operations:

```rust
pub fn spawn_main_camera(context: &mut Context) -> EntityId {
    let entity = spawn_entities(context, CAMERA | LOCAL_TRANSFORM | GLOBAL_TRANSFORM | NAME, 1)[0];
    
    if let Some(name) = get_component_mut::<Name>(context, entity, NAME) {
        *name = Name("Main Camera".to_string());
    }

    context.resources.active_camera_entity = Some(entity);
    entity
}
```

Commands:

- Can be either immediate functions or queueable data
- Handle world mutations in a structured way
- Keep entity creation code organized by domain
- Provide clear patterns for world modification

## Queries

Queries are read-only operations that extract data from the world:

```rust
pub fn query_root_nodes(context: &Context) -> Vec<EntityId> {
    context.tables
        .iter()
        .filter_map(|table| {
            if has_components!(table, PARENT) {
                return None;
            }
            Some(table.entity_indices.to_vec())
        })
        .flatten()
        .collect()
}
```

## Component Access Safety

The ECS provides safe component access through component masks:

```rust
// Safe - component type matches mask
get_component_mut::<LocalTransform>(context, entity, LOCAL_TRANSFORM)

// Unsafe - component type doesn't match mask
get_component_mut::<Name>(context, entity, LOCAL_TRANSFORM) // DON'T DO THIS
```

## Naming Conventions

Functions follow these naming patterns to indicate their purpose:

- **Systems**: Suffix with `_system`

  ```rust
  pub fn update_global_transforms_system(context: &mut Context)
  ```

- **Queries**: Prefix with `query_`

  ```rust
  pub fn query_root_nodes(context: &Context) -> Vec<EntityId>
  ```

- **Commands**: Suffix with `_command`

  ```rust
  pub fn spawn_main_camera_command(context: &mut Context) -> EntityId
  pub fn create_scene_command(context: &mut Context) -> EntityId
  ```

## API Categories

The engine's functionality is organized into these key categories:

| Category | Purpose | Pattern | Example |
|----------|---------|---------|---------|
| Systems | Frame-by-frame world updates | Functions that modify world state | Update transforms, handle input |
| Queries | Data inspection | Pure functions that read state | Find entities, get hierarchies |
| Commands | World modification | Functions that make structural changes | Create entities, modify scenes |
| Components | Core data | Plain data structures | Transform, Camera, Name |
| Resources | Global state | Shared data accessible to all systems | Window, Input, Graphics |

### Patterns

Each category follows specific patterns:

**Systems**

- Take Context, modify state
- Run every frame
- Handle one specific aspect of the game/engine
- Examples: physics, input, rendering

**Queries**

- Take Context, return data
- Never modify state
- Answer specific questions about the world
- Examples: scene hierarchy, spatial queries

**Commands**

- Take Context, modify world structure
- Create/destroy entities
- Modify component layouts
- Examples: spawning, scene loading

**Components**

- Pure data, no behavior
- Represent one aspect of an entity
- Can be added/removed dynamically
- Examples: position, rendering data

**Resources**

- Global singleton data
- Shared across all systems
- Core engine/game state
- Examples: time, input state, rendering context

## Data-Oriented Design Philosophy

Core principles demonstrated in the codebase:

1. **State as Data**

- All game state lives in Components and Resources
- Components are pure data structures
- No behavior methods on data types

2. **Logic as Functions**

- Systems are pure functions operating on Context
- Queries are read-only functions
- Commands describe world mutations

3. **Performance Through Data Layout**

- Archetypal storage for components
- Contiguous memory in component tables
- Minimal indirection

4. **Clear Boundaries**

- State: Components, Resources
- Behavior: Systems, Queries
- Mutations: Commands
- Queries: Read-only operations

The custom ECS implementation is based on the [freecs](https://crates.io/crates/freecs) crate, inlined with no modifications.

---

This is a living document and will be updated as the engine evolves.
