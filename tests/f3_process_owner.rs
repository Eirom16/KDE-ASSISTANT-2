//! Linux parent-death belongs to the actual spawning thread, not Tokio as a whole.
use kde_assistant_lib::owned_process::{terminate_with_parent, OwnedChild};
use std::{
    os::unix::process::ExitStatusExt,
    process::Command,
    sync::mpsc,
    time::{Duration, Instant},
};

#[test]
fn parent_death_follows_creator_thread_while_process_and_sibling_live() {
    let mut sibling = OwnedChild::new(Command::new("sleep").arg("30").spawn().unwrap());
    let (send_child, receive_child) = mpsc::channel();
    let (release_creator, wait_release) = mpsc::channel();
    let creator = std::thread::spawn(move || {
        let mut command = Command::new("sleep");
        command.arg("30");
        terminate_with_parent(&mut command);
        send_child
            .send(OwnedChild::new(command.spawn().unwrap()))
            .unwrap();
        wait_release.recv().unwrap();
    });
    let mut child = receive_child.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(child.try_wait().unwrap().is_none());
    release_creator.send(()).unwrap();
    creator.join().unwrap();
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "creator died but child remained alive"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(status.signal(), Some(9));
    assert!(sibling.try_wait().unwrap().is_none());
}
