pub(crate) fn bootstrap() {
    ensure_test_apartment();
}

pub(crate) fn ensure_test_apartment() {
    crate::system::runtime::com_runtime::ensure_hosted_library_mta_and_dpi()
        .expect("the process-wide MTA registration succeeds");
}
