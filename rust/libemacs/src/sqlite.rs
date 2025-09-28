//! Rust-backed SQLite runtime for Emacs.
//!
//! Phase 2 replaces the temporary stubs with a real implementation on top of
//! `rusqlite` and the raw SQLite C interface.  The C layer keeps ownership of
//! Lisp objects and remains responsible for translating between Lisp values and
//! the simple scalar representations defined here.  This module focuses on
//! connection management, prepared statements, parameter binding, stepping, and
//! error propagation.

use libc::{c_char, c_int, c_void};
use rusqlite::ffi;
use rusqlite::{Connection, OpenFlags};
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::ptr;
use std::slice;
use std::str;

/// Result structure shared with the C shim.
#[repr(C)]
pub struct EmacsSqliteResult {
    /// Zero indicates success; SQLite error codes are propagated as-is.
    pub code: c_int,
    /// Optional UTF-8 error message owned by a thread-local scratch buffer.
    pub message: *const c_char,
}

/// Tag describing a scalar SQLite value.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EmacsSqliteValueTag {
    Null = 0,
    Integer = 1,
    Float = 2,
    Text = 3,
    Blob = 4,
}

/// Simple value representation exchanged with the C layer.  For `Text` and
/// `Blob`, the data pointer references memory managed by the Rust side and
/// remains valid until the next call that touches the same statement handle.
#[repr(C)]
pub struct EmacsSqliteValue {
    pub tag: EmacsSqliteValueTag,
    pub int_value: i64,
    pub float_value: f64,
    pub bytes_ptr: *const u8,
    pub bytes_len: usize,
}

/// Helper describing a UTF-8 string slice returned to C.
#[repr(C)]
pub struct EmacsSqliteString {
    pub data: *const c_char,
    pub len: usize,
}

#[repr(C)]
pub struct EmacsSqliteHandle {
    pub raw: *mut c_void,
}

thread_local! {
    static LAST_ERROR: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}

fn flush_error(out: *mut EmacsSqliteResult, code: c_int, message: Option<&str>) {
    unsafe {
        if out.is_null() {
            return;
        }
        (*out).code = code;
        if let Some(msg) = message {
            LAST_ERROR.with(|buf| {
                let mut buf = buf.borrow_mut();
                buf.clear();
                buf.extend_from_slice(msg.as_bytes());
                buf.push(0);
                (*out).message = buf.as_ptr() as *const c_char;
            });
        } else {
            (*out).message = ptr::null();
        }
    }
}

fn flush_error_from_sqlite(out: *mut EmacsSqliteResult, code: c_int, raw: *const c_char) {
    if raw.is_null() {
        flush_error(out, code, None);
    } else {
        unsafe {
            let c_str = CStr::from_ptr(raw);
            let text = c_str.to_string_lossy();
            flush_error(out, code, Some(&text));
        }
    }
}

fn ok(out: *mut EmacsSqliteResult) {
    flush_error(out, ffi::SQLITE_OK, None);
}

struct ValueStorage {
    tag: EmacsSqliteValueTag,
    int_value: i64,
    float_value: f64,
    bytes: Vec<u8>,
}

impl Default for ValueStorage {
    fn default() -> Self {
        Self {
            tag: EmacsSqliteValueTag::Null,
            int_value: 0,
            float_value: 0.0,
            bytes: Vec::new(),
        }
    }
}

impl ValueStorage {
    fn as_c_value(&self, dest: &mut EmacsSqliteValue) {
        dest.tag = self.tag;
        dest.int_value = self.int_value;
        dest.float_value = self.float_value;
        if matches!(self.tag, EmacsSqliteValueTag::Text | EmacsSqliteValueTag::Blob) {
            dest.bytes_ptr = self.bytes.as_ptr();
            dest.bytes_len = self.bytes.len();
        } else {
            dest.bytes_ptr = ptr::null();
            dest.bytes_len = 0;
        }
    }
}

struct ConnectionHandle {
    conn: RefCell<Connection>,
    statement_count: usize,
}

struct StatementHandle {
    conn: *mut ConnectionHandle,
    stmt: *mut ffi::sqlite3_stmt,
    column_names: Vec<Vec<u8>>,
    row_values: Vec<ValueStorage>,
    column_count: usize,
    done: bool,
}

