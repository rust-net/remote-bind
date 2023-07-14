#![allow(unused)]

#[allow(non_camel_case_types)]
pub enum Level {
    VERBOSE = 2,
    DEBUG,
    INFO,
    WARN,
    ERROR,
    WTF,
}

#[cfg(not(target_os = "android"))]
pub mod log {
    #[macro_export]
    macro_rules! log {
        ($type: expr, $($arg: tt)+) => {{
            use $crate::log::Level;
            use $crate::time::get_time;

            let time = get_time();
            let level: Level = $type;
            let str = format!($($arg)+);
            match level {
                Level::ERROR => {
                    eprintln!("{time} - {}:{} -> {}", file!(), line!(), str);
                },
                _ => {
                    println!("{time} - {}:{} -> {}", file!(), line!(), str);
                },
            }
        }}
    }

    #[macro_export]
    macro_rules! wtf {
        ($($arg: expr $(,)?)+) => {{
            dbg!($($arg,)+);
        }}
    }

    pub use log;
    pub use wtf;
}

#[cfg(target_os = "android")]
pub mod log {
    pub const FMT: *const u8 = "%s\0".as_ptr();

    extern {
        // int __android_log_write(int prio, const char* tag, const char* text);
        // int __android_log_print(int prio, const char* tag, const char* fmt, ...) __attribute__((__format__(printf, 3, 4)));
        pub fn __android_log_print(level: i32, tag: *const u8, fmt: *const u8, ...);
    }

    #[macro_export]
    macro_rules! log {
        ($type: expr, $($arg: tt)+) => {{
            use $crate::log::{Level, log};

            let level: Level = $type;
            let tag = format!("{}:{}\0", file!(), line!());
            let mut str = format!($($arg)+);
            str.push('\0');
            unsafe {
                log::__android_log_print(level as i32, tag.as_ptr(), log::FMT, str.as_ptr());
            }
        }}
    }
    
    #[macro_export]
    macro_rules! wtf {
        ($($arg: expr $(,)?)+) => {{
            $(
                $crate::log::e!("{} = {:?}", stringify!($arg), $arg);
            )+
        }}
    }

    pub use log;
    pub use wtf;
}

#[cfg(debug_assertions)]
pub mod inner {
    #[macro_export]
    macro_rules! d {
        ($($arg: tt)+) => {{
            $crate::log::log::log!(Level::DEBUG, $($arg)+);
        }}
    }

    pub use super::log::wtf;
    pub use d;
}

#[cfg(not(debug_assertions))]
pub mod inner {
    #[macro_export]
    macro_rules! empty {
        ($($arg: tt)+) => (())
    }

    pub use empty as d;
    pub use empty as wtf;
}

pub mod common {
    #[macro_export]
    macro_rules! i {
        ($($arg: tt)+) => {{
            $crate::log::log::log!(Level::INFO, $($arg)+);
        }}
    }

    #[macro_export]
    macro_rules! e {
        ($($arg: tt)+) => {{
            $crate::log::log::log!(Level::ERROR, $($arg)+);
        }}
    }

    pub use super::log::log;
    pub use i;
    pub use e;
}

pub use inner::*; // d!() and wtf!() will be delete in release version.
pub use common::*;