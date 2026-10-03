//! Durable request history and printer-specific PNG cache.
//! SQLite transactions and connection pragmas:
//! https://www.sqlite.org/lang_transaction.html
//! https://www.sqlite.org/pragma.html#pragma_busy_timeout
use crate::{
    models::{Input, Png},
    schema::{inputs, png_requests, pngs, render_cache},
};
use diesel::{connection::SimpleConnection, prelude::*};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Cache(Arc<Mutex<SqliteConnection>>);

#[cfg(test)]
mod tests;

pub struct Attempt {
    pub request_id: i64,
    pub input_id: i64,
    pub renderer_key: Vec<u8>,
    pub cached_png: Option<Vec<u8>>,
    pub cached_identity: Option<zebra_sgd::PrinterIdentity>,
}

pub fn renderer_key(url: &str, headers: &[String], namespace: &str, sgd_target: &str) -> Vec<u8> {
    // Length prefixes avoid ambiguous concatenation. Only the digest is stored,
    // never the configured authentication headers or credential-bearing URL.
    let mut hash = Sha256::new();
    for part in std::iter::once("zebra-http-preview-v4-sgd")
        .chain(std::iter::once(url))
        .chain(std::iter::once(namespace))
        .chain(std::iter::once(sgd_target))
        .chain(headers.iter().map(String::as_str))
    {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part.as_bytes());
    }
    hash.finalize().to_vec()
}

impl Cache {
    /// The caller must run Diesel migrations first. No user DB is migrated here.
    pub fn open(path: &str) -> eyre::Result<Self> {
        let mut connection = SqliteConnection::establish(path)?;
        connection.batch_execute(
            "PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;",
        )?;
        // Fail at startup with a migration error, not after accepting a request.
        render_cache::table
            .select((render_cache::input_id, render_cache::printer_identity))
            .limit(1)
            .load::<(i64, Option<String>)>(&mut connection)?;
        png_requests::table
            .select((
                png_requests::completed_at,
                png_requests::error,
                png_requests::printer_identity,
            ))
            .limit(1)
            .load::<(Option<String>, Option<String>, Option<String>)>(&mut connection)?;
        Ok(Self(Arc::new(Mutex::new(connection))))
    }

