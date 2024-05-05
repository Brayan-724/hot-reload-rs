use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub struct Thread<T>(JoinHandle<T>, mpsc::SyncSender<()>);
impl<T> Thread<T> {
    pub fn kill(self) {
        self.1.send(()).unwrap();
        let _ = self.0.join();
    }

    pub fn join(self) -> Result<T, Box<dyn std::any::Any + Send>> {
        self.0.join()
    }
}

pub fn check_ended(rx: &mpsc::Receiver<()>) -> bool {
    rx.recv_timeout(Duration::from_millis(50)).is_ok()
}

#[macro_export]
macro_rules! select_ended {
    ($rx:expr; $action:expr) => {{
        // Macro type-safety
        let rx: &$crate::mpmc::Receiver<_> = $rx;

        if rx
            .recv_timeout(::std::time::Duration::from_millis(50))
            .is_ok()
        {
            $action
        }
    }};
}
pub use select_ended;

pub struct Timer(Instant, Instant, Duration);

impl Timer {
    pub fn poll_interval(&mut self) -> bool {
        if self.poll() {
            self.reset();
            true
        } else {
            false
        }
    }

    pub fn poll(&mut self) -> bool {
        self.0 = Instant::now();
        self.0 >= self.1
    }

    pub fn reset(&mut self) {
        self.0 = Instant::now();
        self.1 = Instant::now() + self.2;
    }
}

pub fn timer(target: Duration) -> Timer {
    Timer(Instant::now(), Instant::now() + target, target)
}

pub fn spawn<F, T>(f: F) -> Thread<T>
where
    F: FnOnce(mpsc::Receiver<()>) -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = mpsc::sync_channel::<()>(1);
    let handler = std::thread::spawn(move || f(rx));

    Thread(handler, tx)
}