unsafe fn connection_ptr(raw: *mut EmacsSqliteHandle) -> Option<*mut ConnectionHandle> {
    if raw.is_null() {
        None
    } else {
        Some((*raw).raw as *mut ConnectionHandle)
    }
}

unsafe fn statement_ptr(raw: *mut EmacsSqliteHandle) -> Option<*mut StatementHandle> {
    if raw.is_null() {
        None
    } else {
        Some((*raw).raw as *mut StatementHandle)
    }
}

fn open_flags(readonly: bool, disable_uri: bool, path: &CStr) -> OpenFlags {
    let mut flags = if readonly {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    };
    flags |= OpenFlags::SQLITE_OPEN_FULL_MUTEX;
    if !disable_uri {
        flags |= OpenFlags::SQLITE_OPEN_URI;
    }
    if let Ok(path_str) = path.to_str() {
        if path_str.starts_with(":memory") {
            flags |= OpenFlags::SQLITE_OPEN_MEMORY;
        }
    }
    flags
}

fn copy_column_names(stmt: *mut ffi::sqlite3_stmt) -> Vec<Vec<u8>> {
    let count = unsafe { ffi::sqlite3_column_count(stmt) };
    let mut names = Vec::with_capacity(count as usize);
    for idx in 0..count {
        let raw = unsafe { ffi::sqlite3_column_name(stmt, idx) };
        if raw.is_null() {
            names.push(Vec::new());
        } else {
            let slice = unsafe { CStr::from_ptr(raw) };
            names.push(slice.to_bytes().to_vec());
        }
    }
    names
}

fn populate_row_storage(stmt: *mut ffi::sqlite3_stmt, storage: &mut [ValueStorage]) -> c_int {
    let column_count = unsafe { ffi::sqlite3_column_count(stmt) } as usize;
    for (idx, cell) in storage.iter_mut().enumerate().take(column_count) {
        let column_type = unsafe { ffi::sqlite3_column_type(stmt, idx as c_int) };
        match column_type {
            ffi::SQLITE_INTEGER => {
                cell.tag = EmacsSqliteValueTag::Integer;
                cell.int_value = unsafe { ffi::sqlite3_column_int64(stmt, idx as c_int) };
                cell.float_value = 0.0;
                cell.bytes.clear();
            }
            ffi::SQLITE_FLOAT => {
                cell.tag = EmacsSqliteValueTag::Float;
                cell.float_value = unsafe { ffi::sqlite3_column_double(stmt, idx as c_int) };
                cell.bytes.clear();
            }
            ffi::SQLITE_TEXT => {
                cell.tag = EmacsSqliteValueTag::Text;
                cell.int_value = 0;
                cell.float_value = 0.0;
                let bytes = unsafe {
                    let len = ffi::sqlite3_column_bytes(stmt, idx as c_int) as usize;
                    let ptr = ffi::sqlite3_column_text(stmt, idx as c_int);
                    if ptr.is_null() {
                        &[]
                    } else {
                        slice::from_raw_parts(ptr as *const u8, len)
                    }
                };
                cell.bytes.clear();
                cell.bytes.extend_from_slice(bytes);
            }
            ffi::SQLITE_BLOB => {
                cell.tag = EmacsSqliteValueTag::Blob;
                cell.int_value = 0;
                cell.float_value = 0.0;
                let bytes = unsafe {
                    let len = ffi::sqlite3_column_bytes(stmt, idx as c_int) as usize;
                    let ptr = ffi::sqlite3_column_blob(stmt, idx as c_int);
                    if ptr.is_null() {
                        &[]
                    } else {
                        slice::from_raw_parts(ptr as *const u8, len)
                    }
                };
                cell.bytes.clear();
                cell.bytes.extend_from_slice(bytes);
            }
            _ => {
                cell.tag = EmacsSqliteValueTag::Null;
                cell.int_value = 0;
                cell.float_value = 0.0;
                cell.bytes.clear();
            }
        }
    }
    ffi::SQLITE_ROW
}

