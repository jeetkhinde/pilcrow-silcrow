
# # Revalidation mechanism for LiveProp fields.

- Revalidation happens on the field level.
- Default hardcoded revalidation happens every 24 hours automatically if global revalidation and field-level revalidation are not defined for a LiveProp field.
- If Revalidation is defined on the field, it will always win.
- If no revalidation is defined on LiveProp, the framework will use global revalidation. If global is not defined, then it will use a hardcoded value of 24 hours.
This is how to use the revalidation proc-macro
```rust
// Both share one timer — same route, same interval
#[revalidate(10)]
pub price: LiveProp<f64>,

#[revalidate(10)]
pub market_cap: LiveProp<f64>,

// Different interval = separate timer
#[revalidate(30)]
pub volume: LiveProp<u64>,
```

