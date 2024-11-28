# repr-rs: Representation Invariants for Rust
A library for representation invariants with support for automatic caching and parallelism.

See https://docs.rs/repr-rs/latest/repr_rs/struct.Repr.html and https://docs.rs/repr-rs/0.3.3/repr_rs/cache/struct.CacheableRepr.html.

```rust
use repr_rs::Repr;

#[derive(Debug)]
struct MinMax { min: i32, max: i32 }

let mut repr = Repr::new(
  MinMax { min: 1, max: 5 },
  |mm| mm.min < mm.max,
);
{
  let view = repr.read();
  assert_eq!(1, view.min);
  assert_eq!(5, view.max);
}
repr.write().min = 4;
let view = repr.read();
assert_eq!(4, view.min);
assert_eq!(5, view.max);
```
