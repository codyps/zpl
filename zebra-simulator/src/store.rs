use crate::{
    schema::{labels, requests, settings},
    Config,
};
use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use eyre::{eyre, Result};
use serde::Serialize;
use std::collections::BTreeMap;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub struct Store {
    pub db: SqliteConnection,
    pub config: Config,
}

#[derive(Queryable, Selectable, Serialize)]
#[diesel(table_name = requests)]
pub struct Entry {
    pub id: i64,
    pub created_at: String,
    pub connection_id: String,
    pub transport: String,
    pub kind: String,
    pub error: Option<String>,
    pub warnings: String,
    pub label_count: i32,
}

#[derive(Serialize)]
pub struct Detail {
    #[serde(flatten)]
    pub entry: Entry,
    pub text: String,
    pub response_text: String,
    /// Lossless bytes even for binary downloads and invalid UTF-8.
    pub raw: Vec<u8>,
    pub response: Vec<u8>,
}

impl Store {
    pub fn open(path: &str, config: Config) -> Result<Self> {
        config.validate()?;
        let mut db = SqliteConnection::establish(path)?;
        diesel::sql_query("PRAGMA foreign_keys = ON").execute(&mut db)?;
        diesel::sql_query("PRAGMA busy_timeout = 5000").execute(&mut db)?;
        diesel::sql_query("PRAGMA journal_mode = WAL").execute(&mut db)?;
        db.run_pending_migrations(MIGRATIONS)
            .map_err(|e| eyre!("{e}"))?;
        for (name, value) in config.defaults() {
            diesel::insert_into(settings::table)
                .values((settings::name.eq(name), settings::value.eq(value)))
                .on_conflict_do_nothing()
                .execute(&mut db)?;
        }
        Ok(Self { db, config })
    }
    pub fn latest_id(&mut self) -> Result<i64> {
        Ok(requests::table
            .select(diesel::dsl::max(requests::id))
            .first::<Option<i64>>(&mut self.db)?
            .unwrap_or(0))
    }
    pub fn values(&mut self) -> Result<BTreeMap<String, String>> {
        Ok(settings::table
            .select((settings::name, settings::value))
            .load::<(String, String)>(&mut self.db)?
            .into_iter()
            .collect())
    }
    pub fn list(&mut self, after: Option<i64>, before: Option<i64>) -> Result<Vec<Entry>> {
        let mut query = requests::table.into_boxed();
        if let Some(after) = after {
            query = query.filter(requests::id.gt(after));
        }
        if let Some(before) = before {
            query = query.filter(requests::id.lt(before));
        }
        if after.is_some() {
            query = query.order(requests::id.asc());
        } else {
            query = query.order(requests::id.desc());
        }
        Ok(query
            .limit(100)
            .select(Entry::as_select())
            .load(&mut self.db)?)
    }
    pub fn detail(&mut self, id: i64) -> Result<Option<Detail>> {
        let Some(entry) = requests::table
            .find(id)
            .select(Entry::as_select())
            .first(&mut self.db)
            .optional()?
        else {
            return Ok(None);
        };
        let (raw, response): (Vec<u8>, Vec<u8>) = requests::table
            .find(id)
            .select((requests::raw, requests::response))
            .first(&mut self.db)?;
        Ok(Some(Detail {
            text: String::from_utf8_lossy(&raw).into_owned(),
            response_text: String::from_utf8_lossy(&response).into_owned(),
            raw,
            response,
            entry,
        }))
    }
    pub fn image(&mut self, id: i64, page: i32) -> Result<Option<Vec<u8>>> {
        Ok(labels::table
            .find((id, page))
            .select(labels::png)
            .first(&mut self.db)
            .optional()?)
    }
}