fn bind_values(stmt: *mut ffi::sqlite3_stmt, values: &[EmacsSqliteValue]) -> Result<(), c_int> {
    for (idx, value) in values.iter().enumerate() {
        let position = (idx + 1) as c_int;
        let code = unsafe {
            match value.tag {
                EmacsSqliteValueTag::Null => ffi::sqlite3_bind_null(stmt, position),
                EmacsSqliteValueTag::Integer => ffi::sqlite3_bind_int64(stmt, position, value.int_value),
                EmacsSqliteValueTag::Float => ffi::sqlite3_bind_double(stmt, position, value.float_value),
                EmacsSqliteValueTag::Text => ffi::sqlite3_bind_text(
                    stmt,
                    position,
                    value.bytes_ptr as *const c_char,
                    value.bytes_len as c_int,
                    ffi::SQLITE_TRANSIENT(),
                ),
                EmacsSqliteValueTag::Blob => ffi::sqlite3_bind_blob(
                    stmt,
                    position,
                    value.bytes_ptr as *const c_void,
                    value.bytes_len as c_int,
                    ffi::SQLITE_TRANSIENT(),
                ),
            }
        };
        if code != ffi::SQLITE_OK {
            return Err(code);
        }
    }
    Ok(())
}

fn make_handle(ptr: *mut c_void) -> *mut EmacsSqliteHandle {
    Box::into_raw(Box::new(EmacsSqliteHandle { raw: ptr }))
}

