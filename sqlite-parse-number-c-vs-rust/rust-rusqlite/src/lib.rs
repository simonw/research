use rusqlite::ffi;
use rusqlite::functions::FunctionFlags;
use rusqlite::types::{Value, ValueRef};
use rusqlite::{Connection, Result};
use std::os::raw::{c_char, c_int};

#[path = "../../rust-core/parse.rs"]
mod parse;
use parse::Num;

/// Entry point name SQLite derives from "libparse_number.so".
#[no_mangle]
pub unsafe extern "C" fn sqlite3_parsenumber_init(
    db: *mut ffi::sqlite3,
    pz_err_msg: *mut *mut c_char,
    p_api: *mut ffi::sqlite3_api_routines,
) -> c_int {
    Connection::extension_init2(db, pz_err_msg, p_api, init)
}

fn init(db: Connection) -> Result<bool> {
    db.create_scalar_function(
        "parse_number",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC | FunctionFlags::SQLITE_INNOCUOUS,
        |ctx| {
            Ok(match ctx.get_raw(0) {
                ValueRef::Integer(i) => Value::Integer(i),
                ValueRef::Real(f) => Value::Real(f),
                ValueRef::Text(t) => match parse::parse_number(t) {
                    Some(Num::Int(i)) => Value::Integer(i),
                    Some(Num::Real(f)) => Value::Real(f),
                    None => Value::Null,
                },
                _ => Value::Null,
            })
        },
    )?;
    Ok(false)
}
