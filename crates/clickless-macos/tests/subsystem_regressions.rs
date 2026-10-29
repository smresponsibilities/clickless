#![cfg(target_os = "macos")]

#[test]
fn held_leader_enters_hook_capture() {
    use clickless_backend_api::{Button, Dir, OutputBackend};
    use clickless_macos::MacosHook;
    struct DummyOut;
    impl OutputBackend for DummyOut {
        fn move_rel(&mut self, _dx: i32, _dy: i32) -> Result<(), String> {
            Ok(())
        }
        fn move_abs(&mut self, _x: i32, _y: i32) -> Result<(), String> {
            Ok(())
        }
        fn button(&mut self, _b: Button, _d: Dir) -> Result<(), String> {
            Ok(())
        }
        fn scroll(&mut self, _dx: i32, _dy: i32) -> Result<(), String> {
            Ok(())
        }
    }
    let mut hook = MacosHook::new(DummyOut);
    let _ = hook.process_key(0x39, true, 0).unwrap();
    let _ = hook.process_key(0x39, true, 200).unwrap();
    assert!(hook.is_intercepting());
}
