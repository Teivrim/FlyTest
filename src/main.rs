use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use flytest::biology;
use flytest::editor;
use flytest::flywire::{
    self, ConnectionPage, DEFAULT_DATA_DIR, Direction, NeuronDetails, NeuronDirection,
};
use flytest::index;
use flytest::runtime;
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(
    name = "flytest",
    version,
    about = "FlyTest digital circus runtime and FlyWire tools"
)]
struct Cli {
    /// Directory containing the downloaded FAFB snapshot.
    #[arg(long, global = true, default_value = DEFAULT_DATA_DIR, value_name = "DIR")]
    data_dir: PathBuf,

    /// SQLite index path. Defaults to <data-dir>/flytest.sqlite.
    #[arg(long, global = true, value_name = "FILE")]
    db: Option<PathBuf>,

    /// Emit machine-readable JSON instead of a human-readable report.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Launch the optimized local FlyEditor web interface.
    Editor {
        #[arg(long, default_value_t = editor::DEFAULT_PORT, value_name = "PORT")]
        port: u16,
        #[arg(long, default_value_t = editor::DEFAULT_FLIES, value_name = "N")]
        flies: usize,
    },
    /// Drive the TFLY.h C core through a scripted scenario.
    Tfly {
        /// Which scenario to run.
        #[arg(long, default_value = "learn", value_name = "NAME")]
        scenario: String,
        /// Stimulus RNG seed. The same seed replays identically.
        #[arg(long, default_value_t = 7, value_name = "N")]
        seed: u32,
        /// Print the state every N seconds instead of only at the end.
        #[arg(long, default_value_t = 0.0, value_name = "SECONDS")]
        trace: f32,
    },
    /// Run the self-contained virtual fly loop and optionally write JSONL events.
    Circus {
        #[arg(long, default_value_t = 600, value_name = "N")]
        ticks: u64,
        #[arg(long, default_value_t = 0.0166667, value_name = "SECONDS")]
        dt: f64,
        #[arg(long, default_value_t = 7, value_name = "N")]
        seed: u64,
        #[arg(long, value_name = "FILE")]
        output: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = CircusModelArg::Synthetic)]
        model: CircusModelArg,
        #[arg(long, value_name = "ROOT_ID")]
        input_root: Option<u64>,
        #[arg(long, default_value_t = 1, value_name = "N")]
        min_synapses: u64,
        #[arg(long, value_name = "EXECUTABLE")]
        model_command: Option<PathBuf>,
        #[arg(long = "model-arg", value_name = "ARG")]
        model_args: Vec<String>,
    },
    /// Summarize all downloaded tables and validate their basic readability.
    Summary,
    /// Verify file sizes and SHA-256 hashes against manifest.json.
    Verify,
    /// Validate hashes, CSV headers, row widths, IDs, and connection numerics.
    Validate,
    /// Build the local SQLite index from the downloaded tables.
    Import {
        #[arg(long, help = "Replace an existing database")]
        force: bool,
    },
    /// Calculate network-level statistics from the SQLite index.
    Stats,
    /// List the most connected or most synaptically active neurons.
    Top {
        #[arg(long, default_value_t = 20, value_name = "N")]
        limit: usize,
        #[arg(long, help = "Sort by total synapse count instead of degree")]
        by_synapses: bool,
    },
    /// Show raw connection rows for one FlyWire root ID.
    Connections {
        root_id: u64,
        #[arg(long, default_value_t = 20, value_name = "N")]
        limit: usize,
        #[arg(long, default_value_t = 1, value_name = "N")]
        min_synapses: u64,
        #[arg(long, value_enum, default_value_t = DirectionArg::Outgoing)]
        direction: DirectionArg,
    },
    /// Search indexed labels, types, classes, groups, tags, and transmitters.
    Search {
        query: String,
        #[arg(long, value_enum, default_value_t = SearchField::All)]
        field: SearchField,
        #[arg(long, default_value_t = 20, value_name = "N")]
        limit: usize,
    },
    /// Show nblast shape-similarity hits for a neuron.
    Nblast {
        root_id: u64,
        #[arg(long, default_value_t = 20, value_name = "N")]
        limit: usize,
    },
    /// List normalized input/output/internal/endocrine channels.
    Channels {
        #[arg(long, value_enum, default_value_t = ChannelKindArg::All)]
        kind: ChannelKindArg,
        #[arg(long, default_value_t = 100, value_name = "N")]
        limit: usize,
    },
    /// List normalized neurotransmitter, neuropeptide, and hormone labels.
    Ligands,
    /// Render a local sub graph as SVG, DOT, or Mermaid.
    Graph {
        root_id: u64,
        #[arg(long, value_enum, default_value_t = DirectionArg::Outgoing)]
        direction: DirectionArg,
        #[arg(long, default_value_t = 3, value_name = "N")]
        depth: usize,
        #[arg(long, default_value_t = 1, value_name = "N")]
        min_synapses: u64,
        #[arg(long, default_value_t = 200, value_name = "N")]
        max_nodes: usize,
        #[arg(long, value_enum, default_value_t = GraphFormatArg::Svg)]
        format: GraphFormatArg,
        #[arg(long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Run a transparent heuristic signal -> reaction -> signal propagation.
    Simulate {
        #[arg(long, value_name = "ROOT_ID")]
        input: u64,
        #[arg(long, value_name = "LIGAND")]
        ligand: Option<String>,
        #[arg(long, default_value_t = 1.0, value_name = "X")]
        intensity: f64,
        #[arg(long, default_value_t = 0.85, value_name = "FACTOR")]
        decay: f64,
        #[arg(long, default_value_t = 3, value_name = "N")]
        depth: usize,
        #[arg(long, default_value_t = 1, value_name = "N")]
        min_synapses: u64,
        #[arg(long, default_value_t = 1000, value_name = "N")]
        max_steps: usize,
    },
    /// Show metadata and connections for one FlyWire root ID.
    Neuron {
        root_id: u64,
        #[arg(long, default_value_t = 20, value_name = "N")]
        limit: usize,
        #[arg(long, default_value_t = 1, value_name = "N")]
        min_synapses: u64,
        #[arg(long, value_enum, default_value_t = DirectionArg::Outgoing)]
        direction: DirectionArg,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DirectionArg {
    Incoming,
    Outgoing,
    Both,
}

impl DirectionArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::Incoming => "incoming",
            Self::Outgoing => "outgoing",
            Self::Both => "both",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CircusModelArg {
    Synthetic,
    Flywire,
    External,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ChannelKindArg {
    All,
    Input,
    Output,
    Internal,
    Endocrine,
    Unknown,
}

impl ChannelKindArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Input => "input",
            Self::Output => "output",
            Self::Internal => "internal",
            Self::Endocrine => "endocrine",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GraphFormatArg {
    Svg,
    Dot,
    Mermaid,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SearchField {
    All,
    Label,
    CellType,
    Classification,
    Tag,
    Group,
    Neurotransmitter,
}

impl SearchField {
    fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Label => "label",
            Self::CellType => "cell_type",
            Self::Classification => "classification",
            Self::Tag => "tag",
            Self::Group => "group",
            Self::Neurotransmitter => "neurotransmitter",
        }
    }
}

impl From<DirectionArg> for NeuronDirection {
    fn from(value: DirectionArg) -> Self {
        match value {
            DirectionArg::Incoming => Self::Incoming,
            DirectionArg::Outgoing => Self::Outgoing,
            DirectionArg::Both => Self::Both,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let data_dir = cli.data_dir.as_path();
    let database = resolve_database(data_dir, cli.db.as_deref());

    match cli.command.unwrap_or(Command::Summary) {
        Command::Editor { port, flies } => {
            editor::serve(port, flies)?;
        }
        Command::Circus {
            ticks,
            dt,
            seed,
            output,
            model,
            input_root,
            min_synapses,
            model_command,
            model_args,
        } => {
            let report = match model {
                CircusModelArg::Synthetic => {
                    runtime::run_synthetic_circus(ticks, dt, seed, output.as_deref())?
                }
                CircusModelArg::Flywire => {
                    ensure_index(&database, cli.db.is_some(), data_dir)?;
                    let root_id =
                        input_root.context("--input-root is required for --model flywire")?;
                    runtime::run_flywire_circus(
                        &database,
                        root_id,
                        ticks,
                        dt,
                        seed,
                        min_synapses,
                        output.as_deref(),
                    )?
                }
                CircusModelArg::External => {
                    let command = model_command
                        .context("--model-command is required for --model external")?;
                    runtime::run_external_circus(
                        &command,
                        &model_args,
                        ticks,
                        dt,
                        seed,
                        output.as_deref(),
                    )?
                }
            };
            if cli.json {
                print_json(&report)?;
            } else {
                println!(
                    "Ran {} ticks with {}; events: {}",
                    report.ticks, report.model, report.events_written
                );
                if let Some(output) = report.output {
                    println!("event log: {output}");
                }
                println!("final position: {:?}", report.final_state.position);
                println!("note: {}", report.note);
            }
        }
        Command::Summary => {
            let summary = flywire::summarize(data_dir)?;
            if cli.json {
                print_json(&summary)?;
            } else {
                print_summary(&summary);
            }
        }
        Command::Verify => {
            let report = flywire::verify_manifest(data_dir)?;
            if cli.json {
                print_json(&report)?;
            } else {
                println!(
                    "Verified {} files ({} bytes) in {}",
                    report.verified_files, report.total_bytes, report.data_dir
                );
            }
        }
        Command::Validate => {
            let report = flywire::validate_data(data_dir)?;
            if cli.json {
                print_json(&report)?;
            } else {
                println!(
                    "Validated {} ({}) — {} files, all rows readable",
                    report.dataset,
                    report.snapshot,
                    report.files.len()
                );
                for file in report.files {
                    println!("  {:<38} {:>10} rows", file.name, file.rows);
                }
            }
        }
        Command::Import { force } => {
            let report = index::import_dataset(data_dir, &database, force)?;
            if cli.json {
                print_json(&report)?;
            } else {
                print_import(&report);
            }
        }
        Command::Tfly {
            scenario,
            seed,
            trace,
        } => {
            let report = run_tfly(&scenario, seed, trace)?;
            if cli.json {
                print_json(&report)?;
            } else {
                print_tfly(&report);
            }
        }
        Command::Stats => {
            ensure_index(&database, cli.db.is_some(), data_dir)?;
            let stats = index::stats(&database)?;
            if cli.json {
                print_json(&stats)?;
            } else {
                print_stats(&stats);
            }
        }
        Command::Top { limit, by_synapses } => {
            ensure_index(&database, cli.db.is_some(), data_dir)?;
            let neurons = index::top_neurons(&database, limit, by_synapses)?;
            if cli.json {
                print_json(&neurons)?;
            } else {
                print_top(&neurons, by_synapses);
            }
        }
        Command::Connections {
            root_id,
            limit,
            min_synapses,
            direction,
        } => {
            let direction = direction.into();
            let page = if database.exists() {
                index::validate_index(&database, data_dir)?;
                let conn = index::open_db(&database)?;
                index::query_connections(&conn, root_id, direction, limit, min_synapses)?
            } else {
                if cli.db.is_some() {
                    bail!("SQLite database does not exist: {}", database.display());
                }
                flywire::query_connections(data_dir, root_id, direction, limit, min_synapses)?
            };
            if cli.json {
                print_json(&page)?;
            } else {
                print_connections(&page);
            }
        }
        Command::Search {
            query,
            field,
            limit,
        } => {
            ensure_index(&database, cli.db.is_some(), data_dir)?;
            let hits = index::search(&database, &query, field.as_str(), limit)?;
            if cli.json {
                print_json(&hits)?;
            } else {
                print_search(&hits);
            }
        }
        Command::Nblast { root_id, limit } => {
            ensure_index(&database, cli.db.is_some(), data_dir)?;
            let hits = index::nblast(&database, root_id, limit)?;
            if cli.json {
                print_json(&hits)?;
            } else {
                print_nblast(&hits);
            }
        }
        Command::Channels { kind, limit } => {
            ensure_index(&database, cli.db.is_some(), data_dir)?;
            let report = biology::channel_report(&database, kind.as_str(), limit)?;
            if cli.json {
                print_json(&report)?;
            } else {
                print_channels(&report);
            }
        }
        Command::Ligands => {
            ensure_index(&database, cli.db.is_some(), data_dir)?;
            let report = biology::ligand_report(&database)?;
            if cli.json {
                print_json(&report)?;
            } else {
                print_ligands(&report);
            }
        }
        Command::Graph {
            root_id,
            direction,
            depth,
            min_synapses,
            max_nodes,
            format,
            output,
        } => {
            ensure_index(&database, cli.db.is_some(), data_dir)?;
            let report = biology::collect_graph(
                &database,
                root_id,
                direction.as_str(),
                depth,
                min_synapses,
                max_nodes,
            )?;
            if cli.json && output.is_none() {
                print_json(&report)?;
            } else {
                let rendered = match format {
                    GraphFormatArg::Svg => biology::render_svg(&report),
                    GraphFormatArg::Dot => biology::render_dot(&report),
                    GraphFormatArg::Mermaid => biology::render_mermaid(&report),
                };
                if let Some(path) = output {
                    fs::write(&path, rendered)
                        .with_context(|| format!("writing graph to {}", path.display()))?;
                    println!("Graph written to {}", path.display());
                } else {
                    println!("{rendered}");
                }
            }
        }
        Command::Simulate {
            input,
            ligand,
            intensity,
            decay,
            depth,
            min_synapses,
            max_steps,
        } => {
            ensure_index(&database, cli.db.is_some(), data_dir)?;
            let report = biology::simulate(
                &database,
                biology::SimulationOptions {
                    input_root_id: input,
                    input_ligand: ligand,
                    initial_intensity: intensity,
                    decay,
                    max_depth: depth,
                    min_synapses,
                    max_steps,
                },
            )?;
            if cli.json {
                print_json(&report)?;
            } else {
                print_simulation(&report);
            }
        }
        Command::Neuron {
            root_id,
            limit,
            min_synapses,
            direction,
        } => {
            let direction = direction.into();
            let details = if database.exists() {
                index::validate_index(&database, data_dir)?;
                index::load_neuron(&database, root_id, direction, limit, min_synapses)?
                    .with_context(|| {
                        format!("neuron {root_id} is not present in the SQLite index")
                    })?
            } else {
                if cli.db.is_some() {
                    bail!("SQLite database does not exist: {}", database.display());
                }
                flywire::load_neuron(data_dir, root_id, direction, limit, min_synapses)?
            };
            if cli.json {
                print_json(&details)?;
            } else {
                print_neuron(&details);
            }
        }
    }

    Ok(())
}

fn ensure_index(database: &Path, explicitly_requested: bool, data_dir: &Path) -> Result<()> {
    if database.exists() {
        return index::validate_index(database, data_dir);
    }
    if explicitly_requested {
        bail!("SQLite database does not exist: {}", database.display());
    }
    bail!(
        "SQLite index is required for this command; run `neurohub import` first: {}",
        database.display()
    )
}

fn resolve_database(data_dir: &Path, requested: Option<&Path>) -> PathBuf {
    requested
        .map(Path::to_path_buf)
        .unwrap_or_else(|| index::default_db_path(data_dir))
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    let mut value = serde_json::to_value(value)?;
    stringify_ids(&mut value);
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn stringify_ids(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                if (key == "root_id"
                    || key.ends_with("_root_id")
                    || key == "supervoxel_id"
                    || key == "label_id")
                    && value.is_number()
                    && let Some(number) = value.as_u64()
                {
                    *value = serde_json::Value::String(number.to_string());
                }
                stringify_ids(value);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                stringify_ids(value);
            }
        }
        _ => {}
    }
}

fn print_summary(summary: &flywire::DatasetSummary) {
    println!("{} ({})", summary.dataset, summary.snapshot);
    println!("data: {}", summary.data_dir);
    println!("source: {}", summary.source);
    println!("citation: {}", summary.citation);
    println!("neurons: {}", summary.neuron_rows);
    println!("connection rows: {}", summary.connection_rows);
    println!("synapses: {}", summary.synapse_count);
    println!("\nfiles:");
    for file in &summary.files {
        println!(
            "  {:<38} {:>12} bytes  {:>10} rows",
            file.name, file.bytes, file.rows
        );
    }
}

fn print_channels(report: &biology::ChannelReport) {
    println!(
        "{}: {} returned / {} total channels",
        report.kind, report.count, report.total_channels
    );
    for channel in &report.channels {
        println!(
            "{} | {} | {} | in={} out={} | in_syn={} out_syn={} | {}",
            channel.root_id,
            channel.kind.label(),
            channel.group,
            channel.in_degree,
            channel.out_degree,
            channel.in_synapses,
            channel.out_synapses,
            channel.labels.join("; ")
        );
    }
}

fn print_ligands(report: &biology::LigandReport) {
    println!(
        "catalog entries: {} (including unobserved standards)",
        report.catalog_size
    );
    println!("observed normalized labels:");
    for ligand in &report.observed {
        println!(
            "{} | {} | {} | neurons={} | examples={:?} | {}",
            ligand.id,
            ligand.name,
            ligand.category,
            ligand.neuron_count,
            ligand.example_root_ids,
            ligand.evidence
        );
    }
    println!("note: {}", report.note);
}

fn print_simulation(report: &biology::SimulationReport) {
    println!(
        "input: {} channel={:?} intensity={} ligand={:?}",
        report.input.name, report.input.channel, report.input.intensity, report.input.ligand
    );
    println!("reactions: {}", report.reactions.len());
    for step in &report.reactions {
        println!(
            "{:>4}: {} -> {} | {} -> {} | strength={:.3} | {} ({}) | {} | target={}",
            step.index,
            step.from_root_id,
            step.to_root_id,
            step.input_intensity,
            step.output_intensity,
            step.edge_strength,
            step.transmitter,
            step.effect,
            step.neuropils.join(","),
            step.target_channel.label()
        );
    }
    println!(
        "visited nodes: {}{}; note: {}",
        report.visited_nodes,
        if report.truncated { " (truncated)" } else { "" },
        report.model_note
    );
}

fn print_search(hits: &[index::SearchHit]) {
    if hits.is_empty() {
        println!("no matches");
        return;
    }
    for hit in hits {
        println!(
            "{} | {} | {} | {}",
            hit.root_id, hit.group, hit.nt_type, hit.matched_text
        );
    }
}

fn print_nblast(hits: &[index::NblastHit]) {
    if hits.is_empty() {
        println!("no nblast hits");
        return;
    }
    for hit in hits {
        println!("{} | score {}", hit.target_root_id, hit.score);
    }
}

fn print_import(report: &index::ImportReport) {
    println!("Indexed {} into {}", report.dataset, report.database);
    println!("files: {}", report.files_imported);
    println!("neurons: {}", report.neuron_rows);
    println!("connection rows: {}", report.connection_rows);
    println!("synapses: {}", report.synapse_count);
    println!("database size: {} bytes", report.database_bytes);
}

/// A single observation of the fly during a scripted scenario.
#[derive(Debug, Serialize)]
struct TflyFrame {
    seconds: f32,
    mood: String,
    drive: String,
    valence: f32,
    arousal: f32,
    energy: f32,
    stress: f32,
    pain: f32,
    learning_gate: f32,
    fear: f32,
    joy: f32,
    curiosity: f32,
    hunger: f32,
    thrust: f32,
    turn: f32,
    wingbeat: f32,
    note: String,
}

#[derive(Debug, Serialize)]
struct TflyReport {
    scenario: String,
    seed: u32,
    core: String,
    struct_bytes: i32,
    frames: Vec<TflyFrame>,
    learned_weight: f32,
    control_weight: f32,
    json: String,
}

/// Run one of the built-in scenarios against the TFLY C core.
fn run_tfly(scenario: &str, seed: u32, trace: f32) -> Result<TflyReport> {
    use flytest::tfly::{Fly, action, body, cue, emotion};

    let mut fly = Fly::new();
    fly.seed(seed);
    let mut frames: Vec<TflyFrame> = Vec::new();
    let dt = 1.0 / 60.0;

    let sample = |fly: &Fly, seconds: f32, note: &str, frames: &mut Vec<TflyFrame>| {
        // Labelled samples are always recorded. `--trace` adds a periodic
        // sample every N simulated seconds on top of them.
        let labelled = frames.last().map(|f: &TflyFrame| f.note.as_str()) != Some(note);
        let periodic = trace > 0.0 && (seconds / trace - frames.len() as f32) >= 1.0;
        if labelled || periodic {
            frames.push(TflyFrame {
                seconds,
                mood: fly.dominant_emotion().to_owned(),
                drive: fly.dominant_drive().to_owned(),
                valence: fly.valence(),
                arousal: fly.arousal(),
                energy: fly.energy(),
                stress: fly.stress(),
                pain: fly.pain_total(),
                learning_gate: fly.learning_gate(),
                fear: fly.emotion_level(emotion::FEAR),
                joy: fly.emotion_level(emotion::JOY),
                curiosity: fly.emotion_level(emotion::CURIOSITY),
                hunger: fly.drive_level(flytest::tfly::drive::HUNGER),
                thrust: fly.out_thrust(),
                turn: fly.out_turn(),
                wingbeat: fly.wingbeat(),
                note: note.to_owned(),
            });
        }
    };

    match scenario {
        // Does the fly form an association, and does the modulator matter?
        "learn" => {
            for trial in 0..40 {
                fly.odor(0.6, flytest::tfly::odor::FRUIT);
                fly.steps(20, dt);
                sample(&fly, fly.elapsed(), &format!("trial {trial}"), &mut frames);
                fly.taste(0.8, 0.0, 0.0, 0.0);
                fly.reward(0.6);
                fly.associate(cue::ODOR_FRUIT, action::FORWARD, 1.0, fly.learning_gate());
                fly.steps(20, dt);
            }
        }
        // Can a shock override an appetitive drive?
        "pain" => {
            fly.odor(0.7, flytest::tfly::odor::FLOWER);
            fly.steps(600, dt);
            sample(&fly, fly.elapsed(), "before injury", &mut frames);
            fly.heart(0.9, body::WING_L);
            fly.steps(120, dt);
            sample(&fly, fly.elapsed(), "after wing injury", &mut frames);
        }
        // Does fear suppress courtship, and does recovery restore it?
        "social" => {
            fly.mate_signal(1.0);
            fly.steps(300, dt);
            sample(&fly, fly.elapsed(), "mate present", &mut frames);
            fly.predator_signal(1.0);
            fly.steps(120, dt);
            sample(&fly, fly.elapsed(), "predator appears", &mut frames);
            fly.heal_all();
            fly.contentment(0.6);
            fly.steps(600, dt);
            sample(&fly, fly.elapsed(), "recovered", &mut frames);
        }
        other => bail!("unknown tfly scenario {other:?}; try learn, pain or social"),
    }

    // control: identical trials with the modulator removed must learn nothing
    let mut control = Fly::new();
    control.seed(seed);
    for _ in 0..40 {
        control.odor(0.6, flytest::tfly::odor::FRUIT);
        control.steps(20, dt);
        control.associate(cue::ODOR_FRUIT, action::FORWARD, 1.0, 0.0);
        control.steps(20, dt);
    }

    Ok(TflyReport {
        scenario: scenario.to_owned(),
        seed,
        core: format!(
            "TFLY.h v{}.{} ({} bytes of state per fly)",
            flytest::tfly::VERSION_MAJOR,
            flytest::tfly::VERSION_MINOR,
            Fly::struct_size()
        ),
        struct_bytes: Fly::struct_size(),
        frames,
        learned_weight: fly.assoc_weight(cue::ODOR_FRUIT, action::FORWARD),
        control_weight: control.assoc_weight(cue::ODOR_FRUIT, action::FORWARD),
        json: fly.to_json(),
    })
}

fn print_tfly(report: &TflyReport) {
    println!("{}", report.core);
    println!("scenario: {}  seed: {}", report.scenario, report.seed);
    if !report.frames.is_empty() {
        println!(
            "\n  {:>7}  {:<11}  {:<9}  {:>7}  {:>6}  {:>6}  note",
            "t", "mood", "drive", "valence", "thrust", "turn"
        );
        for frame in &report.frames {
            println!(
                "  {:>6.1}s  {:<11}  {:<9}  {:>+7.3}  {:>6.3}  {:>+6.3}  {}",
                frame.seconds,
                frame.mood,
                frame.drive,
                frame.valence,
                frame.thrust,
                frame.turn,
                frame.note
            );
        }
    }
    println!("\nassociative weight fruit -> forward");
    println!("  with modulator:    {:+.4}", report.learned_weight);
    println!(
        "  control, gate = 0: {:+.4}  (must be 0.0000)",
        report.control_weight
    );
    println!("\nstate: {}", report.json);
}

fn print_stats(stats: &index::NetworkStats) {
    println!("database: {}", stats.database);
    println!("neurons: {}", stats.neuron_count);
    println!("connection rows: {}", stats.connection_rows);
    println!("unique neuron pairs: {}", stats.unique_neuron_pairs);
    println!("synapses: {}", stats.synapse_count);
    println!("reciprocal directed pairs: {}", stats.reciprocal_pairs);
    println!("average raw degree: {:.3}", stats.average_raw_degree);
    println!("average unique degree: {:.3}", stats.average_unique_degree);
    println!("maximum out-degree (rows): {}", stats.max_out_degree);
    println!("maximum in-degree (rows): {}", stats.max_in_degree);
}

fn print_top(neurons: &[index::RankedNeuron], by_synapses: bool) {
    let sort = if by_synapses { "synapses" } else { "degree" };
    println!("top neurons by {sort}:");
    println!("rank\troot_id\tgroup\tNT\tout\tin\tsynapses");
    for (rank, neuron) in neurons.iter().enumerate() {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            rank + 1,
            neuron.root_id,
            neuron.group,
            neuron.nt_type,
            neuron.out_degree,
            neuron.in_degree,
            neuron.total_synapses
        );
    }
}

fn print_neuron(details: &NeuronDetails) {
    let neuron = &details.neuron;
    println!("root_id: {}", neuron.root_id);
    println!("group: {}", neuron.group);
    println!(
        "neurotransmitter: {} ({})",
        neuron.nt_type, neuron.nt_type_score
    );
    if let Some(classification) = &details.classification {
        println!(
            "classification: {} / {} / {}",
            classification.super_class, classification.class, classification.sub_class
        );
        println!(
            "flow: {}, side: {}, nerve: {}, hemilineage: {}",
            classification.flow,
            classification.side,
            classification.nerve,
            classification.hemilineage
        );
    } else {
        println!("classification: <none>");
    }
    if !details.cell_types.is_empty() {
        println!("cell types: {}", details.cell_types.join(", "));
    }
    if !details.connectivity_tags.is_empty() {
        println!(
            "connectivity tags: {}",
            details.connectivity_tags.join(", ")
        );
    }
    if let Some(stats) = &details.cell_stats {
        println!(
            "size: length={} nm, area={} nm², volume={} nm³",
            stats.length_nm, stats.area_nm, stats.size_nm
        );
    }
    if !details.labels.is_empty() {
        println!("labels:");
        for label in &details.labels {
            println!("  {} ({})", label.label, label.date_created);
        }
    }
    if !details.coordinates.is_empty() {
        println!("coordinates:");
        for coordinate in &details.coordinates {
            println!(
                "  {} (supervoxel {})",
                coordinate.position, coordinate.supervoxel_id
            );
        }
    }
    print_connections(&details.connections);
}

fn print_connections(page: &ConnectionPage) {
    println!(
        "\n{} connections (synapses >= {}): {} raw rows, {} partners, {} synapses, {} returned{}",
        match page.direction {
            Direction::Incoming => "incoming",
            Direction::Outgoing => "outgoing",
            Direction::Both => "incoming/outgoing",
        },
        page.min_synapses,
        page.matched,
        page.unique_partners,
        page.total_synapses,
        page.returned,
        if page.truncated { " (truncated)" } else { "" }
    );
    for connection in &page.connections {
        let partner = if connection.pre_root_id == page.root_id {
            connection.post_root_id
        } else {
            connection.pre_root_id
        };
        println!(
            "  {} -> {} | {} | {} synapses | {}",
            connection.pre_root_id,
            connection.post_root_id,
            connection.neuropil,
            connection.syn_count,
            connection.nt_type
        );
        println!("    partner: {partner}");
    }
}
