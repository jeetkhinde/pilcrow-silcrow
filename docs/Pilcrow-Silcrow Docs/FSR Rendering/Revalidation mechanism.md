# Revalidation mechanism for LiveProp fields

Revalidation is timer-driven invalidation for eligible `LiveProp<T>` fields.

Important placement rule: `#[revalidate(N)]` goes on `LiveProp<T>` fields in
`Props`, not on fields in inline `Live`.

Precedence:

1. Field-level `#[revalidate(N)]`.
2. `[fsr] revalidate_seconds` in `Pilcrow.toml`.
3. Hardcoded 86400 seconds, or 24 hours.

This is how to use the revalidation proc-macro:

```rust
// Both share one timer - same route, same interval
#[revalidate(10)]
pub price: LiveProp<f64>,

#[revalidate(10)]
pub market_cap: LiveProp<f64>,

// Different interval = separate timer
#[revalidate(30)]
pub volume: LiveProp<u64>,
```

Fields with the same route and same interval share one timer. Different
intervals get separate timers.

Do not combine `#[revalidate(N)]` and `#[depends_on("key")]` on the same field.

See also [[../03 Rendering/Live Props and FSR]].
