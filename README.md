# conveyor-rs

Needs nightly (nightly-2019-01-19-x86_64-apple-darwin)


```rust
#![feature(async_await, await_macro, futures_api)]

use conveyor::*;

let chain = conveyor![
  station_fn(async move |input: &str| Ok(input.len())),
  station_fn(async move |len: usize| Ok(len * 7))
];

let ans = futures::executor::block_on(chain.execute("Hello!"));

assert_eq!(ans.unwrap(), 42);


```
