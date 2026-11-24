use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{ApiError, ApiResult};

/// Uma única conexão SQLite (WAL) protegida por `Mutex`; todo acesso roda em `spawn_blocking`.
#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Db> {
        let mut conn = Connection::open(path)?;
        let mode: String = conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            tracing::warn!("SQLite não aceitou WAL (modo atual: {mode})");
        }
        conn.execute_batch(
            "PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;",
        )?;
        migrate(&mut conn)?;
        Ok(Db(Arc::new(Mutex::new(conn))))
    }

    pub async fn call<F, T>(&self, f: F) -> ApiResult<T>
    where
        F: FnOnce(&mut Connection) -> ApiResult<T> + Send + 'static,
        T: Send + 'static,
    {
        let db = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = db.lock().unwrap_or_else(|e| e.into_inner());
            f(&mut conn)
        })
        .await
        .map_err(|e| {
            tracing::error!("tarefa do banco falhou: {e}");
            ApiError::internal("Erro interno")
        })?
    }

    /// Acesso síncrono (uso na inicialização).
    pub fn blocking<T>(&self, f: impl FnOnce(&mut Connection) -> T) -> T {
        let mut conn = self.0.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut conn)
    }
}

/// Migrações sequenciais; `PRAGMA user_version` guarda quantas já foram aplicadas.
const MIGRATIONS: &[&str] = &[
    // v1 — esquema inicial
    r#"
    CREATE TABLE crew (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        name        TEXT NOT NULL,
        emoji       TEXT NOT NULL DEFAULT '',
        created_at  TEXT NOT NULL
    );

    CREATE TABLE settings (
        key    TEXT PRIMARY KEY,
        value  TEXT NOT NULL
    );

    CREATE TABLE destinations (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        name            TEXT NOT NULL,
        lat             REAL NOT NULL,
        lon             REAL NOT NULL,
        photo           TEXT,
        photo_original  TEXT,
        good_winds      TEXT NOT NULL DEFAULT '[]',
        created_by      TEXT,
        created_at      TEXT NOT NULL
    );

    CREATE TABLE votes (
        device_id       TEXT PRIMARY KEY,
        destination_id  INTEGER NOT NULL REFERENCES destinations(id) ON DELETE CASCADE,
        created_at      TEXT NOT NULL
    );
    CREATE INDEX votes_destination ON votes(destination_id);

    CREATE TABLE positions (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        device_id   TEXT,
        lat         REAL NOT NULL,
        lon         REAL NOT NULL,
        accuracy_m  REAL,
        speed_mps   REAL,
        heading     REAL,
        speed_kn    REAL,
        at          TEXT NOT NULL
    );
    CREATE INDEX positions_at ON positions(at);

    CREATE TABLE tags (
        id       INTEGER PRIMARY KEY AUTOINCREMENT,
        name     TEXT NOT NULL,
        emoji    TEXT,
        unit     TEXT NOT NULL,
        min_qty  REAL
    );

    CREATE TABLE items (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        name            TEXT NOT NULL,
        photo           TEXT,
        photo_original  TEXT,
        count_unit      TEXT NOT NULL DEFAULT 'un',
        content_amount  REAL,
        content_unit    TEXT,
        quantity        REAL NOT NULL DEFAULT 0,
        min_qty         REAL,
        fractional      INTEGER NOT NULL DEFAULT 0,
        created_at      TEXT NOT NULL,
        updated_at      TEXT NOT NULL,
        updated_by      TEXT
    );

    CREATE TABLE item_tags (
        item_id  INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
        tag_id   INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
        PRIMARY KEY (item_id, tag_id)
    );
    CREATE INDEX item_tags_tag ON item_tags(tag_id);

    CREATE TABLE movements (
        id       INTEGER PRIMARY KEY AUTOINCREMENT,
        item_id  INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
        at       TEXT NOT NULL,
        by_name  TEXT,
        reason   TEXT NOT NULL,
        before   REAL NOT NULL,
        after    REAL NOT NULL
    );
    CREATE INDEX movements_item ON movements(item_id, id);

    CREATE TABLE menu_items (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        name         TEXT NOT NULL,
        category     TEXT NOT NULL,
        description  TEXT NOT NULL DEFAULT '',
        recipe       TEXT NOT NULL DEFAULT '',
        photo        TEXT,
        available    INTEGER NOT NULL DEFAULT 1,
        sort         INTEGER NOT NULL DEFAULT 0,
        created_at   TEXT NOT NULL
    );

    CREATE TABLE orders (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        menu_item_id  INTEGER NOT NULL,
        item_name     TEXT NOT NULL,
        item_photo    TEXT,
        category      TEXT NOT NULL,
        for_name      TEXT NOT NULL DEFAULT '',
        notes         TEXT NOT NULL DEFAULT '',
        status        TEXT NOT NULL,
        handled_by    TEXT,
        device_id     TEXT,
        created_at    TEXT NOT NULL,
        updated_at    TEXT NOT NULL
    );
    CREATE INDEX orders_created ON orders(created_at);
    CREATE INDEX orders_device ON orders(device_id, created_at);

    CREATE TABLE push_subs (
        endpoint    TEXT PRIMARY KEY,
        p256dh      TEXT NOT NULL,
        auth        TEXT NOT NULL,
        device_id   TEXT,
        crew_id     INTEGER,
        created_at  TEXT NOT NULL,
        updated_at  TEXT NOT NULL
    );
    CREATE INDEX push_subs_device ON push_subs(device_id);
    "#,
    "CREATE TABLE crew_activity (
        id INTEGER PRIMARY KEY, at TEXT NOT NULL,
        crew_id INTEGER NOT NULL, crew_name TEXT NOT NULL,
        method TEXT NOT NULL, path TEXT NOT NULL
    );",
];

fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current.max(0) as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
        tracing::info!("migração do banco aplicada: v{}", i + 1);
    }
    Ok(())
}
