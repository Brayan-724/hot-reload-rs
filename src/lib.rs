#[cfg(feature = "enable")]
pub mod main_lib;
#[cfg(feature = "enable")]
mod watcher;

pub mod mpmc;
pub mod thread;

#[cfg(feature = "enable")]
pub extern crate libloading;

#[macro_export]
macro_rules! hot_lib {
    (   $state:ty;
        init => $hot_init:ident;
        main => $hot_main:ident;
        $(post_main => $hot_post_main:ident;)?
        $(drop => $hot_drop:ident;)?
    ) => {
        #[no_mangle]
        pub fn hot_init() -> Box<dyn ::std::any::Any> {
            // macro type-safety
            let state: $state = $hot_init();

            ::std::boxed::Box::new(state)
        }

        #[no_mangle]
        pub unsafe fn hot_main(state: Box<dyn ::std::any::Any>, rx: $crate::mpmc::Receiver<()>) -> Box<dyn ::std::any::Any> {
            // macro type-safety
            let state: $state = *state.downcast().expect("State should be equal between reloads");

            // macro type-safety
            let state: $state = $hot_main(state, rx);

            ::std::boxed::Box::new(state)
        }

        $(
        #[no_mangle]
        pub unsafe fn hot_post_main(state: Box<dyn ::std::any::Any>) -> Box<dyn ::std::any::Any> {
            // macro type-safety
            let state: $state = *state.downcast().expect("State should be equal between reloads");

            // macro type-safety
            let state: $state = $hot_post_main(state);

            ::std::boxed::Box::new(state)
        }
        )?

        $(
        #[no_mangle]
        pub unsafe fn hot_drop(state: Box<dyn ::std::any::Any>) {
            // macro type-safety
            let state: $state = *state.downcast().expect("State should be equal between reloads");
            $hot_drop(state)
        }
        )?
    };

    (@syntax-check $exp:literal;) => {compile_error!(concat!("Expected ", $exp, " definition. '", $exp, " => $hot_", $exp, ";'"));};
    (@syntax-check $exp:literal; $tag:ident) => {compile_error!("Expected '=>'.");};
    (@syntax-check $exp:literal; $tag:ident => ) => {compile_error!(concat!("Expected identifier for ", stringify!($tag), "."));};
    (@syntax-check $exp:literal; $tag:ident => $val:ident) => {compile_error!("Expected ';'.");};
    (@syntax-check $exp:literal; $tag:ident => $val:ident;) => {};
    (@syntax-check $exp:literal; $($_:tt)*) => {compile_error!(concat!("Malformed input for ", $exp, " definition: ", stringify!($($_)*)));};

    ($state:ty;
        init => $a:ident;
        main => $b:ident;
        post_main => $c:ident;
        drop $($_:tt)*
    ) => {
        $crate::hot_lib!(@syntax-check "drop"; drop $($_)*);
    };

    ($state:ty;
        init => $hot_init:ident;
        main => $hot_main:ident;
        post_main => $c:ident;
        $tag:ident $($_:tt)*
    ) => {
        compile_error!(concat!("Expected 'drop'. Found '",stringify!($tag),"'"));
    };

    ($state:ty;
        init => $hot_init:ident;
        main => $hot_main:ident;
        post_main => $c:ident;
        $($_:tt)+
    ) => {
        $crate::hot_lib!(@syntax-check "drop"; $($_)*);
    };

    ($state:ty;
        init => $hot_init:ident;
        main => $hot_main:ident;
        post_main $($_:tt)*
    ) => {
        $crate::hot_lib!(@syntax-check "post_main"; post_main $($_)*);
    };

    ($state:ty;
        init => $hot_init:ident;
        main => $hot_main:ident;
        $tag:ident $($_:tt)*
    ) => {
        compile_error!(concat!("Expected 'post_main'. Found '",stringify!($tag),"'"));
    };

    ($state:ty;
        init => $hot_init:ident;
        main => $hot_main:ident;
        $($_:tt)*
    ) => {
        $crate::hot_lib!(@syntax-check "post_main"; $($_)*);
    };

    ($state:ty;
        init => $hot_init:ident;
        main $($_:tt)*
    ) => {
        $crate::hot_lib!(@syntax-check "main"; main $($_)*);
    };

    ($state:ty;
        init => $hot_init:ident;
        $tag:ident $($_:tt)*
    ) => {
        compile_error!(concat!("Expected 'main'. Found '",stringify!($tag),"'"));
    };

    ($state:ty;
        init => $hot_init:ident;
        $($_:tt)*
    ) => {
        $crate::hot_lib!(@syntax-check "main"; $($_)*);
    };

    ($state:ty; init $($_:tt)*) => {
        $crate::hot_lib!(@syntax-check "init"; init $($_)*);
    };

    ($state:ty; $tag:ident $($_:tt)*) => {
        compile_error!(concat!("Expected 'init'. Found '",stringify!($tag),"'"));
    };

    ($state:ty; $($_:tt)*) => {
        $crate::hot_lib!(@syntax-check "init"; $($_)*);
    };

    ($state:ty) => {
        compile_error!("Expected ';'");
    };

    () => {
        compile_error!("Expected state type. Use '()' for no state");
    };
}

#[cfg(feature = "enable")]
#[macro_export]
macro_rules! start_main {
    () => {
        let src_path = env!("CARGO_MANIFEST_DIR");

        let path = ::std::env::current_exe().expect("Cannot get executable path");
        let path = path.parent().expect("Executable should be on directory");
        let module = ::std::module_path!();
        let module = ::cargo_hot::libloading::library_filename(module);
        let module = module.to_string_lossy();
        let lib = path.to_string_lossy().to_owned() + ::std::path::MAIN_SEPARATOR_STR + module;
        let lib = lib.to_string();

        $crate::main_lib::main_lib(lib, src_path);
    };
}

#[cfg(not(feature = "enable"))]
#[macro_export]
macro_rules! start_main {
    ($($_:tt)*) => {};
}
