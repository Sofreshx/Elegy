//! Registers the `sqlite-vec` extension with every SQLite connection opened by
//! this process, via `sqlite3_auto_extension` — the one FFI call in the
//! workspace that needs `unsafe`, isolated in its own crate so
//! `unsafe_code = "forbid"` can stay in force everywhere else. Consumed by
//! `elegy-memory`; see `docs/specs/eval-harness-v1/spec.md` for why this
//! exists (accelerated vector search, replacing a Rust-side brute-force scan).
//!
//! Call [`register`] once per process, before opening any `rusqlite::Connection`
//! that needs the `vec0` virtual table module. It is safe to call more than
//! once — `sqlite3_auto_extension` de-duplicates identical registrations
//! internally — but callers should still prefer registering exactly once
//! (e.g. behind a `std::sync::OnceLock`) to avoid relying on that dedup.

use rusqlite::ffi::{sqlite3, sqlite3_api_routines};

/// Registers the `sqlite-vec` extension so every `rusqlite::Connection`
/// subsequently opened in this process has the `vec0` virtual table module
/// available, without any per-connection `load_extension` call.
pub fn register() {
    // SAFETY: `sqlite_vec::sqlite3_vec_init` has the exact C ABI SQLite's
    // extension-entry-point convention expects — `sqlite3_auto_extension`'s own
    // contract requires exactly that signature, verified here by naming it in
    // the transmute's target type rather than trusting inference. The transmute
    // only reinterprets a function pointer's type, changing no bytes.
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<
            *const (),
            unsafe extern "C" fn(*mut sqlite3, *mut *mut i8, *const sqlite3_api_routines) -> i32,
        >(
            sqlite_vec::sqlite3_vec_init as *const ()
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_makes_vec0_available_and_cosine_distance_matches_expectations() {
        register();
        let connection = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        connection
            .execute_batch(
                "CREATE VIRTUAL TABLE vec_probe USING vec0(embedding float[4] distance_metric=cosine);",
            )
            .expect("vec0 module must be available after register()");

        connection
            .execute(
                "INSERT INTO vec_probe(rowid, embedding) VALUES (1, ?1)",
                rusqlite::params![vector_bytes(&[1.0, 0.0, 0.0, 0.0])],
            )
            .expect("insert identical vector");
        connection
            .execute(
                "INSERT INTO vec_probe(rowid, embedding) VALUES (2, ?1)",
                rusqlite::params![vector_bytes(&[0.0, 1.0, 0.0, 0.0])],
            )
            .expect("insert orthogonal vector");

        let mut statement = connection
            .prepare(
                "SELECT rowid, distance FROM vec_probe WHERE embedding MATCH ?1 ORDER BY distance LIMIT 5",
            )
            .expect("prepare KNN query");
        let mut rows = statement
            .query_map(
                rusqlite::params![vector_bytes(&[1.0, 0.0, 0.0, 0.0])],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?)),
            )
            .expect("run KNN query");

        let (first_rowid, first_distance) = rows.next().expect("first row").expect("row ok");
        assert_eq!(first_rowid, 1);
        assert!(
            first_distance.abs() < 1e-6,
            "identical vector must have ~0 cosine distance, got {first_distance}"
        );

        let (second_rowid, second_distance) = rows.next().expect("second row").expect("row ok");
        assert_eq!(second_rowid, 2);
        assert!(
            (second_distance - 1.0).abs() < 1e-6,
            "orthogonal vector must have ~1.0 cosine distance, got {second_distance}"
        );
    }

    fn vector_bytes(values: &[f32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }
}
