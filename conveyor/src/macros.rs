#[macro_export]
macro_rules! conveyor {
    [ $y: expr, $( $x:expr ),* ] => {
        {
            use $crate::Chain;
            let m = $y;
            $(
                let m = m.chain($x);
            )*
            m
        }
     };

     [ $y: expr] => {
        $y
     };
}

#[cfg(test)]
mod tests {

    use super::super::station_fn;
    use super::super::Station;
    #[test]
    fn test_conveyor() {
        let chain = conveyor![
            station_fn(async move |s: &str| Ok(s)),
            station_fn(async move |s: &str| Ok(s)),
            conveyor![station_fn(async move |s: &str| Ok(s))],
            conveyor![station_fn(async move |s: &str| Ok(s))]
        ];

        let result = futures::executor::block_on(chain.execute("Hello, World!")).unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_conveyor_meaning_of_life() {
        let chain = conveyor![
            station_fn(async move |input: &str| Ok(input.len())),
            station_fn(async move |len: usize| Ok(len * 7))
        ];

        let ans = futures::executor::block_on(chain.execute("Hello!"));

        assert_eq!(ans.unwrap(), 42);
    }

}
