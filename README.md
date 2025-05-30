[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/JoJoJet/bevy-trait-query#license)
[![Crates.io](https://img.shields.io/crates/v/bevy-trait-query.svg)](https://crates.io/crates/bevy-trait-query)
[![Downloads](https://img.shields.io/crates/d/bevy-trait-query.svg)](https://crates.io/crates/bevy-trait-query)
[![Docs](https://docs.rs/bevy-trait-query/badge.svg)](https://docs.rs/bevy_trait_query/latest/bevy_trait_query/)
[![CI](https://github.com/JoJoJet/bevy-trait-query/workflows/CI/badge.svg)](https://github.com/JoJoJet/bevy-trait-query/actions)

# bevy-trait-query

An implementation of trait queries for the bevy game engine.

Before using this crate, you should be familiar with bevy: https://bevyengine.org/.

| Bevy Version | [Crate Version](CHANGELOG.md) |
|--------------|---------------|
| Preview      | Main branch   |
| 0.15         | 0.7           |
| 0.14         | 0.6           |
| 0.13         | 0.5           |
| 0.12         | 0.4           |
| 0.11         | 0.3           |
| 0.10         | 0.2           |
| 0.9          | 0.1           |
| 0.8          | 0.0.3         |

## Note on reliability

While this crate has seen some use in the world with no issues yet,
it is still quite new and experimental. Use with caution (and miri!).

If you find a bug, please [open an issue](https://github.com/JoJoJet/bevy-trait-query/issues).

## Overview

<!-- cargo-rdme start -->

Let's say you have a trait that you want to implement for some of your components.

```rust
/// Components that display a message when hovered.
pub trait Tooltip {
    /// Text displayed when hovering over an entity with this trait.
    fn tooltip(&self) -> &str;
}
```

In order to be useful within bevy, you'll want to be able to query for this trait.

```rust

// Just add this attribute...
#[bevy_trait_query::queryable]
pub trait Tooltip {
    fn tooltip(&self) -> &str;
}

// ...and now you can use your trait in queries.
fn show_tooltips_system(
    tooltips: Query<&dyn Tooltip>,
    // ...
) {
    // ...
}
```

Since Rust unfortunately lacks any kind of reflection, it is necessary to register each
component with the trait when the app gets built.

```rust
#[derive(Component)]
struct Player(String);

#[derive(Component)]
enum Villager {
    Farmer,
    // ...
}

#[derive(Component)]
struct Monster;

/* ...trait implementations omitted for brevity... */

struct TooltipPlugin;

impl Plugin for TooltipPlugin {
    fn build(&self, app: &mut App) {
        // We must import this trait in order to register our components.
        // If we don't register them, they will be invisible to the game engine.
        use bevy_trait_query::RegisterExt;

        app
            .register_component_as::<dyn Tooltip, Player>()
            .register_component_as::<dyn Tooltip, Villager>()
            .register_component_as::<dyn Tooltip, Monster>()
            .add_systems(Update, show_tooltips);
    }
}
```

Unlike queries for concrete types, it's possible for an entity to have multiple components
that match a trait query.

```rust

fn show_tooltips(
    tooltips: Query<&dyn Tooltip>,
    // ...
) {
    // Iterate over each entity that has tooltips.
    for entity_tooltips in &tooltips {
        // Iterate over each component implementing `Tooltip` for the current entity.
        for tooltip in entity_tooltips {
            println!("Tooltip: {}", tooltip.tooltip());
        }
    }

    // If you instead just want to iterate over all tooltips, you can do:
    for tooltip in tooltips.iter().flatten() {
        println!("Tooltip: {}", tooltip.tooltip());
    }
}
```

Alternatively, if you expect to only have component implementing the trait for each entity,
you can use the filter [`One`](https://docs.rs/bevy-trait-query/latest/bevy_trait_query/one/struct.One.html). This has significantly better performance than iterating
over all trait impls.

```rust
use bevy_trait_query::One;

fn show_tooltips(
    tooltips: Query<One<&dyn Tooltip>>,
    // ...
) {
    for tooltip in &tooltips {
        println!("Tooltip: {}", tooltip.tooltip());
    }
}
```

Trait queries support basic change detection filtration.

- queries requesting shared access yield [`ReadTraits`](https://docs.rs/bevy-trait-query/latest/bevy_trait_query/all/struct.ReadTraits.html) which is
  similar to [`Ref`](https://docs.rs/bevy/latest/bevy/ecs/change_detection/struct.Ref.html)
- queries requesting exclusive access yield [`WriteTraits`](https://docs.rs/bevy-trait-query/latest/bevy_trait_query/all/struct.WriteTraits.html) which is
  similar to [`Mut`](https://docs.rs/bevy/latest/bevy/ecs/change_detection/struct.Mut.html)

To get all the components that implement the target trait, and have also changed in some way
since the last tick, you can:
```rust
fn show_tooltips(
    tooltips_query: Query<All<&dyn Tooltip>>
    // tooltips_query: Query<&dyn Tooltip>  // <-- equivalent to line above
    // ...
) {
    // Iterate over all entities with at least one component implementing `Tooltip`
    for entity_tooltips in &tooltips_query {
        // Iterate over each component for the current entity that changed since the last time the system was run.
        for tooltip in entity_tooltips.iter_changed() {
            println!("Changed Tooltip: {}", tooltip.tooltip());
        }
    }
}
```

Similar to [`iter_changed`](https://docs.rs/bevy-trait-query/latest/bevy_trait_query/all/struct.ReadTraits.html), we have [`iter_added`](https://docs.rs/bevy-trait-query/latest/bevy_trait_query/all/struct.ReadTraits.html)
to detect entities which have had a trait-implementing component added since the last tick.

If you know you have only one component that implements the target trait,
you can use [`OneAdded`](https://docs.rs/bevy-trait-query/latest/bevy_trait_query/one/struct.OneAdded.html) or [`OneChanged`](https://docs.rs/bevy-trait-query/latest/bevy_trait_query/one/struct.OneChanged.html) which behave more like the typical
`bevy` [`Added`](https://docs.rs/bevy/latest/bevy/ecs/query/struct.Added.html)/[`Changed`](https://docs.rs/bevy/latest/bevy/ecs/query/struct.Changed.html) filters:
```rust
fn show_tooltips(
    tooltips_query: Query<One<&dyn Tooltip>, OneChanged<dyn Tooltip>>
    // ...
) {
    // Iterate over each entity that has one tooltip implementing component that has also changed
    for tooltip in &tooltips_query {
        println!("Changed Tooltip: {}", tooltip.tooltip());
    }
}
```
Note in the above example how [`OneChanged`](https://docs.rs/bevy-trait-query/latest/bevy_trait_query/one/struct.OneChanged.html) does *not* take a reference to the trait object!

### Performance

The performance of trait queries is quite competitive. Here are some benchmarks for simple cases:

|                   | Concrete type  | [`One<dyn Trait>`](https://docs.rs/bevy-trait-query/latest/bevy_trait_query/one/struct.One.html)    | [`All<dyn Trait>`](https://docs.rs/bevy-trait-query/latest/bevy_trait_query/all/struct.All.html) |
|-------------------|----------------|---------------------|-------------------|
| 1 match           | 8.395 µs       | 28.174 µs           | 81.027 µs         |
| 2 matches         | 8.473 µs       | -                   | 106.47 µs         |
| 1-2 matches       | -              | 14.619 µs           | 92.876 µs         |

<!-- cargo-rdme end -->

# License

[MIT](LICENSE-MIT) or [APACHE-2.0](LICENSE-APACHE)