unsafe fn drop_handle(handle: *mut EmacsSqliteHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

/// Ensure the Rust archive links in.
#[no_mangle]
pub extern "C" fn emacs_rust_sqlite_probe() {}

#[no_mangle]
pub extern "C" fn emacs_rust_sqlite_is_available() -> bool {
    Connection::open_in_memory().is_ok()
}

#[no_mangle]
pub extern "C" fn emacs_rust_sqlite_open(
    path: *const c_char,
    readonly: bool,
    disable_uri: bool,
    out: *mut EmacsSqliteResult,
) -> *mut EmacsSqliteHandle {
    if path.is_null() {
        flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: path must not be null"));
        return ptr::null_mut();
    }

    let c_path = unsafe { CStr::from_ptr(path) };
    let flags = open_flags(readonly, disable_uri, c_path);
    match Connection::open_with_flags(c_path.to_string_lossy().as_ref(), flags) {
        Ok(conn) => {
            ok(out);
            let handle = ConnectionHandle {
                conn: RefCell::new(conn),
                statement_count: 0,
            };
            let boxed = Box::new(handle);
            make_handle(Box::into_raw(boxed) as *mut c_void)
        }
        Err(err) => {
            flush_error(out, ffi::SQLITE_CANTOPEN, Some(&err.to_string()));
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn emacs_rust_sqlite_close(
    handle: *mut EmacsSqliteHandle,
    out: *mut EmacsSqliteResult,
) -> c_int {
    unsafe {
        match connection_ptr(handle) {
            None => {
                flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: invalid connection handle"));
                ffi::SQLITE_MISUSE
            }
            Some(conn_ptr) => {
                let conn = &mut *conn_ptr;
                if conn.statement_count > 0 {
                    flush_error(out, ffi::SQLITE_BUSY, Some("sqlite: statements still active"));
                    return ffi::SQLITE_BUSY;
                }
                drop(Box::from_raw(conn_ptr));
                drop_handle(handle);
                ok(out);
                ffi::SQLITE_OK
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn emacs_rust_sqlite_prepare(
    handle: *mut EmacsSqliteHandle,
    query: *const c_char,
    values: *const EmacsSqliteValue,
    value_len: usize,
    out: *mut EmacsSqliteResult,
) -> *mut EmacsSqliteHandle {
    unsafe {
        let conn_ptr = match connection_ptr(handle) {
            None => {
                flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: invalid connection handle"));
                return ptr::null_mut();
            }
            Some(ptr) => &mut *ptr,
        };
        if query.is_null() {
            flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: query is null"));
            return ptr::null_mut();
        }

        let sql = CStr::from_ptr(query);
        let db_handle = {
            let borrow = conn_ptr.conn.borrow();
            borrow.handle()
        };

        let sql_c = match CString::new(sql.to_bytes()) {
            Ok(cstr) => cstr,
            Err(_) => {
                flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: query contains embedded NUL"));
                return ptr::null_mut();
            }
        };

        let mut stmt_ptr: *mut ffi::sqlite3_stmt = ptr::null_mut();
        let prepare_code = ffi::sqlite3_prepare_v2(
            db_handle,
            sql_c.as_ptr(),
            -1,
            &mut stmt_ptr,
            ptr::null_mut(),
        );
        if prepare_code != ffi::SQLITE_OK {
            let errmsg = ffi::sqlite3_errmsg(db_handle);
            flush_error_from_sqlite(out, prepare_code, errmsg);
            return ptr::null_mut();
        }

        if value_len > 0 {
            let slice = slice::from_raw_parts(values, value_len);
            if let Err(code) = bind_values(stmt_ptr, slice) {
                let errmsg = ffi::sqlite3_errmsg(db_handle);
                flush_error_from_sqlite(out, code, errmsg);
                ffi::sqlite3_finalize(stmt_ptr);
                return ptr::null_mut();
            }
        }

        let column_count = ffi::sqlite3_column_count(stmt_ptr) as usize;
        let column_names = copy_column_names(stmt_ptr);
        let mut row_values = Vec::with_capacity(column_count);
        row_values.resize_with(column_count, ValueStorage::default);

        conn_ptr.statement_count += 1;
        ok(out);
        let handle = StatementHandle {
            conn: conn_ptr,
            stmt: stmt_ptr,
            column_names,
            row_values,
            column_count,
            done: false,
        };
        let boxed = Box::new(handle);
        make_handle(Box::into_raw(boxed) as *mut c_void)
    }
}

#[no_mangle]
pub extern "C" fn emacs_rust_sqlite_step(
    handle: *mut EmacsSqliteHandle,
    out_values: *mut EmacsSqliteValue,
    capacity: usize,
    out_count: *mut usize,
    out: *mut EmacsSqliteResult,
) -> c_int {
    unsafe {
        let stmt_ptr = match statement_ptr(handle) {
            None => {
                flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: invalid statement handle"));
                return ffi::SQLITE_MISUSE;
            }
            Some(ptr) => ptr,
        };

        let stmt = &mut *stmt_ptr;
        if capacity < stmt.column_count {
            flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: insufficient buffer for row values"));
            return ffi::SQLITE_MISUSE;
        }

        let code = ffi::sqlite3_step(stmt.stmt);
        if code == ffi::SQLITE_ROW {
            populate_row_storage(stmt.stmt, &mut stmt.row_values);
            let count = stmt.column_count;
            for idx in 0..count {
                let mut value = EmacsSqliteValue {
                    tag: EmacsSqliteValueTag::Null,
                    int_value: 0,
                    float_value: 0.0,
                    bytes_ptr: ptr::null(),
                    bytes_len: 0,
                };
                stmt.row_values[idx].as_c_value(&mut value);
                ptr::write(out_values.add(idx), value);
            }
            if !out_count.is_null() {
                *out_count = count;
            }
            stmt.done = false;
            ok(out);
            ffi::SQLITE_ROW
        } else if code == ffi::SQLITE_DONE {
            stmt.done = true;
            if !out_count.is_null() {
                *out_count = 0;
            }
            ok(out);
            ffi::SQLITE_DONE
        } else {
            let errmsg = ffi::sqlite3_errmsg(ffi::sqlite3_db_handle(stmt.stmt));
            flush_error_from_sqlite(out, code, errmsg);
            code
        }
    }
}

#[no_mangle]
pub extern "C" fn emacs_rust_sqlite_more(handle: *mut EmacsSqliteHandle) -> bool {
    unsafe {
        match statement_ptr(handle) {
            None => false,
            Some(ptr) => {
                let stmt = &*ptr;
                !stmt.done
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn emacs_rust_sqlite_columns(
    handle: *mut EmacsSqliteHandle,
    out_columns: *mut EmacsSqliteString,
    capacity: usize,
    out_count: *mut usize,
) {
    unsafe {
        if let Some(ptr) = statement_ptr(handle) {
            let stmt = &*ptr;
            let count = stmt.column_names.len().min(capacity);
            for idx in 0..count {
                let bytes = &stmt.column_names[idx];
                let entry = EmacsSqliteString {
                    data: bytes.as_ptr() as *const c_char,
                    len: bytes.len(),
                };
                ptr::write(out_columns.add(idx), entry);
            }
            if !out_count.is_null() {
                *out_count = count;
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn emacs_rust_sqlite_finalize(
    handle: *mut EmacsSqliteHandle,
    out: *mut EmacsSqliteResult,
) -> c_int {
    unsafe {
        let stmt_ptr = match statement_ptr(handle) {
            None => {
                flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: invalid statement handle"));
                return ffi::SQLITE_MISUSE;
            }
            Some(ptr) => ptr,
        };
        let stmt_box = Box::from_raw(stmt_ptr);
        let conn = &mut *stmt_box.conn;
        conn.statement_count = conn.statement_count.saturating_sub(1);
        let raw_stmt = stmt_box.stmt;
        let code = ffi::sqlite3_finalize(raw_stmt);
        drop(stmt_box);
        drop_handle(handle);
        if code == ffi::SQLITE_OK {
            ok(out);
        } else {
            let errmsg = ffi::sqlite3_errmsg(ffi::sqlite3_db_handle(raw_stmt));
            flush_error_from_sqlite(out, code, errmsg);
        }
        code
    }
}

#[no_mangle]
pub extern "C" fn emacs_rust_sqlite_execute(
    handle: *mut EmacsSqliteHandle,
    query: *const c_char,
    values: *const EmacsSqliteValue,
    value_len: usize,
    out_changes: *mut i64,
    out: *mut EmacsSqliteResult,
) -> c_int {
    unsafe {
        let conn_ptr = match connection_ptr(handle) {
            None => {
                flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: invalid connection handle"));
                return ffi::SQLITE_MISUSE;
            }
            Some(ptr) => &mut *ptr,
        };
        if query.is_null() {
            flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: query is null"));
            return ffi::SQLITE_MISUSE;
        }

        let sql = CStr::from_ptr(query);
        let db_handle = {
            let borrow = conn_ptr.conn.borrow();
            borrow.handle()
        };

        let sql_c = match CString::new(sql.to_bytes()) {
            Ok(cstr) => cstr,
            Err(_) => {
                flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: query contains embedded NUL"));
                return ffi::SQLITE_MISUSE;
            }
        };

        let mut stmt_ptr: *mut ffi::sqlite3_stmt = ptr::null_mut();
        let prepare_code = ffi::sqlite3_prepare_v2(
            db_handle,
            sql_c.as_ptr(),
            -1,
            &mut stmt_ptr,
            ptr::null_mut(),
        );
        if prepare_code != ffi::SQLITE_OK {
            let errmsg = ffi::sqlite3_errmsg(db_handle);
            flush_error_from_sqlite(out, prepare_code, errmsg);
            return prepare_code;
        }

        if value_len > 0 {
            let slice = slice::from_raw_parts(values, value_len);
            if let Err(code) = bind_values(stmt_ptr, slice) {
                let errmsg = ffi::sqlite3_errmsg(db_handle);
                flush_error_from_sqlite(out, code, errmsg);
                ffi::sqlite3_finalize(stmt_ptr);
                return code;
            }
        }

        let mut step_code;
        loop {
            step_code = ffi::sqlite3_step(stmt_ptr);
            if step_code != ffi::SQLITE_ROW {
                break;
            }
        }

        if step_code != ffi::SQLITE_DONE {
            let errmsg = ffi::sqlite3_errmsg(db_handle);
            flush_error_from_sqlite(out, step_code, errmsg);
            ffi::sqlite3_finalize(stmt_ptr);
            return step_code;
        }

        let finalize_code = ffi::sqlite3_finalize(stmt_ptr);
        if finalize_code != ffi::SQLITE_OK {
            let errmsg = ffi::sqlite3_errmsg(db_handle);
            flush_error_from_sqlite(out, finalize_code, errmsg);
            return finalize_code;
        }

        if !out_changes.is_null() {
            *out_changes = ffi::sqlite3_changes(db_handle) as i64;
        }
        ok(out);
        ffi::SQLITE_OK
    }
}

#[no_mangle]
pub extern "C" fn emacs_rust_sqlite_execute_batch(
    handle: *mut EmacsSqliteHandle,
    statements: *const c_char,
    out: *mut EmacsSqliteResult,
) -> c_int {
    unsafe {
        let conn_ptr = match connection_ptr(handle) {
            None => {
                flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: invalid connection handle"));
                return ffi::SQLITE_MISUSE;
            }
            Some(ptr) => &mut *ptr,
        };
        if statements.is_null() {
            flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: statements string is null"));
            return ffi::SQLITE_MISUSE;
        }
        let sql = CStr::from_ptr(statements);
        let sql_c = match CString::new(sql.to_bytes()) {
            Ok(cstr) => cstr,
            Err(_) => {
                flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: statements contain embedded NUL"));
                return ffi::SQLITE_MISUSE;
            }
        };
        let db_handle = {
            let borrow = conn_ptr.conn.borrow();
            borrow.handle()
        };
        let mut errmsg: *mut c_char = ptr::null_mut();
        let code = ffi::sqlite3_exec(
            db_handle,
            sql_c.as_ptr(),
            None,
            ptr::null_mut(),
            &mut errmsg,
        );
        if code != ffi::SQLITE_OK {
            if !errmsg.is_null() {
                let msg = CStr::from_ptr(errmsg).to_string_lossy().into_owned();
                flush_error(out, code, Some(&msg));
                ffi::sqlite3_free(errmsg as *mut c_void);
            } else {
                let msg = ffi::sqlite3_errmsg(db_handle);
                flush_error_from_sqlite(out, code, msg);
            }
            code
        } else {
            if !errmsg.is_null() {
                ffi::sqlite3_free(errmsg as *mut c_void);
            }
            ok(out);
            ffi::SQLITE_OK
        }
    }
}

#[no_mangle]
pub extern "C" fn emacs_rust_sqlite_version(out: *mut EmacsSqliteResult) -> EmacsSqliteString {
    ok(out);
    let version = rusqlite::version();
    EmacsSqliteString {
        data: version.as_ptr() as *const c_char,
        len: version.len(),
    }
}

#[no_mangle]
pub extern "C" fn emacs_rust_sqlite_load_extension(
    handle: *mut EmacsSqliteHandle,
    path: *const c_char,
    symbol: *const c_char,
    out: *mut EmacsSqliteResult,
) -> c_int {
    unsafe {
        let conn = match connection_ptr(handle) {
            None => {
                flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: invalid connection handle"));
                return ffi::SQLITE_MISUSE;
            }
            Some(ptr) => &mut *ptr,
        };
        if path.is_null() {
            flush_error(out, ffi::SQLITE_MISUSE, Some("sqlite: module path is null"));
            return ffi::SQLITE_MISUSE;
        }
        let module = CStr::from_ptr(path).to_string_lossy().into_owned();
        let entry = if symbol.is_null() {
            None
        } else {
            Some(CStr::from_ptr(symbol).to_string_lossy().into_owned())
        };
        let borrow = conn.conn.borrow_mut();
        if let Err(err) = borrow.load_extension_enable() {
            flush_error(out, ffi::SQLITE_ERROR, Some(&err.to_string()));
            return ffi::SQLITE_ERROR;
        }
        let load_result = borrow.load_extension(&module, entry.as_deref());
        let _ = borrow.load_extension_disable();
        match load_result {
            Ok(()) => {
                ok(out);
                ffi::SQLITE_OK
            }
            Err(err) => {
                flush_error(out, ffi::SQLITE_ERROR, Some(&err.to_string()));
                ffi::SQLITE_ERROR
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn emacs_rust_sqlite_changes(
    handle: *mut EmacsSqliteHandle,
) -> i64 {
    unsafe {
        match connection_ptr(handle) {
            None => 0,
            Some(ptr) => {
                let db_handle = {
                    let borrow = (&*ptr).conn.borrow();
                    borrow.handle()
                };
                ffi::sqlite3_changes(db_handle) as i64
            }
        }
    }
}
