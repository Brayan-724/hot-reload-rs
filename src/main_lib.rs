use std::{
    any::Any,
    ffi::{OsStr, OsString},
    io::Write,
    process::Command,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use crate::mpmc;
use crate::watcher::PollWatcher;

type StateLib = Box<dyn Any + Send + 'static>;

#[derive(Debug)]
struct StateWrapper(Box<dyn Any + Send + 'static>);
unsafe impl Send for StateWrapper {}
unsafe impl Sync for StateWrapper {}

fn get_lib_path(mut orginal: OsString, counter: Arc<AtomicU8>) -> OsString {
    orginal.push(format!(".{}", counter.load(Ordering::Relaxed)));

    orginal
}

pub fn main_lib(lib_path: impl AsRef<OsStr>, src_path: &'static str) {
    let counter = Arc::new(AtomicU8::new(0));

    let org_lib_path = lib_path.as_ref().to_os_string();

    let mut poller = PollWatcher::new(src_path.into()).expect("Cannot initialize watcher poller");

    if !build(src_path) {
        return;
    }

    let lib_path = get_lib_path(org_lib_path.clone(), Arc::clone(&counter));
    std::fs::copy(&org_lib_path, &lib_path).expect("Cannot backup library");

    let lib = load_lib(lib_path.clone()).expect("Load dynamic library");

    let state = unsafe {
        let init = lib
            .get::<fn() -> StateLib>(b"hot_init")
            .expect("Should have 'hot_init' function public");
        Some(init())
    };

    let (tx, rx) = mpmc::sync_channel::<()>(1);

    println!("[HOT] Initialized: {state:?}");

    let lib = Arc::new(Mutex::new(Some(lib)));
    let state = Arc::new(Mutex::new(state));

    let term = Arc::new(AtomicBool::new(false));
    let sigint =
        signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term)).unwrap();

    let mut reload_pending = false;
    let reloading = Arc::new(AtomicBool::new(true));

    {
        let reloading = Arc::clone(&reloading);
        let lib = Arc::clone(&lib);
        let state = Arc::clone(&state);
        let rx = rx.clone();

        thread::spawn(move || {
            reloading.store(false, Ordering::Release);

            unsafe {
                call_lib_symbol::<fn(StateLib, mpmc::Receiver<()>) -> StateLib>(
                    lib,
                    b"hot_main",
                    state,
                    move |f, state| Some(f(state, rx.clone())),
                );
            }
        });
    }

    loop {
        let changed = poller.poll();
        if changed {
            reload_pending = true;
            println!("[HOT] Needs reload: {:?}", (reload_pending, &*reloading))
        }

        if reload_pending && !reloading.load(Ordering::Relaxed) {
            reloading.store(true, Ordering::Release);
            reload_pending = false;
            tx.send(()).unwrap();

            if !build(src_path) {
                reloading.store(false, Ordering::Release);
                continue;
            }

            unsafe {
                call_lib_symbol_opt::<fn(StateLib) -> StateLib>(
                    Arc::clone(&lib),
                    b"hot_post_main",
                    Arc::clone(&state),
                    |f, state| Some(f(state)),
                );
            }

            if let Some(l) = lib.lock().unwrap().take() {
                l.close().expect("Cannot close old library");
                let _ = std::fs::remove_file(&lib_path);
            }

            let counter = Arc::clone(&counter);
            let reloading = Arc::clone(&reloading);
            let lib = Arc::clone(&lib);
            let state = Arc::clone(&state);
            let rx = rx.clone();
            let org_lib_path = org_lib_path.clone();

            thread::spawn(move || {
                counter.fetch_add(1, Ordering::Release);
                let lib_path = get_lib_path(org_lib_path.clone(), counter);
                std::fs::copy(&org_lib_path, &lib_path).expect("Cannot backup library");

                let l = load_lib(lib_path).expect("Load dynamic library");
                *lib.lock().unwrap() = Some(l);

                reloading.store(false, Ordering::Release);

                unsafe {
                    call_lib_symbol::<fn(StateLib, mpmc::Receiver<()>) -> StateLib>(
                        lib,
                        b"hot_main",
                        state,
                        move |f, state| Some(f(state, rx.clone())),
                    );
                }
            });
        }

        // wait to interrupt
        if term.load(Ordering::Relaxed) {
            tx.send(()).unwrap();
            break;
        }

        thread::sleep(Duration::from_millis(100));
    }

    // TODO: Check memory leak on this line
    assert!(signal_hook::low_level::unregister(sigint));
    unsafe {
        call_lib_symbol_opt::<fn(StateLib)>(lib, b"hot_drop", state, |f, state| {
            f(state);
            None
        });
    }
}

fn load_lib(lib_path: OsString) -> Result<libloading::Library, libloading::Error> {
    unsafe { libloading::Library::new(lib_path) }
}

fn build(src_path: &'static str) -> bool {
    println!("\x1b[2m------------------\x1b[0m");
    print!("\x1b[1J\x1b[1;1H");
    _ = std::io::stdout().flush();
    println!("\x1b[2m[HOT] Rebuilding {src_path:?}\x1b[0m");
    let mut cmd = Command::new(env!("CARGO"))
        .args(["build", "--lib", "--features", "hot"])
        .current_dir(src_path)
        .spawn()
        .expect("Cannot execute build for package");

    let exit = cmd.wait().expect("Waiting for build");
    if !exit.success() {
        eprintln!("[HOT] Error building. Cancelling reload");
        return false;
    }
    _ = std::io::stdout().flush();
    println!("\x1b[2m------------------\x1b[0m");
    print!("\x1b[1J\x1b[1;1H");
    _ = std::io::stdout().flush();

    true
}

unsafe fn call_lib_symbol_opt<T>(
    lib: Arc<Mutex<Option<libloading::Library>>>,
    symbol: &[u8],
    state: Arc<Mutex<Option<StateLib>>>,
    fun: impl Fn(libloading::Symbol<'_, T>, StateLib) -> Option<StateLib>,
) {
    let lib = lib.lock().unwrap();
    let lib = lib.as_ref().expect("Please report this bug");
    let Ok(main) = lib.get::<T>(symbol) else {
        return;
    };

    let old_state = state.lock().unwrap().take().unwrap();
    let new_state = fun(main, old_state);
    *state.lock().unwrap() = new_state;
}

unsafe fn call_lib_symbol<T>(
    lib: Arc<Mutex<Option<libloading::Library>>>,
    symbol: &[u8],
    state: Arc<Mutex<Option<StateLib>>>,
    fun: impl Fn(libloading::Symbol<'_, T>, StateLib) -> Option<StateLib>,
) {
    let lib = lib.lock().unwrap();
    let lib = lib.as_ref().expect("Please report this bug");
    let main = lib.get::<T>(symbol).expect(&format!(
        "Should have '{}' function public",
        String::from_utf8_lossy(symbol)
    ));

    let old_state = state.lock().unwrap().take().unwrap();
    let new_state = fun(main, old_state);
    *state.lock().unwrap() = new_state;
}
