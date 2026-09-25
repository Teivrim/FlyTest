use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
use flate2::read::GzDecoder;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::flywire::{
    CellStats, CellTypeRow, Classification, ConnectionPage, ConnectionRow, ConnectivityTagRow,
    Coordinate, Direction, Label, Neuron, NeuronDetails, NeuronDirection,
};

pub const DEFAULT_DB_NAME: &str = "flytest.sqlite";
pub const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub database: String,
    pub dataset: String,
    pub files_imported: usize,
    pub neuron_rows: u64,
    pub connection_rows: u64,
    pub synapse_count: u64,
    pub database_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkStats {
    pub database: String,
    pub neuron_count: u64,
    pub connection_rows: u64,
    pub unique_neuron_pairs: u64,
    pub synapse_count: u64,
    pub reciprocal_pairs: u64,
    pub average_raw_degree: f64,
    pub average_unique_degree: f64,
    pub max_out_degree: u64,
    pub max_in_degree: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RankedNeuron {
    pub root_id: u64,
    pub group: String,
    pub nt_type: String,
    pub out_degree: u64,
    pub in_degree: u64,
    pub out_synapses: u64,
    pub in_synapses: u64,
    pub total_synapses: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexedNeighbor {
    pub pre_root_id: u64,
    pub post_root_id: u64,
    pub neuropil: String,
    pub syn_count: u64,
    pub nt_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub root_id: u64,
    pub group: String,
    pub nt_type: String,
    pub matched_text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NblastHit {
    pub target_root_id: u64,
    pub score: u8,
}

#[derive(Debug, Deserialize)]
struct NblastRow {
    root_id: u64,
    scores: String,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS neurons (
    root_id INTEGER PRIMARY KEY,
    group_name TEXT NOT NULL,
    nt_type TEXT NOT NULL,
    nt_type_score REAL NOT NULL,
    da_avg REAL NOT NULL,
    ser_avg REAL NOT NULL,
    gaba_avg REAL NOT NULL,
    glut_avg REAL NOT NULL,
    ach_avg REAL NOT NULL,
    oct_avg REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS classification (
    root_id INTEGER PRIMARY KEY,
    flow TEXT NOT NULL,
    super_class TEXT NOT NULL,
    class TEXT NOT NULL,
    sub_class TEXT NOT NULL,
    hemilineage TEXT NOT NULL,
    side TEXT NOT NULL,
    nerve TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cell_types (
    root_id INTEGER NOT NULL,
    cell_type TEXT NOT NULL,
    is_primary INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS connectivity_tags (
    root_id INTEGER NOT NULL,
    tag TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cell_stats (
    root_id INTEGER PRIMARY KEY,
    length_nm INTEGER NOT NULL,
    area_nm INTEGER NOT NULL,
    size_nm INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS connections (
    pre_root_id INTEGER NOT NULL,
    post_root_id INTEGER NOT NULL,
    neuropil TEXT NOT NULL,
    syn_count INTEGER NOT NULL,
    nt_type TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS edge_pairs (
    pre_root_id INTEGER NOT NULL,
    post_root_id INTEGER NOT NULL,
    syn_count INTEGER NOT NULL,
    row_count INTEGER NOT NULL,
    PRIMARY KEY (pre_root_id, post_root_id)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS labels (
    root_id INTEGER NOT NULL,
    label TEXT NOT NULL,
    user_id TEXT NOT NULL,
    position TEXT NOT NULL,
    supervoxel_id INTEGER NOT NULL,
    label_id TEXT NOT NULL,
    date_created TEXT NOT NULL,
    user_name TEXT NOT NULL,
    user_affiliation TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS coordinates (
    root_id INTEGER NOT NULL,
    position TEXT NOT NULL,
    supervoxel_id INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS nblast (
    root_id INTEGER PRIMARY KEY,
    scores TEXT NOT NULL
);
"#;

const INDEXES: &str = r#"
CREATE INDEX IF NOT EXISTS connections_pre_idx ON connections(pre_root_id);
CREATE INDEX IF NOT EXISTS connections_post_idx ON connections(post_root_id);
CREATE INDEX IF NOT EXISTS connections_neuropil_idx ON connections(neuropil);
CREATE INDEX IF NOT EXISTS edge_pairs_post_idx ON edge_pairs(post_root_id);
CREATE INDEX IF NOT EXISTS cell_types_root_idx ON cell_types(root_id);
CREATE INDEX IF NOT EXISTS tags_root_idx ON connectivity_tags(root_id);
CREATE INDEX IF NOT EXISTS labels_root_idx ON labels(root_id);
CREATE INDEX IF NOT EXISTS coordinates_root_idx ON coordinates(root_id);
"#;

pub fn default_db_path(data_dir: &Path) -> PathBuf {
    data_dir.join(DEFAULT_DB_NAME)
}

pub fn open_db(path: &Path) -> Result<Connection> {
    Connection::open(path).with_context(|| format!("opening SQLite database {}", path.display()))
}

pub fn validate_index(database: &Path, data_dir: &Path) -> Result<()> {
    let conn = open_db(database)?;
    let schema_version: String = conn.query_row(
        "SELECT value FROM metadata WHERE key = 'schema_version'",
        [],
        |row| row.get(0),
    )?;
    ensure!(
        schema_version == SCHEMA_VERSION.to_string(),
        "index schema version {} does not match expected {}",
        schema_version,
        SCHEMA_VERSION
    );
    let stored_manifest: String = conn.query_row(
        "SELECT value FROM metadata WHERE key = 'manifest_sha256'",
        [],
        |row| row.get(0),
    )?;
    let current_manifest = crate::flywire::manifest_sha256(data_dir)?;
    ensure!(
        stored_manifest == current_manifest,
        "SQLite index is stale: manifest changed; rebuild with `neurohub import --force`"
    );
    Ok(())
}

fn sql_id(id: u64) -> Result<i64> {
    i64::try_from(id).with_context(|| format!("FlyWire ID {id} does not fit in SQLite INTEGER"))
}

fn from_sql_id(id: i64) -> rusqlite::Result<u64> {
    u64::try_from(id).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn remove_database(path: &Path) -> Result<()> {
    for suffix in ["", "-wal", "-shm"] {
        let candidate = PathBuf::from(format!("{}{}", path.display(), suffix));
        if candidate.exists() {
            fs::remove_file(&candidate)
                .with_context(|| format!("removing old database file {}", candidate.display()))?;
        }
    }
    Ok(())
}

fn for_each_record<T, F>(path: &Path, mut callback: F) -> Result<()>
where
    T: DeserializeOwned,
    F: FnMut(T) -> Result<()>,
{
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(GzDecoder::new(file));
    let headers = reader
        .headers()
        .with_context(|| format!("reading headers from {}", path.display()))?
        .clone();
    for record in reader.records() {
        let record = record.with_context(|| format!("reading {}", path.display()))?;
        let value: T = record
            .deserialize(Some(&headers))
            .with_context(|| format!("decoding {}", path.display()))?;
        callback(value)?;
    }
    Ok(())
}

fn split_values(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub fn import_dataset(data_dir: &Path, database: &Path, force: bool) -> Result<ImportReport> {
    ensure!(
        data_dir.is_dir(),
        "data directory does not exist: {}",
        data_dir.display()
    );
    let manifest = crate::flywire::read_manifest(data_dir)?;
    let manifest_sha256 = crate::flywire::manifest_sha256(data_dir)?;
    ensure!(
        !database.exists() || force,
        "database already exists: {} (use --force)",
        database.display()
    );
    if let Some(parent) = database
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating database directory {}", parent.display()))?;
    }
    let build_path = database.with_file_name(format!(
        ".{}.building-{}",
        database
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("neurohub.sqlite"),
        process::id()
    ));
    remove_database(&build_path)?;

    let mut conn = open_db(&build_path)?;
    conn.execute_batch(
        "PRAGMA journal_mode = DELETE; PRAGMA synchronous = OFF; PRAGMA temp_store = MEMORY;",
    )?;
    conn.execute_batch(SCHEMA)?;

    let tx = conn.transaction()?;
    let mut neuron_rows = 0_u64;
    let mut connection_rows = 0_u64;
    let mut synapse_count = 0_u64;

    for_each_record::<Neuron, _>(&data_dir.join("neurons.csv.gz"), |row| {
        tx.execute(
            "INSERT INTO neurons
             (root_id, group_name, nt_type, nt_type_score, da_avg, ser_avg, gaba_avg, glut_avg, ach_avg, oct_avg)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                sql_id(row.root_id)?,
                row.group,
                row.nt_type,
                row.nt_type_score,
                row.da_avg,
                row.ser_avg,
                row.gaba_avg,
                row.glut_avg,
                row.ach_avg,
                row.oct_avg,
            ],
        )?;
        neuron_rows += 1;
        Ok(())
    })?;

    for_each_record::<Classification, _>(&data_dir.join("classification.csv.gz"), |row| {
        tx.execute(
            "INSERT INTO classification
                 (root_id, flow, super_class, class, sub_class, hemilineage, side, nerve)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                sql_id(row.root_id)?,
                row.flow,
                row.super_class,
                row.class,
                row.sub_class,
                row.hemilineage,
                row.side,
                row.nerve,
            ],
        )?;
        Ok(())
    })?;

    for_each_record::<CellTypeRow, _>(&data_dir.join("consolidated_cell_types.csv.gz"), |row| {
        if !row.primary_type.is_empty() {
            tx.execute(
                "INSERT INTO cell_types (root_id, cell_type, is_primary) VALUES (?1, ?2, 1)",
                params![sql_id(row.root_id)?, row.primary_type],
            )?;
        }
        for cell_type in split_values(&row.additional_types) {
            tx.execute(
                "INSERT INTO cell_types (root_id, cell_type, is_primary) VALUES (?1, ?2, 0)",
                params![sql_id(row.root_id)?, cell_type],
            )?;
        }
        Ok(())
    })?;

    for_each_record::<ConnectivityTagRow, _>(&data_dir.join("connectivity_tags.csv.gz"), |row| {
        for tag in split_values(&row.connectivity_tag) {
            tx.execute(
                "INSERT INTO connectivity_tags (root_id, tag) VALUES (?1, ?2)",
                params![sql_id(row.root_id)?, tag],
            )?;
        }
        Ok(())
    })?;

    for_each_record::<CellStats, _>(&data_dir.join("cell_stats.csv.gz"), |row| {
        tx.execute(
            "INSERT INTO cell_stats (root_id, length_nm, area_nm, size_nm)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                sql_id(row.root_id)?,
                sql_id(row.length_nm)?,
                sql_id(row.area_nm)?,
                sql_id(row.size_nm)?,
            ],
        )?;
        Ok(())
    })?;

    for_each_record::<ConnectionRow, _>(&data_dir.join("connections.csv.gz"), |row| {
        let syn_count = sql_id(row.syn_count)?;
        tx.execute(
            "INSERT INTO connections
                 (pre_root_id, post_root_id, neuropil, syn_count, nt_type)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                sql_id(row.pre_root_id)?,
                sql_id(row.post_root_id)?,
                row.neuropil,
                syn_count,
                row.nt_type,
            ],
        )?;
        connection_rows += 1;
        synapse_count += row.syn_count;
        Ok(())
    })?;

    for_each_record::<Label, _>(&data_dir.join("labels.csv.gz"), |row| {
        tx.execute(
            "INSERT INTO labels
             (root_id, label, user_id, position, supervoxel_id, label_id, date_created, user_name, user_affiliation)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                sql_id(row.root_id)?,
                row.label,
                row.user_id,
                row.position,
                sql_id(row.supervoxel_id)?,
                row.label_id,
                row.date_created,
                row.user_name,
                row.user_affiliation,
            ],
        )?;
        Ok(())
    })?;

    for_each_record::<Coordinate, _>(&data_dir.join("coordinates.csv.gz"), |row| {
        tx.execute(
            "INSERT INTO coordinates (root_id, position, supervoxel_id) VALUES (?1, ?2, ?3)",
            params![
                sql_id(row.root_id)?,
                row.position,
                sql_id(row.supervoxel_id)?
            ],
        )?;
        Ok(())
    })?;

    for_each_record::<NblastRow, _>(&data_dir.join("nblast.csv.gz"), |row| {
        tx.execute(
            "INSERT INTO nblast (root_id, scores) VALUES (?1, ?2)",
            params![sql_id(row.root_id)?, row.scores],
        )?;
        Ok(())
    })?;

    let imported_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "unknown".to_owned());
    for (key, value) in [
        ("schema_version", SCHEMA_VERSION.to_string()),
        ("dataset", manifest.dataset.clone()),
        ("snapshot", manifest.snapshot.clone()),
        ("source", manifest.source.clone()),
        ("manifest_sha256", manifest_sha256),
        ("neuron_rows", neuron_rows.to_string()),
        ("connection_rows", connection_rows.to_string()),
        ("synapse_count", synapse_count.to_string()),
        ("imported_at_unix", imported_at),
    ] {
        tx.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
    }
    tx.commit()?;

    conn.execute_batch(
        "INSERT INTO edge_pairs (pre_root_id, post_root_id, syn_count, row_count)
         SELECT pre_root_id, post_root_id, SUM(syn_count), COUNT(*)
         FROM connections
         GROUP BY pre_root_id, post_root_id;",
    )?;
    conn.execute_batch(INDEXES)?;
    conn.execute_batch("ANALYZE; PRAGMA optimize;")?;
    drop(conn);

    if database.exists() {
        remove_database(database)?;
    }
    fs::rename(&build_path, database).with_context(|| {
        format!(
            "installing completed index {} as {}",
            build_path.display(),
            database.display()
        )
    })?;
    let database_bytes = fs::metadata(database)?.len();
    Ok(ImportReport {
        database: database.display().to_string(),
        dataset: manifest.dataset,
        files_imported: 9,
        neuron_rows,
        connection_rows,
        synapse_count,
        database_bytes,
    })
}

pub fn stats(database: &Path) -> Result<NetworkStats> {
    let conn = open_db(database)?;
    let neuron_count = scalar_u64(&conn, "SELECT COUNT(*) FROM neurons")?;
    let connection_rows = scalar_u64(&conn, "SELECT COUNT(*) FROM connections")?;
    let synapse_count = scalar_u64(&conn, "SELECT COALESCE(SUM(syn_count), 0) FROM connections")?;
    let unique_neuron_pairs = scalar_u64(&conn, "SELECT COUNT(*) FROM edge_pairs")?;
    let reciprocal_pairs = scalar_u64(
        &conn,
        "SELECT COUNT(*) FROM edge_pairs forward
         JOIN edge_pairs reverse_edge
           ON reverse_edge.pre_root_id = forward.post_root_id
          AND reverse_edge.post_root_id = forward.pre_root_id
         WHERE forward.pre_root_id <= forward.post_root_id",
    )?;
    let max_out_degree = scalar_u64(
        &conn,
        "SELECT COALESCE(MAX(degree), 0) FROM (SELECT pre_root_id, COUNT(*) AS degree FROM edge_pairs GROUP BY pre_root_id)",
    )?;
    let max_in_degree = scalar_u64(
        &conn,
        "SELECT COALESCE(MAX(degree), 0) FROM (SELECT post_root_id, COUNT(*) AS degree FROM edge_pairs GROUP BY post_root_id)",
    )?;
    let average_raw_degree = if neuron_count == 0 {
        0.0
    } else {
        connection_rows as f64 / neuron_count as f64
    };
    let average_unique_degree = if neuron_count == 0 {
        0.0
    } else {
        unique_neuron_pairs as f64 / neuron_count as f64
    };

    Ok(NetworkStats {
        database: database.display().to_string(),
        neuron_count,
        connection_rows,
        unique_neuron_pairs,
        synapse_count,
        reciprocal_pairs,
        average_raw_degree,
        average_unique_degree,
        max_out_degree,
        max_in_degree,
    })
}

pub fn top_neurons(database: &Path, limit: usize, by_synapses: bool) -> Result<Vec<RankedNeuron>> {
    let conn = open_db(database)?;
    let order = if by_synapses {
        "8 DESC, 4 + 5 DESC, 1"
    } else {
        "4 + 5 DESC, 7 DESC, 1"
    };
    let sql = format!(
        "SELECT n.root_id, n.group_name, n.nt_type,
                COALESCE(o.degree, 0), COALESCE(i.degree, 0),
                COALESCE(o.synapses, 0), COALESCE(i.synapses, 0),
                COALESCE(o.synapses, 0) + COALESCE(i.synapses, 0) AS total_synapses
         FROM neurons n
         LEFT JOIN (SELECT pre_root_id, COUNT(*) AS degree, SUM(syn_count) AS synapses
                    FROM edge_pairs GROUP BY pre_root_id) o ON o.pre_root_id = n.root_id
         LEFT JOIN (SELECT post_root_id, COUNT(*) AS degree, SUM(syn_count) AS synapses
                    FROM edge_pairs GROUP BY post_root_id) i ON i.post_root_id = n.root_id
         ORDER BY {order}
         LIMIT ?1"
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
        let root_id: i64 = row.get(0)?;
        let out_degree: i64 = row.get(3)?;
        let in_degree: i64 = row.get(4)?;
        let out_synapses: i64 = row.get(5)?;
        let in_synapses: i64 = row.get(6)?;
        Ok(RankedNeuron {
            root_id: from_sql_id(root_id)?,
            group: row.get(1)?,
            nt_type: row.get(2)?,
            out_degree: nonnegative_u64(out_degree)?,
            in_degree: nonnegative_u64(in_degree)?,
            out_synapses: nonnegative_u64(out_synapses)?,
            in_synapses: nonnegative_u64(in_synapses)?,
            total_synapses: nonnegative_u64(out_synapses + in_synapses)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(anyhow::Error::from)
}

pub fn search(database: &Path, query: &str, field: &str, limit: usize) -> Result<Vec<SearchHit>> {
    let predicate = match field {
        "all" => {
            "LOWER(COALESCE(n.group_name, '')) LIKE ?1 OR LOWER(COALESCE(n.nt_type, '')) LIKE ?1 OR LOWER(COALESCE(c.flow, '')) LIKE ?1 OR LOWER(COALESCE(c.super_class, '')) LIKE ?1 OR LOWER(COALESCE(c.class, '')) LIKE ?1 OR LOWER(COALESCE(c.sub_class, '')) LIKE ?1 OR EXISTS (SELECT 1 FROM cell_types ct WHERE ct.root_id = n.root_id AND LOWER(ct.cell_type) LIKE ?1) OR EXISTS (SELECT 1 FROM labels l WHERE l.root_id = n.root_id AND LOWER(l.label) LIKE ?1) OR EXISTS (SELECT 1 FROM connectivity_tags t WHERE t.root_id = n.root_id AND LOWER(t.tag) LIKE ?1)"
        }
        "label" => {
            "EXISTS (SELECT 1 FROM labels l WHERE l.root_id = n.root_id AND LOWER(l.label) LIKE ?1)"
        }
        "cell_type" | "cell-type" => {
            "EXISTS (SELECT 1 FROM cell_types ct WHERE ct.root_id = n.root_id AND LOWER(ct.cell_type) LIKE ?1)"
        }
        "classification" => {
            "LOWER(COALESCE(c.flow, '')) LIKE ?1 OR LOWER(COALESCE(c.super_class, '')) LIKE ?1 OR LOWER(COALESCE(c.class, '')) LIKE ?1 OR LOWER(COALESCE(c.sub_class, '')) LIKE ?1"
        }
        "tag" => {
            "EXISTS (SELECT 1 FROM connectivity_tags t WHERE t.root_id = n.root_id AND LOWER(t.tag) LIKE ?1)"
        }
        "group" => "LOWER(COALESCE(n.group_name, '')) LIKE ?1",
        "neurotransmitter" => "LOWER(COALESCE(n.nt_type, '')) LIKE ?1",
        other => bail!("unknown search field: {other}"),
    };
    let pattern = format!("%{}%", query.trim().to_lowercase());
    let limit = i64::try_from(limit.min(10_000)).unwrap_or(i64::MAX);
    let sql = format!(
        "SELECT n.root_id, n.group_name, n.nt_type, COALESCE(c.flow, ''), COALESCE(c.class, ''),
                COALESCE((SELECT group_concat(label, ' | ') FROM labels WHERE root_id = n.root_id), '')
         FROM neurons n
         LEFT JOIN classification c ON c.root_id = n.root_id
         WHERE {predicate}
         ORDER BY n.root_id
         LIMIT ?2"
    );
    let conn = open_db(database)?;
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map(params![pattern, limit], |row| {
        let root_id: i64 = row.get(0)?;
        let group: String = row.get(1)?;
        let nt_type: String = row.get(2)?;
        let flow: String = row.get(3)?;
        let class: String = row.get(4)?;
        let labels: String = row.get(5)?;
        Ok(SearchHit {
            root_id: from_sql_id(root_id)?,
            matched_text: format!("{group} | {nt_type} | {flow} | {class} | {labels}"),
            group,
            nt_type,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(anyhow::Error::from)
}

pub fn nblast(database: &Path, root_id: u64, limit: usize) -> Result<Vec<NblastHit>> {
    let conn = open_db(database)?;
    let id = sql_id(root_id)?;
    let scores: Option<String> = conn
        .query_row(
            "SELECT scores FROM nblast WHERE root_id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(scores) = scores else {
        return Ok(Vec::new());
    };
    let mut hits = Vec::new();
    for item in scores.split(';').filter(|item| !item.trim().is_empty()) {
        let (target, score) = item
            .split_once(':')
            .with_context(|| format!("invalid nblast score: {item}"))?;
        let target_root_id = target
            .parse::<u64>()
            .with_context(|| format!("invalid nblast target: {target}"))?;
        let score = score
            .parse::<u8>()
            .with_context(|| format!("invalid nblast score: {score}"))?;
        hits.push(NblastHit {
            target_root_id,
            score,
        });
    }
    hits.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.target_root_id.cmp(&right.target_root_id))
    });
    hits.truncate(limit.min(10_000));
    Ok(hits)
}

pub fn load_neuron(
    database: &Path,
    root_id: u64,
    direction: NeuronDirection,
    limit: usize,
    min_synapses: u64,
) -> Result<Option<NeuronDetails>> {
    let conn = open_db(database)?;
    let id = sql_id(root_id)?;
    let neuron = conn
        .query_row(
            "SELECT root_id, group_name, nt_type, nt_type_score, da_avg, ser_avg, gaba_avg, glut_avg, ach_avg, oct_avg
             FROM neurons WHERE root_id = ?1",
            [id],
            |row| {
                Ok(Neuron {
                    root_id: from_sql_id(row.get(0)?)?,
                    group: row.get(1)?,
                    nt_type: row.get(2)?,
                    nt_type_score: row.get(3)?,
                    da_avg: row.get(4)?,
                    ser_avg: row.get(5)?,
                    gaba_avg: row.get(6)?,
                    glut_avg: row.get(7)?,
                    ach_avg: row.get(8)?,
                    oct_avg: row.get(9)?,
                })
            },
        )
        .optional()?;
    let Some(neuron) = neuron else {
        return Ok(None);
    };

    let classification = conn
        .query_row(
            "SELECT root_id, flow, super_class, class, sub_class, hemilineage, side, nerve
             FROM classification WHERE root_id = ?1",
            [id],
            |row| {
                Ok(Classification {
                    root_id: from_sql_id(row.get(0)?)?,
                    flow: row.get(1)?,
                    super_class: row.get(2)?,
                    class: row.get(3)?,
                    sub_class: row.get(4)?,
                    hemilineage: row.get(5)?,
                    side: row.get(6)?,
                    nerve: row.get(7)?,
                })
            },
        )
        .optional()?;
    let cell_types = query_strings(
        &conn,
        "SELECT cell_type FROM cell_types WHERE root_id = ?1 ORDER BY is_primary DESC, cell_type",
        id,
    )?;
    let connectivity_tags = query_strings(
        &conn,
        "SELECT tag FROM connectivity_tags WHERE root_id = ?1 ORDER BY tag",
        id,
    )?;
    let cell_stats = conn
        .query_row(
            "SELECT root_id, length_nm, area_nm, size_nm FROM cell_stats WHERE root_id = ?1",
            [id],
            |row| {
                Ok(CellStats {
                    root_id: from_sql_id(row.get(0)?)?,
                    length_nm: nonnegative_u64(row.get(1)?)?,
                    area_nm: nonnegative_u64(row.get(2)?)?,
                    size_nm: nonnegative_u64(row.get(3)?)?,
                })
            },
        )
        .optional()?;
    let labels = query_labels(&conn, id)?;
    let coordinates = query_coordinates(&conn, id)?;
    let connections = query_connections(&conn, root_id, direction, limit, min_synapses)?;

    Ok(Some(NeuronDetails {
        neuron,
        classification,
        cell_types,
        cell_stats,
        connectivity_tags,
        labels,
        coordinates,
        connections,
    }))
}

fn scalar_u64(conn: &Connection, sql: &str) -> Result<u64> {
    let value: i64 = conn.query_row(sql, [], |row| row.get(0))?;
    Ok(nonnegative_u64(value)?)
}

fn nonnegative_u64(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn query_strings(conn: &Connection, sql: &str, id: i64) -> Result<Vec<String>> {
    let mut statement = conn.prepare(sql)?;
    let rows = statement.query_map([id], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<String>>>()
        .map_err(anyhow::Error::from)
}

fn query_labels(conn: &Connection, id: i64) -> Result<Vec<Label>> {
    let mut statement = conn.prepare(
        "SELECT root_id, label, user_id, position, supervoxel_id, label_id, date_created, user_name, user_affiliation
         FROM labels WHERE root_id = ?1 ORDER BY date_created",
    )?;
    let rows = statement.query_map([id], |row| {
        Ok(Label {
            root_id: from_sql_id(row.get(0)?)?,
            label: row.get(1)?,
            user_id: row.get(2)?,
            position: row.get(3)?,
            supervoxel_id: from_sql_id(row.get(4)?)?,
            label_id: row.get(5)?,
            date_created: row.get(6)?,
            user_name: row.get(7)?,
            user_affiliation: row.get(8)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(anyhow::Error::from)
}

fn query_coordinates(conn: &Connection, id: i64) -> Result<Vec<Coordinate>> {
    let mut statement = conn
        .prepare("SELECT root_id, position, supervoxel_id FROM coordinates WHERE root_id = ?1")?;
    let rows = statement.query_map([id], |row| {
        Ok(Coordinate {
            root_id: from_sql_id(row.get(0)?)?,
            position: row.get(1)?,
            supervoxel_id: from_sql_id(row.get(2)?)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(anyhow::Error::from)
}

pub fn query_connections(
    conn: &Connection,
    root_id: u64,
    direction: NeuronDirection,
    limit: usize,
    min_synapses: u64,
) -> Result<ConnectionPage> {
    let id = sql_id(root_id)?;
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let min_synapses_sql = i64::try_from(min_synapses).unwrap_or(i64::MAX);
    let (where_clause, partner_expression, direction) = match direction {
        NeuronDirection::Incoming => ("post_root_id = ?1", "pre_root_id", Direction::Incoming),
        NeuronDirection::Outgoing => ("pre_root_id = ?1", "post_root_id", Direction::Outgoing),
        NeuronDirection::Both => (
            "(pre_root_id = ?1 OR post_root_id = ?1)",
            "CASE WHEN pre_root_id = ?1 THEN post_root_id ELSE pre_root_id END",
            Direction::Both,
        ),
    };
    let filter = format!("{where_clause} AND syn_count >= ?2");
    let count_sql = format!("SELECT COUNT(*) FROM connections WHERE {filter}");
    let matched: i64 =
        conn.query_row(&count_sql, params![id, min_synapses_sql], |row| row.get(0))?;
    let matched = nonnegative_u64(matched)?;

    let unique_sql = format!(
        "SELECT COUNT(*) FROM (SELECT {partner_expression} AS partner
         FROM connections WHERE {filter} GROUP BY partner)"
    );
    let unique_partners: i64 =
        conn.query_row(&unique_sql, params![id, min_synapses_sql], |row| row.get(0))?;
    let unique_partners = nonnegative_u64(unique_partners)?;

    let total_sql = format!("SELECT COALESCE(SUM(syn_count), 0) FROM connections WHERE {filter}");
    let total_synapses: i64 =
        conn.query_row(&total_sql, params![id, min_synapses_sql], |row| row.get(0))?;
    let total_synapses = nonnegative_u64(total_synapses)?;

    let sql = format!(
        "SELECT pre_root_id, post_root_id, neuropil, syn_count, nt_type
         FROM connections WHERE {filter}
         ORDER BY syn_count DESC, pre_root_id, post_root_id, neuropil
         LIMIT ?3"
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map(params![id, min_synapses_sql, limit], |row| {
        Ok(ConnectionRow {
            pre_root_id: from_sql_id(row.get(0)?)?,
            post_root_id: from_sql_id(row.get(1)?)?,
            neuropil: row.get(2)?,
            syn_count: nonnegative_u64(row.get(3)?)?,
            nt_type: row.get(4)?,
        })
    })?;
    let connections = rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(anyhow::Error::from)?;
    Ok(ConnectionPage {
        root_id,
        direction,
        min_synapses,
        matched,
        unique_partners,
        total_synapses,
        returned: connections.len(),
        truncated: matched > connections.len() as u64,
        connections,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_database(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("neurohub-{name}-{}-{stamp}.sqlite", process::id()))
    }

    fn remove_test_database(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(format!("{}-wal", path.display()));
        let _ = fs::remove_file(format!("{}-shm", path.display()));
    }

    #[test]
    fn connection_filter_reports_aggregates() {
        let path = test_database("connections");
        let conn = Connection::open(&path).expect("open test database");
        conn.execute_batch(SCHEMA).expect("create schema");
        conn.execute_batch(
            "INSERT INTO connections (pre_root_id, post_root_id, neuropil, syn_count, nt_type)
             VALUES (1, 2, 'A', 7, 'GABA'), (1, 2, 'B', 5, 'ACH'), (1, 3, 'A', 2, 'GABA');",
        )
        .expect("insert connections");

        let page = query_connections(&conn, 1, NeuronDirection::Outgoing, 1, 5).expect("query");
        assert_eq!(page.matched, 2);
        assert_eq!(page.unique_partners, 1);
        assert_eq!(page.total_synapses, 12);
        assert_eq!(page.returned, 1);
        assert!(page.truncated);
        assert_eq!(page.min_synapses, 5);

        drop(conn);
        remove_test_database(&path);
    }

    #[test]
    fn nblast_scores_are_sorted() {
        let path = test_database("nblast");
        {
            let conn = Connection::open(&path).expect("open test database");
            conn.execute_batch(SCHEMA).expect("create schema");
            conn.execute_batch("INSERT INTO nblast (root_id, scores) VALUES (1, '2:3;3:9;4:7');")
                .expect("insert nblast");
        }
        let hits = nblast(&path, 1, 10).expect("query nblast");
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].target_root_id, 3);
        assert_eq!(hits[0].score, 9);
        assert_eq!(hits[2].target_root_id, 2);
        remove_test_database(&path);
    }
}
