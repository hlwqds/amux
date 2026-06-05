use amux::pty::PtyHandle;

#[test]
fn test_shell_spawn_and_output() {
    let tmp = tempfile::tempdir().unwrap();
    let handle = PtyHandle::spawn_shell(tmp.path(), (80, 24)).expect("failed to spawn shell");

    // Should be alive
    assert!(handle.is_alive(), "shell should be alive after spawn");

    // Send a command that produces known output
    handle
        .write_input(b"echo HELLO_AMUX_TEST\n")
        .expect("write should succeed");

    // Wait for output to arrive
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Read screen contents
    let contents = handle.screen_contents();
    assert!(
        contents.contains("HELLO_AMUX_TEST"),
        "screen should contain our echo output, got: {contents}"
    );
}

#[test]
fn test_shell_resize() {
    let tmp = tempfile::tempdir().unwrap();
    let handle = PtyHandle::spawn_shell(tmp.path(), (80, 24)).expect("failed to spawn shell");

    // Resize should not panic or error
    handle.resize((120, 40));

    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(handle.is_alive(), "shell should survive resize");
}
