#[test]
fn high_abc_voice_number_does_not_panic() {
    let s = std::fs::read_to_string("/tmp/abc-vcheck-run/travail/mscz/141/tk.abc").unwrap();
    let result = std::panic::catch_unwind(|| scoretk_abc_import::read_abc(&s));
    assert!(result.is_ok(), "reader panicked on high voice numbers");
}
