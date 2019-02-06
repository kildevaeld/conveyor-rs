# conveyor-rs

```rust

use conveyor::*;

let chain = conveyor![
  station_fn(async move |input:String| Ok(input.len())),
  station_fn(async move |len:i32| Ok(len * 7))
];

let ans = chain.execute("Hello!");

assert_eq(ans, 42);




```
