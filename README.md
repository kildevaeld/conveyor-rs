# conveyor-rs

```rust

use conveyor::*;

let chain = conveyor![
  station_fn(async move |input: &str| Ok(input.len())),
  station_fn(async move |len: usize| Ok(len * 7))
];

let ans = futures::executor::block_on(chain.execute("Hello!"));

assert_eq!(ans.unwrap(), 42);


```
