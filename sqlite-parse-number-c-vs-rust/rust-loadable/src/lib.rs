use sqlite_loadable::prelude::*;
use sqlite_loadable::{api, define_scalar_function, FunctionFlags, Result};

#[path = "../../rust-core/parse.rs"]
mod parse;
use parse::Num;

fn parse_number(ctx: *mut sqlite3_context, values: &[*mut sqlite3_value]) -> Result<()> {
    let v = &values[0];
    match api::value_type(v) {
        api::ValueType::Integer => api::result_int64(ctx, api::value_int64(v)),
        api::ValueType::Float => api::result_double(ctx, api::value_double(v)),
        api::ValueType::Text => match parse::parse_number(api::value_blob(v)) {
            Some(Num::Int(i)) => api::result_int64(ctx, i),
            Some(Num::Real(f)) => api::result_double(ctx, f),
            None => api::result_null(ctx),
        },
        _ => api::result_null(ctx),
    }
    Ok(())
}

#[sqlite_entrypoint]
pub fn sqlite3_parsenumber_init(db: *mut sqlite3) -> Result<()> {
    define_scalar_function(
        db,
        "parse_number",
        1,
        parse_number,
        FunctionFlags::UTF8 | FunctionFlags::DETERMINISTIC | FunctionFlags::INNOCUOUS,
    )?;
    Ok(())
}
