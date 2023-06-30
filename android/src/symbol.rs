use core::log::*;
use std::ffi::CStr;
use std::ffi::CString;
use std::ffi::c_char;

use crate::start as rust_start;
use crate::stop as rust_stop;

fn ptr2string(ptr: *const c_char) -> String {
    let cstr = unsafe { CStr::from_ptr(ptr) } ;
    let str_slice = cstr.to_str().expect("Invalid UTF-8 sequence");
    let rust_string = String::from(str_slice);
    // 在这里可以使用rust_string
    d!("Rust String: {}", rust_string);
    rust_string
}

#[no_mangle]
pub extern "C" fn start(server: *const c_char, port: u16, password: *const c_char, local_service: *const c_char) -> *const c_char {
    let handler = rust_start(ptr2string(server), port, ptr2string(password), ptr2string(local_service));
    let cstring = CString::new(handler).expect("Failed to create CString");
    // 使用 CString 并将字符串的所有权转移给 C 调用者，该函数返回的指针必须返回到 Rust 并使用 CString::from_raw 重新构造才能正确释放。
    let cstring_ptr = cstring.into_raw();
    // 将指针传递给C++
    cstring_ptr
}

#[no_mangle]
pub extern "C" fn stop(handler: *const c_char) {
    rust_stop(ptr2string(handler))
}

#[no_mangle]
pub extern "C" fn test() {
    i!("Hello, Test from librust.so!");
}
