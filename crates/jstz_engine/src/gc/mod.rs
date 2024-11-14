//! # Garbage Collection in SpiderMonkey
//!
//! This module implements memory-safe abstractions on SpiderMonkey's garbage collection (GC) system,
//! designed for efficient memory management in the JavaScript engine. The GC is a mark-sweep collector
//! with features such as incremental marking, generational collection, and compaction.
//!
//! # Key Concepts
//! - **Cells**: The atomic unit of memory managed by the GC. All GC-allocated objects, such as `JsObject`, derive from `Cell`.
//! - **Compartments & Zones**: Memory is organized into compartments (for security and isolation) and zones (GC boundaries).
//!
//! # Features
//! - **Incremental GC**: Reduces pause times by interleaving marking work with JavaScript execution.
//! - **Write and Read Barriers**: Ensure correctness during incremental GC by maintaining object reachability.
//! - **Generational GC**: Optimizes for short-lived objects, separating them from long-lived ones.
//!
//! # Implementation Notes
//! - Write barriers, triggered during pointer updates, and read barriers for weak references, prevent GC hazards.
//! - Sweeping and additional GC phases like compaction are integrated into the collection process.
//!
//! For further details, see the [GC Implementation Guide](https://udn.realityripple.com/docs/Mozilla/Projects/SpiderMonkey/Internals/Garbage_collection).

mod ptr;