    async fn run<T, F>(&self, name: &'static str, operation: F) -> eyre::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut SqliteConnection) -> eyre::Result<T> + Send + 'static,
    {
        let connection = self.0.clone();
        crate::telemetry::spawn_blocking(name, move || {
            let mut connection = connection
                .lock()
                .map_err(|_| eyre::eyre!("database lock poisoned"))?;
            let result = operation(&mut connection);
            fastrace::local::LocalSpan::add_property(|| ("db.system.name", "sqlite"));
            if result.is_err() {
                fastrace::local::LocalSpan::add_property(|| ("span.status_code", "error"));
            }
            result
        })
        .await?
    }

    /// Persist input and request before contacting the printer. Pending rows
    /// remain identifiable if the process stops before storing an outcome.
    pub async fn begin(&self, data: Vec<u8>, key: Vec<u8>, refresh: bool) -> eyre::Result<Attempt> {
        self.run("cache.lookup", move |connection| {
            connection.immediate_transaction(|connection| {
                let hash = Sha256::digest(&data).to_vec();
                diesel::insert_into(inputs::table)
                    .values((inputs::hash.eq(&hash), inputs::data.eq(&data)))
                    .on_conflict(inputs::hash)
                    .do_nothing()
                    .execute(connection)?;
                let input = inputs::table
                    .filter(inputs::hash.eq(&hash))
                    .select(Input::as_select())
                    .first(connection)?;
                eyre::ensure!(input.data == data, "input hash collision");
                diesel::insert_into(png_requests::table)
                    .values((
                        png_requests::input_id.eq(input.id),
                        png_requests::renderer_key.eq(&key),
                        png_requests::timestamp.eq(diesel::dsl::sql::<diesel::sql_types::Text>(
                            "strftime('%Y-%m-%dT%H:%M:%fZ','now')",
                        )),
                    ))
                    .execute(connection)?;
                let request_id = diesel::select(diesel::dsl::sql::<diesel::sql_types::BigInt>(
                    "last_insert_rowid()",
                ))
                .get_result(connection)?;
                let cached = if refresh {
                    None
                } else {
                    render_cache::table
                        .inner_join(pngs::table)
                        .filter(render_cache::input_id.eq(input.id))
                        .filter(render_cache::renderer_key.eq(&key))
                        .select((pngs::id, pngs::data, render_cache::printer_identity))
                        .first::<(i64, Vec<u8>, Option<String>)>(connection)
                        .optional()?
                };
                let cached_identity = cached
                    .as_ref()
                    .and_then(|(_, _, identity)| identity.as_deref())
                    .map(serde_json::from_str)
                    .transpose()?;
                if let Some((png_id, _, identity)) = &cached {
                    diesel::update(png_requests::table.filter(png_requests::rowid.eq(request_id)))
                        .set((
                            png_requests::png_id.eq(png_id),
                            png_requests::cache_hit.eq(true),
                            png_requests::printer_identity.eq(identity),
                            png_requests::completed_at.eq(diesel::dsl::sql::<
                                diesel::sql_types::Nullable<diesel::sql_types::Text>,
                            >(
                                "strftime('%Y-%m-%dT%H:%M:%fZ','now')"
                            )),
                        ))
                        .execute(connection)?;
                }
                Ok(Attempt {
                    request_id,
                    input_id: input.id,
                    renderer_key: key,
                    cached_png: cached.map(|(_, data, _)| data),
                    cached_identity,
                })
            })
        })
        .await
    }

    pub async fn success(
        &self,
        attempt: Attempt,
        data: Vec<u8>,
        identity: Option<zebra_sgd::PrinterIdentity>,
    ) -> eyre::Result<()> {
        self.rendered(attempt, data, identity, None).await
    }

    /// Preserve the original printer response even if response annotation fails.
    /// Failed responses remain in history but cannot populate the render cache.
    pub async fn rendered(
        &self,
        attempt: Attempt,
        data: Vec<u8>,
        identity: Option<zebra_sgd::PrinterIdentity>,
        response_error: Option<String>,
    ) -> eyre::Result<()> {
        let identity = identity
            .map(|identity| serde_json::to_string(&identity))
            .transpose()?;
        self.run("cache.store", move |connection| {
            connection.immediate_transaction(|connection| {
                let hash = Sha256::digest(&data).to_vec();
                let public_id = hash.iter().map(|b| format!("{b:02x}")).collect::<String>();
                diesel::insert_into(pngs::table)
                    .values((
                        pngs::hash.eq(&hash),
                        pngs::data.eq(&data),
                        pngs::public_id.eq(public_id),
                        pngs::created_at.eq(diesel::dsl::sql::<diesel::sql_types::BigInt>(
                            "CAST(strftime('%s','now') AS INTEGER)",
                        )),
                    ))
                    .on_conflict(pngs::hash)
                    .do_nothing()
                    .execute(connection)?;
                let png = pngs::table
                    .filter(pngs::hash.eq(&hash))
                    .select(Png::as_select())
                    .first(connection)?;
                eyre::ensure!(png.data == data, "PNG hash collision");
                if response_error.is_some() {
                    diesel::delete(
                        render_cache::table
                            .filter(render_cache::input_id.eq(attempt.input_id))
                            .filter(render_cache::renderer_key.eq(&attempt.renderer_key)),
                    )
                    .execute(connection)?;
                } else {
                    diesel::insert_into(render_cache::table)
                        .values((
                            render_cache::input_id.eq(attempt.input_id),
                            render_cache::renderer_key.eq(&attempt.renderer_key),
                            render_cache::png_id.eq(png.id),
                            render_cache::printer_identity.eq(&identity),
                        ))
                        .on_conflict((render_cache::input_id, render_cache::renderer_key))
                        .do_update()
                        .set((
                            render_cache::png_id.eq(png.id),
                            render_cache::printer_identity.eq(&identity),
                        ))
                        .execute(connection)?;
                    // Preserve the legacy latest-output association for existing readers.
                    diesel::update(inputs::table.filter(inputs::id.eq(attempt.input_id)))
                        .set(inputs::png_id.eq(png.id))
                        .execute(connection)?;
                }
                diesel::update(
                    png_requests::table.filter(png_requests::rowid.eq(attempt.request_id)),
                )
                .set((
                    png_requests::png_id.eq(png.id),
                    png_requests::printer_identity.eq(&identity),
                    png_requests::error.eq(&response_error),
                    png_requests::completed_at.eq(diesel::dsl::sql::<
                        diesel::sql_types::Nullable<diesel::sql_types::Text>,
                    >(
                        "strftime('%Y-%m-%dT%H:%M:%fZ','now')"
                    )),
                ))
                .execute(connection)?;
                Ok(())
            })
        })
        .await
    }

    pub async fn failure(&self, attempt: Attempt, error: String) -> eyre::Result<()> {
        self.run("cache.failure", move |connection| {
            connection.immediate_transaction(|connection| {
                // A failed explicit refresh invalidates stale success for this key.
                diesel::delete(
                    render_cache::table
                        .filter(render_cache::input_id.eq(attempt.input_id))
                        .filter(render_cache::renderer_key.eq(&attempt.renderer_key)),
                )
                .execute(connection)?;
                diesel::update(
                    png_requests::table.filter(png_requests::rowid.eq(attempt.request_id)),
                )
                .set((
                    png_requests::error.eq(error),
                    png_requests::completed_at.eq(diesel::dsl::sql::<
                        diesel::sql_types::Nullable<diesel::sql_types::Text>,
                    >(
                        "strftime('%Y-%m-%dT%H:%M:%fZ','now')"
                    )),
                ))
                .execute(connection)?;
                Ok(())
            })
        })
        .await
    }
}
