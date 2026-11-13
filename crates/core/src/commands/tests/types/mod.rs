macro_rules! golden {
    ($golden:expr, $actual:expr) => {
        golden!($golden, $actual, &[])
    };
    ($golden:expr, $actual:expr, $varying:expr) => {
        crate::commands::tests::golden::assert_golden($actual, $golden, $varying)
    };
}

mod action;
mod envelope;
mod records;
mod system;
