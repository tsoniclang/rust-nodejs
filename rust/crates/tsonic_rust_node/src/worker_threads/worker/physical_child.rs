use shared_child::SharedChild;
use std::io;
use std::process::{Command, ExitStatus};

pub(super) struct WorkerChild {
    child: SharedChild,
}

impl WorkerChild {
    pub(super) fn spawn(command: &mut Command) -> io::Result<Self> {
        Ok(Self {
            child: SharedChild::spawn(command)?,
        })
    }

    pub(super) fn try_wait_startup(&self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    pub(super) fn kill(&self) -> io::Result<()> {
        self.child.kill()
    }

    pub(super) fn wait(&self) -> io::Result<ExitStatus> {
        self.child.wait()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::process::Stdio;
    use std::sync::{mpsc, Arc};
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn controlled_native_child_fixture() {
        let Ok(address) = std::env::var("TSONIC_P05_CHILD_LIFETIME_CONTROL") else {
            return;
        };
        let mut control = TcpStream::connect(address).unwrap();
        control.write_all(&[1]).unwrap();
        let mut released = [0];
        control.read_exact(&mut released).unwrap();
        assert_eq!(
            released,
            [1],
            "native child exits only on an explicit control release"
        );
    }

    #[cfg(target_os = "linux")]
    fn require_blocked_native_thread(name: &str, wait_channel: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            for entry in std::fs::read_dir("/proc/self/task").unwrap() {
                let directory = entry.unwrap().path();
                let Ok(selected) = std::fs::read_to_string(directory.join("comm")) else {
                    continue;
                };
                if selected.trim() != name {
                    continue;
                }
                if std::fs::read_to_string(directory.join("wchan"))
                    .is_ok_and(|channel| channel.contains(wait_channel))
                {
                    return;
                }
            }
            assert!(
                Instant::now() < deadline,
                "named native child waiter reaches its actual blocking owner"
            );
            thread::yield_now();
        }
    }

    #[test]
    fn concurrent_kill_releases_native_waiters_and_reuses_one_reaped_status() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let child = Arc::new(
            WorkerChild::spawn(
                Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "worker_threads::worker::physical_child::tests::controlled_native_child_fixture",
                        "--nocapture",
                    ])
                    .env("TSONIC_P05_CHILD_LIFETIME_CONTROL", listener.local_addr().unwrap().to_string())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null()),
            )
            .unwrap(),
        );
        let process = child.child.id();
        let (mut control, _) = listener.accept().unwrap();
        control
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut ready = [0];
        control.read_exact(&mut ready).unwrap();
        assert_eq!(ready, [1], "native child is alive and held before waiting");
        assert!(
            child.try_wait_startup().unwrap().is_none(),
            "held native child has no status"
        );
        let (started, ready) = mpsc::channel();
        let (finished, statuses) = mpsc::channel();
        let mut waiters = Vec::new();
        for name in ["p05-native-wait", "p05-peer-wait"] {
            let waiting = Arc::clone(&child);
            let started = started.clone();
            let finished = finished.clone();
            waiters.push(
                thread::Builder::new()
                    .name(name.to_owned())
                    .spawn(move || {
                        started.send(()).unwrap();
                        finished.send(waiting.wait()).unwrap();
                    })
                    .unwrap(),
            );
            ready.recv_timeout(Duration::from_secs(10)).unwrap();
            #[cfg(target_os = "linux")]
            require_blocked_native_thread(
                name,
                if name == "p05-native-wait" {
                    "do_wait"
                } else {
                    "futex"
                },
            );
        }
        assert!(
            matches!(statuses.try_recv(), Err(mpsc::TryRecvError::Empty)),
            "both native waits stay blocked before kill"
        );
        assert!(
            child.try_wait_startup().unwrap().is_none(),
            "try_wait does not steal a blocked waiter's child"
        );
        child.kill().unwrap();
        let first = statuses
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap();
        let second = statuses
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap();
        assert_eq!(
            first, second,
            "concurrent native waiters share one exact status"
        );
        assert!(
            !first.success(),
            "native termination status is not fabricated success"
        );
        for waiter in waiters {
            waiter.join().unwrap();
        }
        for _ in 0..4 {
            assert_eq!(
                child.wait().unwrap(),
                first,
                "later waits read the same cached status"
            );
            assert_eq!(
                child.try_wait_startup().unwrap(),
                Some(first),
                "later polls read the same cached status"
            );
        }
        child.kill().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(
                first.signal(),
                Some(9),
                "actual Unix SIGKILL status is retained"
            );
        }
        println!("P05_SHARED_CHILD_REAP process={process} waiters=2 later_waits=4 later_polls=4 native_status={first}");
    }
}
