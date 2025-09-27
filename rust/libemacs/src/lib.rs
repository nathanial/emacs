//! Rust implementation for the Emacs dynamic library bridge.
//!
//! Phase 2 replaces the interim C delegation stubs with a Rust backend
//! built on top of `libloading`. Behaviour matches the historical POSIX
//! `dynlib.c` implementation and now ships as the default loader.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
compile_error!("rust dynlib backend currently supports macOS and Linux only");

use libloading::os::unix::Library as UnixLibrary;
use libloading::{Error as LibLoadingError, Library};
use std::cell::RefCell;
use std::ffi::{c_char, c_int, c_void, CStr, OsStr};
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::ptr;

/// Opaque handle type exchanged with the C side.
type DynlibHandle = *mut c_void;

/// Default flags used for module dlopen calls.
const MODULE_FLAGS: c_int = libc::RTLD_LAZY | libc::RTLD_GLOBAL;
/// Flags used for native-comp ELN handles (no RTLD_GLOBAL).
const ELN_FLAGS: c_int = libc::RTLD_LAZY;

/// Heap-owned wrapper around a libloading `Library`.
struct Handle {
    library: Option<Library>,
}

#[derive(Debug)]
struct LastError {
    buffer: Vec<u8>,
    consumed: bool,
}

impl Default for LastError {
    fn default() -> Self {
        Self {
            buffer: vec![0],
            consumed: true,
        }
    }
}

impl LastError {
    fn set_bytes(&mut self, bytes: &[u8]) {
        self.buffer.clear();
        let terminator = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        self.buffer.extend_from_slice(&bytes[..terminator]);
        self.buffer.push(0);
        self.consumed = false;
    }

    fn clear(&mut self) {
        self.consumed = true;
    }

    fn take_ptr(&mut self) -> *const c_char {
        if self.consumed || self.buffer.is_empty() {
            ptr::null()
        } else {
            self.consumed = true;
            self.buffer.as_ptr() as *const c_char
        }
    }
}

thread_local! {
    static LAST_ERROR: RefCell<LastError> = RefCell::new(LastError::default());
}

fn record_error_bytes(bytes: &[u8]) {
    LAST_ERROR.with(|state| state.borrow_mut().set_bytes(bytes));
}

fn record_error_string(message: &str) {
    record_error_bytes(message.as_bytes());
}

fn record_invalid_handle() {
    record_error_string("dynlib: invalid handle");
}

fn record_null_symbol() {
    record_error_string("dynlib: symbol name is null");
}

fn record_libloading_error(err: LibLoadingError) {
    record_error_string(&err.to_string());
}

fn clear_error() {
    LAST_ERROR.with(|state| state.borrow_mut().clear());
}

unsafe fn library_from_handle(handle: DynlibHandle) -> Result<&'static Library, ()> {
    if handle.is_null() {
        record_invalid_handle();
        return Err(());
    }
    let handle_ref = &*(handle as *const Handle);
    match handle_ref.library.as_ref() {
        Some(lib) => Ok(lib),
        None => {
            record_invalid_handle();
            Err(())
        }
    }
}

unsafe fn open_library(path: *const c_char, flags: c_int) -> Result<Library, LibLoadingError> {
    if path.is_null() {
        UnixLibrary::open(None::<&OsStr>, flags).map(Into::into)
    } else {
        let c_path = CStr::from_ptr(path);
        let os_path = OsStr::from_bytes(c_path.to_bytes());
        UnixLibrary::open(Some(os_path), flags).map(Into::into)
    }
}

unsafe fn lookup_symbol<T>(handle: DynlibHandle, symbol: *const c_char) -> Result<T, ()>
where
    T: Copy,
{
    if symbol.is_null() {
        record_null_symbol();
        return Err(());
    }
    let library = library_from_handle(handle)?;
    match library.get::<T>(CStr::from_ptr(symbol).to_bytes_with_nul()) {
        Ok(sym) => {
            clear_error();
            Ok(*sym)
        }
        Err(err) => {
            record_libloading_error(err);
            Err(())
        }
    }
}

/// Lightweight probe invoked from C to ensure the Rust archive links in.
#[no_mangle]
pub extern "C" fn emacs_rust_dynlib_probe() {}

#[no_mangle]
pub unsafe extern "C" fn emacs_rust_dynlib_open(path: *const c_char) -> DynlibHandle {
    match open_library(path, MODULE_FLAGS) {
        Ok(lib) => {
            clear_error();
            Box::into_raw(Box::new(Handle { library: Some(lib) })) as DynlibHandle
        }
        Err(err) => {
            record_libloading_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn emacs_rust_dynlib_open_for_eln(path: *const c_char) -> DynlibHandle {
    match open_library(path, ELN_FLAGS) {
        Ok(lib) => {
            clear_error();
            Box::into_raw(Box::new(Handle { library: Some(lib) })) as DynlibHandle
        }
        Err(err) => {
            record_libloading_error(err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn emacs_rust_dynlib_close(handle: DynlibHandle) -> c_int {
    if handle.is_null() {
        record_invalid_handle();
        return 0;
    }
    let mut boxed = Box::from_raw(handle as *mut Handle);
    match boxed.library.take() {
        Some(lib) => match lib.close() {
            Ok(()) => {
                clear_error();
                1
            }
            Err(err) => {
                record_libloading_error(err);
                0
            }
        },
        None => {
            record_invalid_handle();
            0
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn emacs_rust_dynlib_error() -> *const c_char {
    LAST_ERROR.with(|state| state.borrow_mut().take_ptr())
}

#[no_mangle]
pub unsafe extern "C" fn emacs_rust_dynlib_sym(
    handle: DynlibHandle,
    symbol: *const c_char,
) -> *mut c_void {
    match lookup_symbol::<*mut c_void>(handle, symbol) {
        Ok(ptr) => ptr,
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn emacs_rust_dynlib_func(
    handle: DynlibHandle,
    symbol: *const c_char,
) -> Option<unsafe extern "C" fn()> {
    match lookup_symbol::<unsafe extern "C" fn()>(handle, symbol) {
        Ok(func) => Some(func),
        Err(_) => None,
    }
}

#[no_mangle]
pub unsafe extern "C" fn emacs_rust_dynlib_addr(
    funcptr: Option<unsafe extern "C" fn()>,
    file: *mut *const c_char,
    sym: *mut *const c_char,
) {
    if !file.is_null() {
        *file = ptr::null();
    }
    if !sym.is_null() {
        *sym = ptr::null();
    }
    let Some(func) = funcptr else {
        return;
    };
    let mut info: libc::Dl_info = mem::zeroed();
    if libc::dladdr(func as *const c_void, &mut info) != 0 {
        if !file.is_null() && !info.dli_fname.is_null() {
            *file = info.dli_fname;
        }
        if !sym.is_null() && !info.dli_sname.is_null() {
            *sym = info.dli_sname;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn last_error_round_trip() {
        LAST_ERROR.with(|state| {
            let mut guard = state.borrow_mut();
            guard.set_bytes(b"example");
            assert!(!guard.take_ptr().is_null());
            assert!(guard.take_ptr().is_null());
            guard.set_bytes(b"next\0ignored");
            let ptr = guard.take_ptr();
            assert!(!ptr.is_null());
            unsafe {
                assert_eq!(CStr::from_ptr(ptr).to_bytes(), b"next");
            }
        });
    }
}
