//! Printer identity and recovery audit records. Values are bound, never SQL text.
use super::*;
use crate::printer::Identity;
use diesel::sql_types::{BigInt, Binary, Nullable, Text};

#[derive(QueryableByName)]
struct ErrorRow {
    #[diesel(sql_type = Text)]
    error: String,
}

#[derive(QueryableByName)]
struct IdentityRow {
    #[diesel(sql_type = Text)]
    serial: String,
    #[diesel(sql_type = Text)]
    firmware: String,
}

impl Cache {
    pub async fn last_identity(&self, name: String) -> eyre::Result<Option<Identity>> {
        self.run("cache.last_identity", move |db| {
            Ok(diesel::sql_query("SELECT serial,firmware FROM printer_requests WHERE name=? AND serial IS NOT NULL AND firmware IS NOT NULL ORDER BY request_id DESC LIMIT 1")
                .bind::<Text,_>(name).get_result::<IdentityRow>(db).optional()?.map(|row| Identity {serial:row.serial,firmware:row.firmware}))
        }).await
    }

    pub async fn reserve_recovery(&self, key: Vec<u8>, cooldown: u64) -> eyre::Result<bool> {
        self.run("cache.recovery", move |db| {
            let milliseconds = i64::try_from(cooldown)?.checked_mul(1000).and_then(|v| v.checked_add(1)).ok_or_else(|| eyre::eyre!("cooldown overflow"))?;
            let changed = diesel::sql_query("INSERT INTO printer_recovery(renderer_key,retry_at) VALUES(?,CAST((julianday('now')-2440587.5)*86400000 AS INTEGER)+?) ON CONFLICT(renderer_key) DO UPDATE SET retry_at=excluded.retry_at WHERE printer_recovery.retry_at <= CAST((julianday('now')-2440587.5)*86400000 AS INTEGER)")
                .bind::<Binary,_>(key).bind::<BigInt,_>(milliseconds).execute(db)?;
            Ok(changed == 1)
        }).await
    }

    pub async fn name_request(&self, request_id: i64, name: String) -> eyre::Result<()> {
        self.run("cache.printer", move |db| {
            diesel::sql_query("INSERT INTO printer_requests(request_id,name) VALUES(?,?)")
                .bind::<BigInt, _>(request_id)
                .bind::<Text, _>(name)
                .execute(db)?;
            Ok(())
        })
        .await
    }

    /// Resolve only after live identity has been read, before any preview. Public
    /// refresh cannot bypass a quarantined label; change namespace to retest it.
    pub async fn resolve_printer(
        &self,
        mut attempt: Attempt,
        identity: Identity,
        key: Vec<u8>,
        refresh: bool,
    ) -> eyre::Result<(Attempt, Option<String>)> {
        self.run("cache.identity", move |db| {
            db.immediate_transaction(|db| {
                diesel::sql_query(
                    "UPDATE printer_requests SET serial=?, firmware=? WHERE request_id=?",
                )
                .bind::<Text, _>(identity.serial)
                .bind::<Text, _>(identity.firmware)
                .bind::<BigInt, _>(attempt.request_id)
                .execute(db)?;
                diesel::update(
                    png_requests::table.filter(png_requests::rowid.eq(attempt.request_id)),
                )
                .set(png_requests::renderer_key.eq(&key))
                .execute(db)?;
                attempt.renderer_key = key;
                let error = diesel::sql_query(
                    "SELECT error FROM permanent_errors WHERE input_id=? AND renderer_key=?",
                )
                .bind::<BigInt, _>(attempt.input_id)
                .bind::<Binary, _>(&attempt.renderer_key)
                .get_result::<ErrorRow>(db)
                .optional()?
                .map(|row| row.error);
                let cached = if refresh || error.is_some() {
                    None
                } else {
                    render_cache::table
                        .inner_join(pngs::table)
                        .filter(render_cache::input_id.eq(attempt.input_id))
                        .filter(render_cache::renderer_key.eq(&attempt.renderer_key))
                        .select((pngs::id, pngs::data, render_cache::printer_identity))
                        .first::<(i64, Vec<u8>, String)>(db)
                        .optional()?
                };
                if cached.is_some() || error.is_some() {
                    diesel::update(
                        png_requests::table.filter(png_requests::rowid.eq(attempt.request_id)),
                    )
                    .set((
                        png_requests::png_id.eq(cached.as_ref().map(|(id, _, _)| *id)),
                        png_requests::error.eq(&error),
                        png_requests::cache_hit.eq(true),
                        png_requests::printer_identity
                            .eq(cached.as_ref().map(|(_, _, identity)| identity)),
                        png_requests::completed_at.eq(diesel::dsl::sql::<Nullable<Text>>(
                            "strftime('%Y-%m-%dT%H:%M:%fZ','now')",
                        )),
                    ))
                    .execute(db)?;
                }
                attempt.cached = cached
                    .map(|(_, png, identity)| {
                        Ok::<_, eyre::Report>(CachedRender {
                            png,
                            identity: serde_json::from_str(&identity)?,
                        })
                    })
                    .transpose()?;
                Ok((attempt, error))
            })
        })
        .await
    }

    pub async fn preview_started(
        &self,
        request_id: i64,
        name: String,
        identity: Identity,
        phase: &'static str,
    ) -> eyre::Result<i64> {
        self.run("cache.preview.start", move |db| {
            diesel::sql_query("INSERT INTO preview_attempts(request_id,name,serial,firmware,phase) VALUES(?,?,?,?,?)")
                .bind::<BigInt,_>(request_id).bind::<Text,_>(name)
                .bind::<Text,_>(identity.serial).bind::<Text,_>(identity.firmware)
                .bind::<Text,_>(phase).execute(db)?;
            Ok(diesel::select(diesel::dsl::sql::<BigInt>("last_insert_rowid()")).get_result(db)?)
        }).await
    }

    pub async fn preview_finished(
        &self,
        id: i64,
        error: Option<String>,
        failure_kind: Option<&'static str>,
    ) -> eyre::Result<()> {
        self.run("cache.preview.finish", move |db| {
            diesel::sql_query("UPDATE preview_attempts SET completed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),error=?,failure_kind=? WHERE id=?")
                .bind::<Nullable<Text>,_>(error).bind::<Nullable<Text>,_>(failure_kind).bind::<BigInt,_>(id).execute(db)?;
            Ok(())
        }).await
    }

    pub async fn quarantine(&self, attempt: &Attempt, error: String) -> eyre::Result<()> {
        let attempt = attempt.clone();
        self.run("cache.quarantine", move |db| {
            db.immediate_transaction(|db| {
                diesel::sql_query("INSERT OR REPLACE INTO permanent_errors(input_id,renderer_key,error) VALUES(?,?,?)")
                    .bind::<BigInt,_>(attempt.input_id).bind::<Binary,_>(&attempt.renderer_key)
                    .bind::<Text,_>(error).execute(db)?;
                diesel::delete(render_cache::table.filter(render_cache::input_id.eq(attempt.input_id))
                    .filter(render_cache::renderer_key.eq(&attempt.renderer_key))).execute(db)?;
                Ok(())
            })
        }).await
    }
}
