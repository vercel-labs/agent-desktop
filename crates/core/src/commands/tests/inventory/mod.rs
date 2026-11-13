macro_rules! golden_test {
    ($name:ident, $golden:expr, $varying:expr, $actual:block) => {
        #[test]
        fn $name() {
            crate::commands::tests::golden::assert_golden($actual, $golden, $varying)
        }
    };
}

mod clipboard;
mod lists;
mod session;
mod status;
mod stub;
mod system;
