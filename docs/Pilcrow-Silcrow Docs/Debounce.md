# Debounce

`#[debounce(30)]` sets a debounce window, in seconds, for FSR live patches.
When a live field is invalidated many times quickly, debounce is meant to wait for
the changes to settle and then send one patch with the latest value instead of
many small patches.

Placement rule: `#[debounce(N)]` belongs on the inline `Live` struct or on
individual `LiveProp<T>` fields inside that struct.

Current implementation note: routekit parses debounce and persists it in FSR
metadata, but the embedded watcher still patches stale slots immediately and does
not enforce the delay yet.

```rust
use pilcrow::live::*;

#[debounce(30)] // default for every LiveProp field below
pub struct Live {
    pub status: LiveProp<String>,

    #[debounce(5)] // override: priority can update sooner
    pub priority: LiveProp<String>,
}
```

In this example, repeated `status` invalidations are grouped into one latest-value
patch after the 30-second window. `priority` uses a shorter 5-second window.

See also [[03 Rendering/Live Props and FSR]].
