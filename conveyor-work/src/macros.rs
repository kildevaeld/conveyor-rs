#[macro_export]
macro_rules! boxwrap {
    ($station:expr) => {{
        use $crate::utils::BoxWrap;
        BoxWrap::new($station)
    }};
}
