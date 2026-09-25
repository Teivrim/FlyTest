use std::collections::HashSet;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use csv::StringRecord;
use flate2::read::GzDecoder;
use serde::{
    Deserialize, Serialize,
    de::{DeserializeOwned, Deserializer},
};
use sha2::{Digest, Sha256};

pub const DATASET_NAME: &str = "FlyWire FAFB v783";
pub const DEFAULT_DATA_DIR: &str = "data/flywire/fafb-v783";
pub const DATASET_BASE_URL: &str =
    "https://storage.googleapis.com/flywire-data/codex/data/fafb/783";

pub const DATA_FILES: [&str; 9] = [
    "neurons.csv.gz",
    "classification.csv.gz",
    "consolidated_cell_types.csv.gz",
    "cell_stats.csv.gz",
    "connections.csv.gz",
    "labels.csv.gz",
    "coordinates.csv.gz",
    "nblast.csv.gz",
    "connectivity_tags.csv.gz",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub dataset: String,
    pub snapshot: String,
    pub description: String,
    pub source: String,
    pub citation: String,
    pub files: Vec<ManifestFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFile {
    pub name: String,
    pub url: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetSummary {
    pub dataset: String,
    pub snapshot: String,
    pub source: String,
    pub citation: String,
    pub data_dir: String,
    pub files: Vec<FileStats>,
    pub neuron_rows: u64,
    pub connection_rows: u64,
    pub synapse_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStats {
    pub name: String,
    pub bytes: u64,
    pub rows: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationFile {
    pub name: String,
    pub bytes: u64,
    pub rows: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    pub dataset: String,
    pub snapshot: String,
    pub files: Vec<ValidationFile>,
    pub valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub data_dir: String,
    pub verified_files: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Neuron {
    pub root_id: u64,
    pub group: String,
    pub nt_type: String,
    #[serde(deserialize_with = "deserialize_f64_default_zero")]
    pub nt_type_score: f64,
    #[serde(deserialize_with = "deserialize_f64_default_zero")]
    pub da_avg: f64,
    #[serde(deserialize_with = "deserialize_f64_default_zero")]
    pub ser_avg: f64,
    #[serde(deserialize_with = "deserialize_f64_default_zero")]
    pub gaba_avg: f64,
    #[serde(deserialize_with = "deserialize_f64_default_zero")]
    pub glut_avg: f64,
    #[serde(deserialize_with = "deserialize_f64_default_zero")]
    pub ach_avg: f64,
    #[serde(deserialize_with = "deserialize_f64_default_zero")]
    pub oct_avg: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Classification {
    pub root_id: u64,
    pub flow: String,
    pub super_class: String,
    pub class: String,
    pub sub_class: String,
    pub hemilineage: String,
    pub side: String,
    pub nerve: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellStats {
    pub root_id: u64,
    pub length_nm: u64,
    pub area_nm: u64,
    pub size_nm: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub root_id: u64,
    pub label: String,
    pub user_id: String,
    pub position: String,
    pub supervoxel_id: u64,
    pub label_id: String,
    pub date_created: String,
    pub user_name: String,
    pub user_affiliation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coordinate {
    pub root_id: u64,
    pub position: String,
    pub supervoxel_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellTypeRow {
    pub root_id: u64,
    pub primary_type: String,
    #[serde(rename = "additional_type(s)")]
    pub additional_types: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectivityTagRow {
    pub root_id: u64,
    pub connectivity_tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionRow {
    pub pre_root_id: u64,
    pub post_root_id: u64,
    pub neuropil: String,
    pub syn_count: u64,
    pub nt_type: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Incoming,
    Outgoing,
    Both,
}

#[derive(Debug, Clone, Copy)]
pub enum NeuronDirection {
    Incoming,
    Outgoing,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPage {
    pub root_id: u64,
    pub direction: Direction,
    pub min_synapses: u64,
    pub matched: u64,
    pub unique_partners: u64,
    pub total_synapses: u64,
    pub returned: usize,
    pub truncated: bool,
    pub connections: Vec<ConnectionRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuronDetails {
    pub neuron: Neuron,
    pub classification: Option<Classification>,
    pub cell_types: Vec<String>,
    pub cell_stats: Option<CellStats>,
    pub connectivity_tags: Vec<String>,
    pub labels: Vec<Label>,
    pub coordinates: Vec<Coordinate>,
    pub connections: ConnectionPage,
}

fn data_path(data_dir: &Path, file_name: &str) -> PathBuf {
    data_dir.join(file_name)
}

fn open_csv(path: &Path) -> Result<csv::Reader<GzDecoder<File>>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let decoder = GzDecoder::new(file);
    Ok(csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(decoder))
}

fn column_index(headers: &StringRecord, name: &str) -> Result<usize> {
    headers
        .iter()
        .position(|header| header == name)
        .with_context(|| format!("missing required CSV column: {name}"))
}

fn root_id_from_record(record: &StringRecord, root_index: usize) -> Result<u64> {
    record
        .get(root_index)
        .context("record has no root_id column")?
        .parse::<u64>()
        .context("parsing root_id")
}

fn find_one<T>(data_dir: &Path, file_name: &str, root_id: u64) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    let path = data_path(data_dir, file_name);
    let mut reader = open_csv(&path)?;
    let headers = reader
        .headers()
        .with_context(|| format!("reading headers from {}", path.display()))?
        .clone();
    let root_index = column_index(&headers, "root_id")?;
    for record in reader.records() {
        let record = record.with_context(|| format!("reading {}", path.display()))?;
        if root_id_from_record(&record, root_index)? == root_id {
            return record
                .deserialize(Some(&headers))
                .with_context(|| format!("decoding {} in {}", file_name, path.display()))
                .map(Some);
        }
    }
    Ok(None)
}

fn find_all<T>(data_dir: &Path, file_name: &str, root_id: u64) -> Result<Vec<T>>
where
    T: DeserializeOwned,
{
    let path = data_path(data_dir, file_name);
    let mut reader = open_csv(&path)?;
    let headers = reader
        .headers()
        .with_context(|| format!("reading headers from {}", path.display()))?
        .clone();
    let root_index = column_index(&headers, "root_id")?;
    let mut result = Vec::new();
    for record in reader.records() {
        let record = record.with_context(|| format!("reading {}", path.display()))?;
        if root_id_from_record(&record, root_index)? == root_id {
            result.push(
                record
                    .deserialize(Some(&headers))
                    .with_context(|| format!("decoding {} in {}", file_name, path.display()))?,
            );
        }
    }
    Ok(result)
}

fn count_rows(path: &Path) -> Result<u64> {
    let mut reader = open_csv(path)?;
    let mut count = 0_u64;
    for record in reader.records() {
        record.with_context(|| format!("reading {}", path.display()))?;
        count += 1;
    }
    Ok(count)
}

fn deserialize_f64_default_zero<'de, D>(deserializer: D) -> std::result::Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    let Some(raw) = raw else {
        return Ok(0.0);
    };
    if raw.trim().is_empty() {
        return Ok(0.0);
    }
    let value = raw
        .trim()
        .parse::<f64>()
        .map_err(serde::de::Error::custom)?;
    if !value.is_finite() {
        return Err(serde::de::Error::custom("non-finite floating point value"));
    }
    Ok(value)
}

fn required_columns(file_name: &str) -> &'static [&'static str] {
    match file_name {
        "neurons.csv.gz" => &[
            "root_id",
            "group",
            "nt_type",
            "nt_type_score",
            "da_avg",
            "ser_avg",
            "gaba_avg",
            "glut_avg",
            "ach_avg",
            "oct_avg",
        ],
        "classification.csv.gz" => &[
            "root_id",
            "flow",
            "super_class",
            "class",
            "sub_class",
            "hemilineage",
            "side",
            "nerve",
        ],
        "consolidated_cell_types.csv.gz" => &["root_id", "primary_type", "additional_type(s)"],
        "cell_stats.csv.gz" => &["root_id", "length_nm", "area_nm", "size_nm"],
        "connections.csv.gz" => &[
            "pre_root_id",
            "post_root_id",
            "neuropil",
            "syn_count",
            "nt_type",
        ],
        "labels.csv.gz" => &[
            "root_id",
            "label",
            "user_id",
            "position",
            "supervoxel_id",
            "label_id",
            "date_created",
            "user_name",
            "user_affiliation",
        ],
        "coordinates.csv.gz" => &["root_id", "position", "supervoxel_id"],
        "nblast.csv.gz" => &["root_id", "scores"],
        "connectivity_tags.csv.gz" => &["root_id", "connectivity_tag"],
        _ => &[],
    }
}

pub fn validate_data(data_dir: &Path) -> Result<ValidationReport> {
    let manifest = read_manifest(data_dir)?;
    verify_manifest(data_dir)?;
    let mut files = Vec::with_capacity(manifest.files.len());

    for file in &manifest.files {
        let path = data_path(data_dir, &file.name);
        let bytes = std::fs::metadata(&path)
            .with_context(|| format!("reading metadata for {}", path.display()))?
            .len();
        let mut reader = open_csv(&path)?;
        let headers = reader
            .headers()
            .with_context(|| format!("reading headers from {}", path.display()))?
            .clone();
        for required in required_columns(&file.name) {
            ensure!(
                column_index(&headers, required).is_ok(),
                "{} is missing required column {}",
                file.name,
                required
            );
        }
        let root_index = column_index(&headers, "root_id").ok();
        let pre_index = column_index(&headers, "pre_root_id").ok();
        let post_index = column_index(&headers, "post_root_id").ok();
        let syn_index = column_index(&headers, "syn_count").ok();
        let mut rows = 0_u64;
        for record in reader.records() {
            let record = record.with_context(|| format!("reading {}", path.display()))?;
            ensure!(
                record.len() == headers.len(),
                "{} has {} fields, expected {}",
                file.name,
                record.len(),
                headers.len()
            );
            if let Some(index) = root_index {
                record
                    .get(index)
                    .with_context(|| format!("missing root_id in {}", file.name))?
                    .parse::<u64>()
                    .with_context(|| format!("invalid root_id in {}", file.name))?;
            }
            if let (Some(pre), Some(post), Some(syn)) = (pre_index, post_index, syn_index) {
                record
                    .get(pre)
                    .with_context(|| format!("missing pre_root_id in {}", file.name))?
                    .parse::<u64>()
                    .with_context(|| format!("invalid pre_root_id in {}", file.name))?;
                record
                    .get(post)
                    .with_context(|| format!("missing post_root_id in {}", file.name))?
                    .parse::<u64>()
                    .with_context(|| format!("invalid post_root_id in {}", file.name))?;
                record
                    .get(syn)
                    .with_context(|| format!("missing syn_count in {}", file.name))?
                    .parse::<u64>()
                    .with_context(|| format!("invalid syn_count in {}", file.name))?;
            }
            rows += 1;
        }
        files.push(ValidationFile {
            name: file.name.clone(),
            bytes,
            rows,
        });
    }

    Ok(ValidationReport {
        dataset: manifest.dataset,
        snapshot: manifest.snapshot,
        files,
        valid: true,
    })
}

fn split_csv_values(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn summarize(data_dir: &Path) -> Result<DatasetSummary> {
    ensure!(
        data_dir.is_dir(),
        "data directory does not exist: {}",
        data_dir.display()
    );
    let manifest = read_manifest(data_dir)?;
    let mut files = Vec::with_capacity(manifest.files.len());
    let mut connection_rows = 0_u64;
    let mut synapse_count = 0_u64;
    let mut neuron_rows = 0_u64;

    for file in &manifest.files {
        let file_name = &file.name;
        let path = data_path(data_dir, file_name);
        let bytes = std::fs::metadata(&path)
            .with_context(|| format!("reading metadata for {}", path.display()))?
            .len();
        let rows = if file_name == "connections.csv.gz" {
            let mut reader = open_csv(&path)?;
            let headers = reader
                .headers()
                .with_context(|| format!("reading headers from {}", path.display()))?
                .clone();
            let syn_index = column_index(&headers, "syn_count")?;
            for record in reader.records() {
                let record = record.with_context(|| format!("reading {}", path.display()))?;
                let syn_count = record
                    .get(syn_index)
                    .context("connection row has no syn_count column")?
                    .parse::<u64>()
                    .context("parsing syn_count")?;
                connection_rows += 1;
                synapse_count += syn_count;
            }
            connection_rows
        } else {
            let rows = count_rows(&path)?;
            if file_name == "neurons.csv.gz" {
                neuron_rows = rows;
            }
            rows
        };
        files.push(FileStats {
            name: file_name.clone(),
            bytes,
            rows,
        });
    }

    Ok(DatasetSummary {
        dataset: manifest.dataset,
        snapshot: manifest.snapshot,
        source: manifest.source,
        citation: manifest.citation,
        data_dir: data_dir.display().to_string(),
        files,
        neuron_rows,
        connection_rows,
        synapse_count,
    })
}

pub fn read_manifest(data_dir: &Path) -> Result<Manifest> {
    let manifest_path = data_path(data_dir, "manifest.json");
    let manifest_file = File::open(&manifest_path)
        .with_context(|| format!("opening {}", manifest_path.display()))?;
    let manifest: Manifest = serde_json::from_reader(manifest_file)
        .with_context(|| format!("decoding {}", manifest_path.display()))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &Manifest) -> Result<()> {
    ensure!(
        !manifest.dataset.trim().is_empty(),
        "manifest dataset is empty"
    );
    ensure!(
        !manifest.snapshot.trim().is_empty(),
        "manifest snapshot is empty"
    );
    let mut names = HashSet::new();
    for file in &manifest.files {
        let name_path = Path::new(&file.name);
        ensure!(
            name_path.components().count() == 1
                && name_path.file_name().and_then(|name| name.to_str()) == Some(file.name.as_str()),
            "manifest contains a non-local file path: {}",
            file.name
        );
        ensure!(
            names.insert(file.name.clone()),
            "duplicate manifest file: {}",
            file.name
        );
        ensure!(
            file.bytes > 0,
            "manifest file has zero bytes: {}",
            file.name
        );
        ensure!(
            file.url.starts_with(&format!("{DATASET_BASE_URL}/")),
            "manifest URL is outside the expected FlyWire dataset: {}",
            file.url
        );
        ensure!(
            file.sha256.len() == 64 && file.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "invalid SHA-256 in manifest for {}",
            file.name
        );
    }
    for expected in DATA_FILES {
        ensure!(
            names.contains(expected),
            "manifest is missing required file: {expected}"
        );
    }
    ensure!(
        manifest.files.len() == DATA_FILES.len(),
        "manifest contains {} files, expected {}",
        manifest.files.len(),
        DATA_FILES.len()
    );
    Ok(())
}

pub fn manifest_sha256(data_dir: &Path) -> Result<String> {
    let path = data_path(data_dir, "manifest.json");
    let mut file = File::open(&path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher).with_context(|| format!("hashing {}", path.display()))?;
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn verify_manifest(data_dir: &Path) -> Result<VerificationReport> {
    let manifest = read_manifest(data_dir)?;

    let mut total_bytes = 0_u64;
    for file in &manifest.files {
        let path = data_path(data_dir, &file.name);
        let metadata = std::fs::metadata(&path)
            .with_context(|| format!("missing data file: {}", path.display()))?;
        ensure!(
            metadata.len() == file.bytes,
            "size mismatch for {}: expected {}, got {}",
            file.name,
            file.bytes,
            metadata.len()
        );

        let mut input = File::open(&path).with_context(|| format!("opening {}", path.display()))?;
        let mut hasher = Sha256::new();
        io::copy(&mut input, &mut hasher).with_context(|| format!("hashing {}", path.display()))?;
        let digest = format!("{:x}", hasher.finalize());
        ensure!(
            digest == file.sha256,
            "SHA-256 mismatch for {}: expected {}, got {}",
            file.name,
            file.sha256,
            digest
        );
        total_bytes += metadata.len();
    }

    Ok(VerificationReport {
        data_dir: data_dir.display().to_string(),
        verified_files: manifest.files.len(),
        total_bytes,
    })
}

pub fn load_neuron(
    data_dir: &Path,
    root_id: u64,
    direction: NeuronDirection,
    limit: usize,
    min_synapses: u64,
) -> Result<NeuronDetails> {
    let neuron: Neuron = find_one(data_dir, "neurons.csv.gz", root_id)?
        .with_context(|| format!("neuron {root_id} is not present in the dataset"))?;
    let classification = find_one(data_dir, "classification.csv.gz", root_id)?;
    let cell_stats = find_one(data_dir, "cell_stats.csv.gz", root_id)?;
    let cell_type_rows =
        find_all::<CellTypeRow>(data_dir, "consolidated_cell_types.csv.gz", root_id)?;
    let connectivity_tag_rows =
        find_all::<ConnectivityTagRow>(data_dir, "connectivity_tags.csv.gz", root_id)?;
    let labels = find_all(data_dir, "labels.csv.gz", root_id)?;
    let coordinates = find_all(data_dir, "coordinates.csv.gz", root_id)?;
    let connections = query_connections(data_dir, root_id, direction, limit, min_synapses)?;

    let mut cell_types = Vec::new();
    for row in cell_type_rows {
        cell_types.extend(
            std::iter::once(row.primary_type).chain(split_csv_values(&row.additional_types)),
        );
    }
    cell_types.retain(|value| !value.is_empty());
    cell_types.sort();
    cell_types.dedup();

    let connectivity_tags = connectivity_tag_rows
        .into_iter()
        .flat_map(|row| split_csv_values(&row.connectivity_tag))
        .collect();

    Ok(NeuronDetails {
        neuron,
        classification,
        cell_types,
        cell_stats,
        connectivity_tags,
        labels,
        coordinates,
        connections,
    })
}

pub fn query_connections(
    data_dir: &Path,
    root_id: u64,
    direction: NeuronDirection,
    limit: usize,
    min_synapses: u64,
) -> Result<ConnectionPage> {
    let path = data_path(data_dir, "connections.csv.gz");
    let mut reader = open_csv(&path)?;
    let headers = reader
        .headers()
        .with_context(|| format!("reading headers from {}", path.display()))?
        .clone();
    let pre_index = column_index(&headers, "pre_root_id")?;
    let post_index = column_index(&headers, "post_root_id")?;
    let syn_index = column_index(&headers, "syn_count")?;
    let mut connections = Vec::with_capacity(limit.min(10_000));
    let mut partners = HashSet::new();
    let mut matched = 0_u64;
    let mut total_synapses = 0_u64;

    for record in reader.records() {
        let record = record.with_context(|| format!("reading {}", path.display()))?;
        let pre_root_id = record
            .get(pre_index)
            .context("connection row has no pre_root_id column")?
            .parse::<u64>()
            .context("parsing pre_root_id")?;
        let post_root_id = record
            .get(post_index)
            .context("connection row has no post_root_id column")?
            .parse::<u64>()
            .context("parsing post_root_id")?;
        let syn_count = record
            .get(syn_index)
            .context("connection row has no syn_count column")?
            .parse::<u64>()
            .context("parsing syn_count")?;
        let is_match = match direction {
            NeuronDirection::Incoming => post_root_id == root_id,
            NeuronDirection::Outgoing => pre_root_id == root_id,
            NeuronDirection::Both => pre_root_id == root_id || post_root_id == root_id,
        };
        if !is_match || syn_count < min_synapses {
            continue;
        }
        let partner = if pre_root_id == root_id {
            post_root_id
        } else {
            pre_root_id
        };
        partners.insert(partner);
        matched += 1;
        total_synapses += syn_count;
        if connections.len() < limit {
            connections.push(
                record
                    .deserialize(Some(&headers))
                    .with_context(|| format!("decoding connection row in {}", path.display()))?,
            );
        }
    }

    let direction = match direction {
        NeuronDirection::Incoming => Direction::Incoming,
        NeuronDirection::Outgoing => Direction::Outgoing,
        NeuronDirection::Both => Direction::Both,
    };
    Ok(ConnectionPage {
        root_id,
        direction,
        min_synapses,
        matched,
        unique_partners: partners.len() as u64,
        total_synapses,
        returned: connections.len(),
        truncated: matched > connections.len() as u64,
        connections,
    })
}
