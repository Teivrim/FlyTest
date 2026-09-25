use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::index::open_db;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    Input,
    Output,
    Internal,
    Endocrine,
    Unknown,
}

impl ChannelKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
            Self::Internal => "internal",
            Self::Endocrine => "endocrine",
            Self::Unknown => "unknown",
        }
    }
}

pub fn classify_channel(
    flow: &str,
    super_class: &str,
    class: &str,
    sub_class: &str,
    _cell_types: &[String],
    _nt_type: &str,
) -> ChannelKind {
    let flow = flow.trim().to_ascii_lowercase();
    let super_class = super_class.trim().to_ascii_lowercase();
    let class = class.trim().to_ascii_lowercase();
    let sub_class = sub_class.trim().to_ascii_lowercase();

    if super_class == "endocrine"
        || class.contains("neurosecretory")
        || sub_class.contains("neurosecretory")
    {
        return ChannelKind::Endocrine;
    }
    if flow == "efferent"
        || super_class == "motor"
        || super_class == "descending"
        || class.contains("motor")
    {
        return ChannelKind::Output;
    }
    if flow == "afferent"
        || super_class == "sensory"
        || super_class == "sensory_ascending"
        || super_class == "ascending"
        || super_class == "visual_projection"
    {
        return ChannelKind::Input;
    }
    if flow == "intrinsic" {
        return ChannelKind::Internal;
    }
    ChannelKind::Unknown
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LigandSpec {
    pub id: String,
    pub name: String,
    pub category: String,
    pub aliases: Vec<String>,
    pub evidence: String,
}

fn ligand(id: &str, name: &str, category: &str, aliases: &[&str]) -> LigandSpec {
    LigandSpec {
        id: id.to_owned(),
        name: name.to_owned(),
        category: category.to_owned(),
        aliases: aliases.iter().map(|value| (*value).to_owned()).collect(),
        evidence: "FlyWire labels/cell types; normalized catalog".to_owned(),
    }
}

pub fn ligand_catalog() -> Vec<LigandSpec> {
    vec![
        ligand(
            "ACH",
            "acetylcholine",
            "neurotransmitter",
            &["ACETYLCHOLINE", "ACH"],
        ),
        ligand(
            "GABA",
            "gamma-aminobutyric acid",
            "neurotransmitter",
            &["GABA"],
        ),
        ligand(
            "GLUT",
            "glutamate",
            "neurotransmitter",
            &["GLUT", "GLUTAMATE"],
        ),
        ligand("DA", "dopamine", "neurotransmitter", &["DA", "DOPAMINE"]),
        ligand(
            "SER",
            "serotonin",
            "neurotransmitter",
            &["SER", "5HT", "SEROTONIN"],
        ),
        ligand(
            "OCT",
            "octopamine",
            "neurotransmitter",
            &["OCT", "OCTOPAMINE"],
        ),
        ligand("TYR", "tyramine", "neurotransmitter", &["TYR", "TYRAMINE"]),
        ligand(
            "DILP",
            "Drosophila insulin-like peptide",
            "neuropeptide",
            &["DILP", "DILPS"],
        ),
        ligand("DH31", "dipeptide DH31", "neuropeptide", &["DH31"]),
        ligand("DH44", "dipeptide DH44", "neuropeptide", &["DH44"]),
        ligand("ITP", "insulin-like peptide", "neuropeptide", &["ITP"]),
        ligand(
            "CAPA",
            "cardiac accelerator peptide",
            "neuropeptide",
            &["CAPA"],
        ),
        ligand("CRZ", "corazonin", "neuropeptide", &["CRZ", "CORAZONIN"]),
        ligand(
            "DNES1",
            "drosophila neuropeptide-like ETS-1",
            "neuropeptide",
            &["DNES1"],
        ),
        ligand(
            "DNES2",
            "drosophila neuropeptide-like ETS-2",
            "neuropeptide",
            &["DNES2"],
        ),
        ligand(
            "DNES3",
            "drosophila neuropeptide-like ETS-3",
            "neuropeptide",
            &["DNES3"],
        ),
        ligand(
            "DMS",
            "dimmed-like neurosecretory peptide",
            "neuropeptide",
            &["DMS"],
        ),
        ligand("HUGIN", "hugin", "neuropeptide", &["HUGIN", "HUGIN-RG"]),
        ligand(
            "PI1",
            "pars intercerebralis peptide 1",
            "neuropeptide",
            &["PI1"],
        ),
        ligand(
            "PI2",
            "pars intercerebralis peptide 2",
            "neuropeptide",
            &["PI2"],
        ),
        ligand(
            "PI3",
            "pars intercerebralis peptide 3",
            "neuropeptide",
            &["PI3"],
        ),
        ligand("NPF", "neuropeptide F", "neuropeptide", &["NPF", "NMF"]),
        ligand("sNPF", "short neuropeptide F", "neuropeptide", &["SNPF"]),
        ligand("PDF", "pigment dispersing factor", "neuropeptide", &["PDF"]),
        ligand(
            "FMRF",
            "FMRFamide-related peptide",
            "neuropeptide",
            &["FMRF", "FMRFAMIDE"],
        ),
        ligand("ECD", "ecdysone", "hormone", &["ECD", "ECDYSONE", "20E"]),
        ligand(
            "JH",
            "juvenile hormone",
            "hormone",
            &["JH", "JUVENILE HORMONE"],
        ),
    ]
}

pub fn canonicalize_ligand(raw: &str) -> Option<LigandSpec> {
    let upper = raw.to_ascii_uppercase();
    ligand_catalog()
        .into_iter()
        .find(|spec| spec.aliases.iter().any(|alias| upper.contains(alias)))
}

pub fn ligand_effect(raw: &str) -> &'static str {
    match canonicalize_ligand(raw).map(|item| item.id).as_deref() {
        Some("GABA") => "inhibitory",
        Some("ACH" | "GLUT") => "excitatory",
        Some("DA" | "SER" | "OCT" | "TYR") => "modulatory",
        Some(_) => "modulatory",
        None => "unknown",
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelRecord {
    pub root_id: u64,
    pub kind: ChannelKind,
    pub group: String,
    pub nt_type: String,
    pub flow: String,
    pub super_class: String,
    pub class: String,
    pub sub_class: String,
    pub cell_types: Vec<String>,
    pub labels: Vec<String>,
    pub in_degree: u64,
    pub out_degree: u64,
    pub in_synapses: u64,
    pub out_synapses: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelReport {
    pub kind: String,
    pub database: String,
    pub count: usize,
    pub total_channels: u64,
    pub channels: Vec<ChannelRecord>,
}

fn sql_id(value: u64) -> Result<i64> {
    i64::try_from(value).context("FlyWire ID does not fit in SQLite INTEGER")
}

fn from_sql_id(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn split_pipe(value: &str) -> Vec<String> {
    value
        .split('|')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn channel_report(database: &Path, kind: &str, limit: usize) -> Result<ChannelReport> {
    let normalized_kind = "CASE
        WHEN c.super_class = 'endocrine'
          OR LOWER(COALESCE(c.class, '')) LIKE '%neurosecretory%'
          OR LOWER(COALESCE(c.sub_class, '')) LIKE '%neurosecretory%' THEN 'endocrine'
        WHEN c.flow = 'efferent'
          OR c.super_class IN ('motor', 'descending')
          OR LOWER(COALESCE(c.class, '')) LIKE '%motor%' THEN 'output'
        WHEN c.flow = 'afferent'
          OR c.super_class IN ('sensory', 'sensory_ascending', 'ascending', 'visual_projection') THEN 'input'
        WHEN c.flow = 'intrinsic' THEN 'internal'
        ELSE 'unknown' END";
    let where_clause = if kind == "all" {
        "1 = 1".to_owned()
    } else if ["input", "output", "internal", "endocrine", "unknown"].contains(&kind) {
        format!("({normalized_kind}) = '{kind}'")
    } else {
        anyhow::bail!("unknown channel kind: {kind}")
    };
    let limit = i64::try_from(limit.clamp(1, 10_000)).unwrap_or(10_000);
    let sql = format!(
        "SELECT n.root_id, n.group_name, n.nt_type,
                COALESCE(c.flow, ''), COALESCE(c.super_class, ''),
                COALESCE(c.class, ''), COALESCE(c.sub_class, ''),
                COALESCE((SELECT group_concat(cell_type) FROM cell_types WHERE root_id = n.root_id), ''),
                COALESCE((SELECT group_concat(label) FROM labels WHERE root_id = n.root_id), ''),
                COALESCE((SELECT COUNT(*) FROM connections WHERE post_root_id = n.root_id), 0),
                COALESCE((SELECT COUNT(*) FROM connections WHERE pre_root_id = n.root_id), 0),
                COALESCE((SELECT SUM(syn_count) FROM connections WHERE post_root_id = n.root_id), 0),
                COALESCE((SELECT SUM(syn_count) FROM connections WHERE pre_root_id = n.root_id), 0)
         FROM neurons n LEFT JOIN classification c ON c.root_id = n.root_id
         WHERE {where_clause}
         ORDER BY n.root_id LIMIT ?1"
    );
    let conn = open_db(database)?;
    let total_channels: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM neurons n LEFT JOIN classification c ON c.root_id = n.root_id WHERE {where_clause}"
        ),
        [],
        |row| row.get(0),
    )?;
    let total_channels = from_sql_id(total_channels)?;
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map([limit], |row| {
        let root_id: i64 = row.get(0)?;
        let group: String = row.get(1)?;
        let nt_type: String = row.get(2)?;
        let flow: String = row.get(3)?;
        let super_class: String = row.get(4)?;
        let class: String = row.get(5)?;
        let sub_class: String = row.get(6)?;
        let cell_types = split_pipe(&row.get::<_, String>(7)?);
        let labels = split_pipe(&row.get::<_, String>(8)?);
        let kind = classify_channel(
            &flow,
            &super_class,
            &class,
            &sub_class,
            &cell_types,
            &nt_type,
        );
        Ok(ChannelRecord {
            root_id: from_sql_id(root_id)?,
            kind,
            group,
            nt_type,
            flow,
            super_class,
            class,
            sub_class,
            cell_types,
            labels,
            in_degree: from_sql_id(row.get(9)?)?,
            out_degree: from_sql_id(row.get(10)?)?,
            in_synapses: from_sql_id(row.get(11)?)?,
            out_synapses: from_sql_id(row.get(12)?)?,
        })
    })?;
    let channels = rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(anyhow::Error::from)?;
    Ok(ChannelReport {
        kind: kind.to_owned(),
        database: database.display().to_string(),
        count: channels.len(),
        total_channels,
        channels,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct LigandRecord {
    pub id: String,
    pub name: String,
    pub category: String,
    pub neuron_count: u64,
    pub example_root_ids: Vec<u64>,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LigandReport {
    pub database: String,
    pub observed: Vec<LigandRecord>,
    pub catalog: Vec<LigandSpec>,
    pub catalog_size: usize,
    pub note: String,
}

#[derive(Default)]
struct LigandAggregate {
    name: String,
    category: String,
    roots: HashSet<u64>,
    evidence: String,
}

pub fn ligand_report(database: &Path) -> Result<LigandReport> {
    let conn = open_db(database)?;
    let mut aggregates: BTreeMap<String, LigandAggregate> = BTreeMap::new();

    let mut statement = conn.prepare(
        "SELECT n.root_id, n.nt_type
         FROM neurons n WHERE n.nt_type IS NOT NULL AND n.nt_type <> ''",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((from_sql_id(row.get(0)?)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (root_id, raw) = row?;
        if let Some(spec) = canonicalize_ligand(&raw) {
            let aggregate = aggregates
                .entry(spec.id.clone())
                .or_insert_with(|| LigandAggregate {
                    name: spec.name,
                    category: spec.category,
                    roots: HashSet::new(),
                    evidence: format!("FlyWire nt_type={raw}"),
                });
            aggregate.roots.insert(root_id);
        }
    }

    let mut statement = conn.prepare(
        "SELECT ct.root_id, ct.cell_type
         FROM cell_types ct JOIN classification c ON c.root_id = ct.root_id
         WHERE c.super_class = 'endocrine'",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((from_sql_id(row.get(0)?)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (root_id, raw) = row?;
        if let Some(spec) = canonicalize_ligand(&raw) {
            let aggregate = aggregates
                .entry(spec.id.clone())
                .or_insert_with(|| LigandAggregate {
                    name: spec.name,
                    category: spec.category,
                    roots: HashSet::new(),
                    evidence: format!("FlyWire endocrine cell type={raw}"),
                });
            aggregate.roots.insert(root_id);
        }
    }

    let mut observed = aggregates
        .into_iter()
        .map(|(id, aggregate)| {
            let mut example_root_ids: Vec<u64> = aggregate.roots.into_iter().collect();
            example_root_ids.sort_unstable();
            example_root_ids.truncate(8);
            LigandRecord {
                id,
                name: aggregate.name,
                category: aggregate.category,
                neuron_count: example_root_ids.len() as u64,
                example_root_ids,
                evidence: aggregate.evidence,
            }
        })
        .collect::<Vec<_>>();
    for record in &mut observed {
        let count_sql = match record.category.as_str() {
            "neurotransmitter" => "SELECT COUNT(*) FROM neurons WHERE nt_type = ?1",
            _ => {
                "SELECT COUNT(DISTINCT ct.root_id) FROM cell_types ct
                 JOIN classification c ON c.root_id = ct.root_id
                 WHERE c.super_class = 'endocrine' AND UPPER(ct.cell_type) LIKE '%' || ?1 || '%'"
            }
        };
        let count: i64 = conn.query_row(count_sql, [&record.id], |row| row.get(0))?;
        record.neuron_count = from_sql_id(count)?;
    }
    observed.sort_by(|left, right| {
        right
            .neuron_count
            .cmp(&left.neuron_count)
            .then(left.id.cmp(&right.id))
    });
    let catalog = ligand_catalog();
    Ok(LigandReport {
        database: database.display().to_string(),
        catalog_size: catalog.len(),
        catalog,
        observed,
        note: "Counts are label/cell-type associations, not measured secretion or hormone production rates.".to_owned(),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub root_id: u64,
    pub depth: usize,
    pub kind: ChannelKind,
    pub group: String,
    pub nt_type: String,
    pub labels: Vec<String>,
    pub in_synapses: u64,
    pub out_synapses: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    pub pre_root_id: u64,
    pub post_root_id: u64,
    pub syn_count: u64,
    pub neuropils: Vec<String>,
    pub transmitters: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphReport {
    pub root_id: u64,
    pub direction: String,
    pub depth: usize,
    pub min_synapses: u64,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub truncated: bool,
    pub model_note: String,
}

#[derive(Debug, Clone)]
struct AdjacentEdge {
    pre_root_id: u64,
    post_root_id: u64,
    syn_count: u64,
    neuropils: Vec<String>,
    transmitters: Vec<String>,
}

fn query_node(
    conn: &Connection,
    root_id: u64,
) -> Result<(ChannelKind, String, String, Vec<String>, u64, u64)> {
    let mut statement = conn.prepare(
        "SELECT COALESCE(c.flow, ''), COALESCE(c.super_class, ''), COALESCE(c.class, ''),
                COALESCE(c.sub_class, ''), n.group_name, n.nt_type,
                COALESCE((SELECT group_concat(label) FROM labels WHERE root_id = n.root_id), ''),
                COALESCE((SELECT SUM(syn_count) FROM connections WHERE post_root_id = n.root_id), 0),
                COALESCE((SELECT SUM(syn_count) FROM connections WHERE pre_root_id = n.root_id), 0)
         FROM neurons n LEFT JOIN classification c ON c.root_id = n.root_id
         WHERE n.root_id = ?1",
    )?;
    let id = sql_id(root_id)?;
    let mut rows = statement.query([id])?;
    let row = rows
        .next()
        .context("reading graph node")?
        .context("node disappeared while reading graph")?;
    let flow: String = row.get(0)?;
    let super_class: String = row.get(1)?;
    let class: String = row.get(2)?;
    let sub_class: String = row.get(3)?;
    let group: String = row.get(4)?;
    let nt_type: String = row.get(5)?;
    let labels = split_pipe(&row.get::<_, String>(6)?);
    let kind = classify_channel(&flow, &super_class, &class, &sub_class, &[], &nt_type);
    let in_synapses = from_sql_id(row.get(7)?)?;
    let out_synapses = from_sql_id(row.get(8)?)?;
    Ok((kind, group, nt_type, labels, in_synapses, out_synapses))
}

fn query_adjacent_edges(
    conn: &Connection,
    root_id: u64,
    direction: &str,
    min_synapses: u64,
    limit: usize,
) -> Result<Vec<AdjacentEdge>> {
    let id = sql_id(root_id)?;
    let min = i64::try_from(min_synapses).unwrap_or(i64::MAX);
    let limit = i64::try_from(limit.clamp(1, 10_000)).unwrap_or(10_000);
    let (predicate, order) = match direction {
        "outgoing" => ("pre_root_id = ?1", "pre_root_id, post_root_id"),
        "incoming" => ("post_root_id = ?1", "post_root_id, pre_root_id"),
        "both" => (
            "(pre_root_id = ?1 OR post_root_id = ?1)",
            "pre_root_id, post_root_id",
        ),
        other => anyhow::bail!("unknown graph direction: {other}"),
    };
    let sql = format!(
        "SELECT pre_root_id, post_root_id, SUM(syn_count),
                GROUP_CONCAT(DISTINCT neuropil), GROUP_CONCAT(DISTINCT nt_type)
         FROM connections WHERE {predicate} AND syn_count >= ?2
         GROUP BY pre_root_id, post_root_id ORDER BY {order}, SUM(syn_count) DESC LIMIT ?3"
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map(params![id, min, limit], |row| {
        Ok(AdjacentEdge {
            pre_root_id: from_sql_id(row.get(0)?)?,
            post_root_id: from_sql_id(row.get(1)?)?,
            syn_count: from_sql_id(row.get(2)?)?,
            neuropils: row
                .get::<_, String>(3)?
                .split(',')
                .map(ToOwned::to_owned)
                .collect(),
            transmitters: row
                .get::<_, String>(4)?
                .split(',')
                .map(ToOwned::to_owned)
                .collect(),
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(anyhow::Error::from)
}

pub fn collect_graph(
    database: &Path,
    root_id: u64,
    direction: &str,
    depth: usize,
    min_synapses: u64,
    max_nodes: usize,
) -> Result<GraphReport> {
    ensure!(depth <= 12, "graph depth must be <= 12");
    ensure!(max_nodes >= 1, "max_nodes must be at least 1");
    let conn = open_db(database)?;
    let mut queue = VecDeque::from([(root_id, 0_usize)]);
    let mut depths = HashMap::from([(root_id, 0_usize)]);
    let mut raw_edges = HashMap::<(u64, u64), AdjacentEdge>::new();
    let mut truncated = false;
    let edge_budget = max_nodes.saturating_mul(10).max(100);
    let per_node_limit = max_nodes.max(32);

    while let Some((node, node_depth)) = queue.pop_front() {
        if node_depth >= depth {
            continue;
        }
        let adjacent = query_adjacent_edges(&conn, node, direction, min_synapses, per_node_limit)?;
        if adjacent.len() >= per_node_limit {
            truncated = true;
        }
        for edge in adjacent {
            if raw_edges.len() >= edge_budget {
                truncated = true;
                break;
            }
            let (pre_root_id, post_root_id) = (edge.pre_root_id, edge.post_root_id);
            raw_edges.insert((pre_root_id, post_root_id), edge);
            for next in [pre_root_id, post_root_id] {
                if depths.len() >= max_nodes && !depths.contains_key(&next) {
                    truncated = true;
                    continue;
                }
                if let std::collections::hash_map::Entry::Vacant(entry) = depths.entry(next) {
                    entry.insert(node_depth + 1);
                    queue.push_back((next, node_depth + 1));
                }
            }
        }
        if raw_edges.len() >= edge_budget || (depths.len() >= max_nodes && !queue.is_empty()) {
            truncated = true;
            break;
        }
    }

    let mut nodes = Vec::with_capacity(depths.len());
    for (root, node_depth) in depths {
        let (kind, group, nt_type, labels, in_synapses, out_synapses) = query_node(&conn, root)?;
        nodes.push(GraphNode {
            root_id: root,
            depth: node_depth,
            kind,
            group,
            nt_type,
            labels,
            in_synapses,
            out_synapses,
        });
    }
    nodes.sort_by_key(|node| (node.depth, node.root_id));
    let edges = raw_edges
        .into_values()
        .map(|edge| GraphEdge {
            pre_root_id: edge.pre_root_id,
            post_root_id: edge.post_root_id,
            syn_count: edge.syn_count,
            neuropils: edge.neuropils,
            transmitters: edge.transmitters,
        })
        .collect();
    Ok(GraphReport {
        root_id,
        direction: direction.to_owned(),
        depth,
        min_synapses,
        nodes,
        edges,
        truncated,
        model_note: "Graph is a structural projection from the FlyWire connectome; edge width represents summed syn_count.".to_owned(),
    })
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn channel_color(kind: ChannelKind) -> &'static str {
    match kind {
        ChannelKind::Input => "#2e7d32",
        ChannelKind::Output => "#c62828",
        ChannelKind::Endocrine => "#6a1b9a",
        ChannelKind::Internal => "#455a64",
        ChannelKind::Unknown => "#8d6e63",
    }
}

pub fn render_svg(report: &GraphReport) -> String {
    let mut layers: BTreeMap<usize, Vec<u64>> = BTreeMap::new();
    for node in &report.nodes {
        layers.entry(node.depth).or_default().push(node.root_id);
    }
    let mut positions = HashMap::new();
    let mut width = 640_usize;
    for (layer, ids) in &layers {
        width = width.max(120 + layer * 240);
        for (index, root_id) in ids.iter().enumerate() {
            positions.insert(
                *root_id,
                (80.0 + *layer as f64 * 240.0, 70.0 + index as f64 * 95.0),
            );
        }
    }
    let height = layers
        .values()
        .map(Vec::len)
        .max()
        .unwrap_or(1)
        .saturating_mul(95)
        .saturating_add(120);
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><defs><marker id=\"arrow\" markerWidth=\"8\" markerHeight=\"8\" refX=\"7\" refY=\"3\" orient=\"auto\"><path d=\"M0,0 L0,6 L8,3 z\" fill=\"#37474f\"/></marker></defs><rect width=\"100%\" height=\"100%\" fill=\"#fafafa\"/><text x=\"20\" y=\"28\" font-family=\"sans-serif\" font-size=\"18\">FlyWire graph: root {}</text>",
        report.root_id
    );
    for edge in &report.edges {
        if let (Some(&(x1, y1)), Some(&(x2, y2))) = (
            positions.get(&edge.pre_root_id),
            positions.get(&edge.post_root_id),
        ) {
            let color = edge
                .neuropils
                .first()
                .map(|_| "#546e7a")
                .unwrap_or("#546e7a");
            svg.push_str(&format!(
                "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"{color}\" stroke-width=\"{:.1}\" marker-end=\"url(#arrow)\" opacity=\"0.65\"><title>{} synapses; {}</title></line>",
                (edge.syn_count as f64).sqrt().clamp(1.0, 8.0),
                edge.syn_count,
                escape_xml(&edge.transmitters.join(", ")),
            ));
        }
    }
    for node in &report.nodes {
        let (x, y) = positions[&node.root_id];
        let label = node
            .labels
            .first()
            .cloned()
            .unwrap_or_else(|| node.group.clone());
        let radius = (node.out_synapses + node.in_synapses).max(1) as f64;
        let radius = radius.sqrt().clamp(7.0, 24.0);
        let short_label = if node.root_id.to_string().len() > 4 {
            "•".to_owned()
        } else {
            node.root_id.to_string()
        };
        svg.push_str(&format!(
            "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"{radius:.1}\" fill=\"{}\" stroke=\"white\" stroke-width=\"2\"><title>{} | {}</title></circle><text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" dominant-baseline=\"middle\" font-family=\"sans-serif\" font-size=\"10\" fill=\"white\">{}</text>",
            channel_color(node.kind),
            node.root_id,
            escape_xml(&label),
            x,
            y,
            short_label
        ));
    }
    svg.push_str("</svg>");
    svg
}

pub fn render_dot(report: &GraphReport) -> String {
    let mut dot = String::from(
        "digraph flywire {\n  rankdir=LR;\n  node [shape=circle, style=filled, fontname=Arial];\n",
    );
    for node in &report.nodes {
        dot.push_str(&format!(
            "  n{} [label=\"{}\", fillcolor=\"{}\"];\n",
            node.root_id,
            node.labels
                .first()
                .map_or_else(|| node.root_id.to_string(), Clone::clone)
                .replace('"', "\\\""),
            channel_color(node.kind)
        ));
    }
    for edge in &report.edges {
        dot.push_str(&format!(
            "  n{} -> n{} [label=\"{}\", penwidth={:.1}];\n",
            edge.pre_root_id,
            edge.post_root_id,
            edge.transmitters.join(","),
            (edge.syn_count as f64).sqrt().clamp(1.0, 8.0)
        ));
    }
    dot.push_str("}\n");
    dot
}

pub fn render_mermaid(report: &GraphReport) -> String {
    let mut mermaid = String::from("graph LR\n");
    for node in &report.nodes {
        let label = node
            .labels
            .first()
            .map_or_else(|| node.root_id.to_string(), Clone::clone)
            .replace('"', "'");
        mermaid.push_str(&format!("  n{}[\"{}\"]\n", node.root_id, label));
    }
    for edge in &report.edges {
        mermaid.push_str(&format!(
            "  n{} -->|{}; {}| n{}\n",
            edge.pre_root_id,
            edge.syn_count,
            edge.transmitters.join(","),
            edge.post_root_id
        ));
    }
    mermaid
}

#[derive(Debug, Clone, Serialize)]
pub struct SignalEvent {
    pub name: String,
    pub kind: String,
    pub channel: Option<ChannelKind>,
    pub intensity: f64,
    pub effect: Option<String>,
    pub ligand: Option<String>,
    pub source_root_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReactionStep {
    pub index: usize,
    pub from_root_id: u64,
    pub to_root_id: u64,
    pub input_intensity: f64,
    pub output_intensity: f64,
    pub syn_count: u64,
    pub edge_strength: f64,
    pub transmitter: String,
    pub effect: String,
    pub target_channel: ChannelKind,
    pub neuropils: Vec<String>,
    pub path: Vec<u64>,
    pub reaction: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SimulationReport {
    pub input: SignalEvent,
    pub reactions: Vec<ReactionStep>,
    pub output_signals: Vec<SignalEvent>,
    pub visited_nodes: usize,
    pub truncated: bool,
    pub model_note: String,
}

#[derive(Debug, Clone)]
struct SignalState {
    node: u64,
    depth: usize,
    intensity: f64,
    path: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct SimulationOptions {
    pub input_root_id: u64,
    pub input_ligand: Option<String>,
    pub initial_intensity: f64,
    pub decay: f64,
    pub max_depth: usize,
    pub min_synapses: u64,
    pub max_steps: usize,
}

pub fn simulate(database: &Path, options: SimulationOptions) -> Result<SimulationReport> {
    let SimulationOptions {
        input_root_id,
        input_ligand,
        initial_intensity,
        decay,
        max_depth,
        min_synapses,
        max_steps,
    } = options;
    ensure!(
        initial_intensity.is_finite() && initial_intensity >= 0.0,
        "initial intensity must be finite and >= 0"
    );
    ensure!(
        decay.is_finite() && (0.0..=1.0).contains(&decay),
        "decay must be between 0 and 1"
    );
    ensure!(max_depth <= 20, "simulation depth must be <= 20");
    ensure!(max_steps >= 1, "max_steps must be at least 1");
    let conn = open_db(database)?;
    let source_meta = query_node(&conn, input_root_id)?;
    let ligand = input_ligand.map(|value| {
        canonicalize_ligand(&value)
            .map(|spec| spec.id)
            .unwrap_or_else(|| value.to_ascii_uppercase())
    });
    let input = SignalEvent {
        name: format!("input:{input_root_id}"),
        kind: "input".to_owned(),
        channel: Some(source_meta.0),
        intensity: initial_intensity,
        effect: None,
        ligand,
        source_root_id: Some(input_root_id),
    };
    let mut queue = VecDeque::from([SignalState {
        node: input_root_id,
        depth: 0,
        intensity: initial_intensity,
        path: vec![input_root_id],
    }]);
    let mut visited = HashSet::from([input_root_id]);
    let mut reactions = Vec::new();
    let mut output_signals = Vec::new();
    let mut truncated = false;

    while let Some(state) = queue.pop_front() {
        if state.depth >= max_depth || state.intensity <= 0.000001 {
            continue;
        }
        let edges = query_adjacent_edges(&conn, state.node, "outgoing", min_synapses, 256)?;
        for edge in edges {
            if reactions.len() >= max_steps {
                truncated = true;
                break;
            }
            let edge_strength = 1.0 - (-(edge.syn_count as f64) / 10.0).exp();
            let output_intensity = state.intensity * decay * edge_strength;
            if output_intensity <= 0.000001 {
                continue;
            }
            let target_meta = query_node(&conn, edge.post_root_id).ok();
            let target_channel = target_meta
                .as_ref()
                .map(|meta| meta.0)
                .unwrap_or(ChannelKind::Unknown);
            let mut path = state.path.clone();
            path.push(edge.post_root_id);
            let step = ReactionStep {
                index: reactions.len() + 1,
                from_root_id: state.node,
                to_root_id: edge.post_root_id,
                input_intensity: state.intensity,
                output_intensity,
                syn_count: edge.syn_count,
                edge_strength,
                transmitter: edge
                    .transmitters
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_owned()),
                effect: ligand_effect(
                    edge.transmitters
                        .first()
                        .map(String::as_str)
                        .unwrap_or("unknown"),
                )
                .to_owned(),
                target_channel,
                neuropils: edge.neuropils,
                path: path.clone(),
                reaction: "weighted_connectome_propagation".to_owned(),
            };
            reactions.push(step);
            let target_ligand = target_meta.as_ref().and_then(|meta| {
                if meta.2.is_empty() {
                    None
                } else {
                    canonicalize_ligand(&meta.2).map(|spec| spec.id)
                }
            });
            output_signals.push(SignalEvent {
                name: format!("output:{}", edge.post_root_id),
                kind: "propagated".to_owned(),
                channel: Some(target_channel),
                intensity: output_intensity,
                effect: Some(
                    edge.transmitters
                        .first()
                        .map(|value| ligand_effect(value).to_owned())
                        .unwrap_or_else(|| "unknown".to_owned()),
                ),
                ligand: target_ligand,
                source_root_id: Some(input_root_id),
            });
            visited.insert(edge.post_root_id);
            if state.depth + 1 < max_depth && !path[..path.len() - 1].contains(&edge.post_root_id) {
                queue.push_back(SignalState {
                    node: edge.post_root_id,
                    depth: state.depth + 1,
                    intensity: output_intensity,
                    path,
                });
            }
        }
        if reactions.len() >= max_steps {
            truncated = true;
            break;
        }
    }
    Ok(SimulationReport {
        input,
        reactions,
        output_signals,
        visited_nodes: visited.len(),
        truncated,
        model_note: "Heuristic signal propagation over structural synapses; not a measured electrophysiological or hormonal simulation.".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_classification_uses_structural_metadata() {
        assert_eq!(
            classify_channel("afferent", "visual", "", "", &[], "ACH"),
            ChannelKind::Input
        );
        assert_eq!(
            classify_channel("efferent", "motor", "", "", &[], "ACH"),
            ChannelKind::Output
        );
        assert_eq!(
            classify_channel("efferent", "endocrine", "", "", &[], ""),
            ChannelKind::Endocrine
        );
        assert_eq!(
            classify_channel("intrinsic", "central", "", "", &[], "GABA"),
            ChannelKind::Internal
        );
    }

    #[test]
    fn ligand_aliases_are_canonicalized() {
        assert_eq!(
            canonicalize_ligand("m_NSC_DILP").map(|item| item.id),
            Some("DILP".to_owned())
        );
        assert_eq!(
            canonicalize_ligand("5HT").map(|item| item.id),
            Some("SER".to_owned())
        );
        assert!(canonicalize_ligand("not-a-ligand").is_none());
    }
}
