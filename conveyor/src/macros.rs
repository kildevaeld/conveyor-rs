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

    #[test]
    fn it_works() {
        conveyor![
            station_fn(async move |s: String| Ok(s)),
            station_fn(async move |s: String| Ok(s))
        ];
        conveyor![station_fn(async move |s: String| Ok(s))];
    }

}
