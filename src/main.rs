use std::ffi::OsStr;

fn main() {
    let mut args = std::env::args().skip(1);
    let lib = args.next().expect(".so path");
    let lib = load_lib(lib).expect("Load dynamic library");

    let mut state = unsafe {
        let init = lib
            .get::<unsafe fn() -> *mut u32>(b"hot_init")
            .expect("Should have 'hot_init' function public");
        init()
    };

    println!("[HOT] Initialized: {state:#?}");

    loop {
        unsafe {
            let main = lib
                .get::<unsafe fn(*mut u32) -> *mut u32>(b"hot_main")
                .expect("Should have 'hot_main' function public");
            let main = main(state);
            println!("{main:#?}");

            state = main;
        }
    }

    unsafe {
        let drop_ = lib
            .get::<unsafe fn(*mut u32)>(b"hot_drop")
            .expect("Should have 'hot_main' function public");
        let drop_ = drop_(state);
        println!("{drop_:#?}");
    }
}

fn load_lib(lib_path: impl AsRef<OsStr>) -> Result<libloading::Library, libloading::Error> {
    unsafe { libloading::Library::new(lib_path) }
}
