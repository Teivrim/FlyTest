use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub tick: u64,
    pub dt: f64,
    pub light: f64,
    pub odor: f64,
    pub taste: f64,
    pub touch: f64,
    pub temperature: f64,
    pub gravity: f64,
    pub proprioception: f64,
    pub position: [f64; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HormonePulse {
    pub ligand: String,
    pub amplitude: f64,
    pub duration_ticks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub tick: u64,
    pub throttle: f64,
    pub turn: f64,
    pub vertical: f64,
    pub wingbeat_hz: f64,
    pub hormone_pulses: Vec<HormonePulse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldState {
    pub tick: u64,
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub energy: f64,
    pub stress: f64,
    pub light: f64,
    pub odor: f64,
    pub taste: f64,
    pub touch: f64,
    pub temperature: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeEvent {
    pub event: String,
    pub observation: Observation,
    pub action: Action,
    pub world: WorldState,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeReport {
    pub model: String,
    pub ticks: u64,
    pub dt: f64,
    pub events_written: u64,
    pub output: Option<String>,
    pub final_state: WorldState,
    pub note: String,
}

pub trait BrainModel {
    fn name(&self) -> &str;
    fn reset(&mut self);
    fn step(&mut self, observation: &Observation) -> Action;
}

pub struct ExternalModel {
    name: String,
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl ExternalModel {
    pub fn start(command: &Path, arguments: &[String]) -> Result<Self> {
        let mut child = Command::new(command)
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .with_context(|| format!("starting external model: {}", command.display()))?;
        let input = child
            .stdin
            .take()
            .context("external model stdin was not piped")?;
        let output = child
            .stdout
            .take()
            .context("external model stdout was not piped")?;
        Ok(Self {
            name: format!("external:{}", command.display()),
            child,
            input,
            output: BufReader::new(output),
        })
    }
}

impl BrainModel for ExternalModel {
    fn name(&self) -> &str {
        &self.name
    }

    fn reset(&mut self) {}

    fn step(&mut self, observation: &Observation) -> Action {
        serde_json::to_writer(&mut self.input, observation).expect("serialize observation");
        self.input
            .write_all(b"\n")
            .expect("write observation newline");
        self.input.flush().expect("flush observation");
        let mut line = String::new();
        self.output
            .read_line(&mut line)
            .expect("read model response");
        if line.trim().is_empty() {
            return Action {
                tick: observation.tick,
                throttle: 0.0,
                turn: 0.0,
                vertical: 0.0,
                wingbeat_hz: 0.0,
                hormone_pulses: Vec::new(),
            };
        }
        let mut action: Action = serde_json::from_str(line.trim()).expect("decode model action");
        action.tick = observation.tick;
        action
    }
}

impl Drop for ExternalModel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Debug, Clone)]
pub struct SyntheticFlyModel {
    phase: f64,
    stress: f64,
    energy: f64,
}

impl SyntheticFlyModel {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            stress: 0.0,
            energy: 1.0,
        }
    }
}

impl Default for SyntheticFlyModel {
    fn default() -> Self {
        Self::new()
    }
}

impl BrainModel for SyntheticFlyModel {
    fn name(&self) -> &str {
        "synthetic-fly-policy"
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.stress = 0.0;
        self.energy = 1.0;
    }

    fn step(&mut self, observation: &Observation) -> Action {
        self.phase += observation.dt;
        let odor_error = observation.odor - 0.5;
        let light_error = observation.light - 0.5;
        let touch_response = observation.touch * 0.8;
        self.stress = (self.stress + observation.touch * 0.02).clamp(0.0, 1.0);
        self.energy = (self.energy - observation.dt * 0.002).clamp(0.0, 1.0);
        let turn = (odor_error * 1.4 + light_error * 0.35 - touch_response).clamp(-1.0, 1.0);
        let throttle = (0.25 + observation.light * 0.35 + self.energy * 0.2).clamp(0.0, 1.0);
        let vertical = ((observation.temperature - 0.5) * 0.5).clamp(-1.0, 1.0);
        let mut hormone_pulses = Vec::new();
        if self.stress > 0.65 {
            hormone_pulses.push(HormonePulse {
                ligand: "DA".to_owned(),
                amplitude: self.stress,
                duration_ticks: 8,
            });
        }
        if observation.odor > 0.75 {
            hormone_pulses.push(HormonePulse {
                ligand: "OCT".to_owned(),
                amplitude: (observation.odor - 0.75) * 2.0,
                duration_ticks: 12,
            });
        }
        Action {
            tick: observation.tick,
            throttle,
            turn,
            vertical,
            wingbeat_hz: 95.0 + 35.0 * throttle + 10.0 * self.phase.sin(),
            hormone_pulses,
        }
    }
}

#[derive(Debug, Clone)]
struct GraphPolicyEdge {
    target_root_id: u64,
    syn_count: u64,
    transmitter: String,
}

pub struct FlyWireGraphModel {
    edges: Vec<GraphPolicyEdge>,
    phase: f64,
}

impl FlyWireGraphModel {
    pub fn from_database(database: &Path, root_id: u64, min_synapses: u64) -> Result<Self> {
        let conn = rusqlite::Connection::open(database)
            .with_context(|| format!("opening FlyWire index {}", database.display()))?;
        let root = i64::try_from(root_id).context("root ID does not fit SQLite INTEGER")?;
        let min = i64::try_from(min_synapses).unwrap_or(i64::MAX);
        let mut statement = conn.prepare(
            "SELECT post_root_id, SUM(syn_count), COALESCE(GROUP_CONCAT(DISTINCT nt_type), '')
             FROM connections WHERE pre_root_id = ?1 AND syn_count >= ?2
             GROUP BY post_root_id ORDER BY SUM(syn_count) DESC LIMIT 128",
        )?;
        let rows = statement.query_map(rusqlite::params![root, min], |row| {
            let target: i64 = row.get(0)?;
            let syn_count: i64 = row.get(1)?;
            let transmitter: String = row.get(2)?;
            Ok(GraphPolicyEdge {
                target_root_id: u64::try_from(target).map_err(|_| rusqlite::Error::InvalidQuery)?,
                syn_count: u64::try_from(syn_count).map_err(|_| rusqlite::Error::InvalidQuery)?,
                transmitter,
            })
        })?;
        let edges = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(anyhow::Error::from)?;
        ensure!(
            !edges.is_empty(),
            "FlyWire root {root_id} has no outgoing edges above threshold"
        );
        Ok(Self { edges, phase: 0.0 })
    }
}

impl BrainModel for FlyWireGraphModel {
    fn name(&self) -> &str {
        "flywire-graph-policy"
    }

    fn reset(&mut self) {
        self.phase = 0.0;
    }

    fn step(&mut self, observation: &Observation) -> Action {
        self.phase += observation.dt;
        let index = (observation.tick as usize) % self.edges.len();
        let edge = &self.edges[index];
        let target_signal = (edge.target_root_id % 1000) as f64 / 1000.0 - 0.5;
        let odor = observation.odor - 0.5;
        let turn = (odor * 1.2 + target_signal * 0.35 - observation.touch * 0.7).clamp(-1.0, 1.0);
        let strength = (edge.syn_count as f64).sqrt().min(20.0) / 20.0;
        let throttle = (0.2 + observation.light * 0.25 + strength * 0.35).clamp(0.0, 1.0);
        let mut hormone_pulses = Vec::new();
        if !edge.transmitter.is_empty() {
            hormone_pulses.push(HormonePulse {
                ligand: edge
                    .transmitter
                    .split(',')
                    .next()
                    .unwrap_or("unknown")
                    .to_owned(),
                amplitude: (0.02 + strength * 0.08).min(1.0),
                duration_ticks: 4,
            });
        }
        Action {
            tick: observation.tick,
            throttle,
            turn,
            vertical: ((observation.temperature - 0.5) * 0.4).clamp(-1.0, 1.0),
            wingbeat_hz: 90.0 + 45.0 * throttle + 5.0 * self.phase.sin(),
            hormone_pulses,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToyCircus {
    state: WorldState,
    dt: f64,
    seed: f64,
}

impl ToyCircus {
    pub fn new(dt: f64, seed: u64) -> Self {
        Self {
            state: WorldState {
                tick: 0,
                position: [0.0, 0.0, 0.0],
                velocity: [0.0, 0.0, 0.0],
                energy: 1.0,
                stress: 0.0,
                light: 0.5,
                odor: 0.5,
                taste: 0.0,
                touch: 0.0,
                temperature: 0.5,
            },
            dt,
            seed: seed as f64,
        }
    }

    pub fn observation(&self) -> Observation {
        Observation {
            tick: self.state.tick,
            dt: self.dt,
            light: self.state.light,
            odor: self.state.odor,
            taste: self.state.taste,
            touch: self.state.touch,
            temperature: self.state.temperature,
            gravity: 1.0,
            proprioception: self.state.velocity[0].abs() + self.state.velocity[1].abs(),
            position: self.state.position,
        }
    }

    pub fn apply(&mut self, action: &Action) {
        self.state.tick += 1;
        let turn = action.turn.clamp(-1.0, 1.0);
        let throttle = action.throttle.clamp(0.0, 1.0);
        self.state.velocity[0] += turn * self.dt * 1.8;
        self.state.velocity[1] += throttle * self.dt * 0.9;
        self.state.velocity[2] += action.vertical.clamp(-1.0, 1.0) * self.dt * 0.5;
        for index in 0..3 {
            self.state.velocity[index] *= 0.97;
            self.state.position[index] += self.state.velocity[index] * self.dt;
        }
        self.state.position[2] = self.state.position[2].max(0.0);
        self.state.energy =
            (self.state.energy - self.dt * (0.001 + throttle * 0.002)).clamp(0.0, 1.0);
        self.state.stress = (self.state.stress * 0.98).clamp(0.0, 1.0);
        for pulse in &action.hormone_pulses {
            self.state.stress = (self.state.stress + pulse.amplitude * 0.01).clamp(0.0, 1.0);
            self.state.energy = (self.state.energy + pulse.amplitude * 0.002).clamp(0.0, 1.0);
        }
        let phase = self.state.tick as f64 * self.dt + self.seed;
        self.state.light = (0.5 + 0.35 * phase.sin()).clamp(0.0, 1.0);
        self.state.odor = (0.5 + 0.3 * (phase * 0.7).sin()).clamp(0.0, 1.0);
        self.state.taste = (0.2 + 0.2 * (phase * 0.23).cos()).clamp(0.0, 1.0);
        self.state.touch = (self.state.velocity[0].abs() * 0.15).clamp(0.0, 1.0);
        self.state.temperature = (0.5 + 0.15 * (phase * 0.17).sin()).clamp(0.0, 1.0);
    }

    pub fn state(&self) -> &WorldState {
        &self.state
    }
}

pub fn run_circus<M: BrainModel>(
    mut model: M,
    ticks: u64,
    dt: f64,
    seed: u64,
    output: Option<&Path>,
) -> Result<RuntimeReport> {
    ensure!(ticks > 0, "ticks must be greater than zero");
    ensure!(dt.is_finite() && dt > 0.0, "dt must be finite and positive");
    let mut world = ToyCircus::new(dt, seed);
    model.reset();
    let mut writer = if let Some(path) = output {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating output directory {}", parent.display()))?;
        }
        Some(BufWriter::new(File::create(path).with_context(|| {
            format!("creating event log {}", path.display())
        })?))
    } else {
        None
    };

    for _ in 0..ticks {
        let observation = world.observation();
        let action = model.step(&observation);
        world.apply(&action);
        let event = RuntimeEvent {
            event: "tick".to_owned(),
            observation,
            action,
            world: world.state().clone(),
        };
        if let Some(writer) = &mut writer {
            serde_json::to_writer(&mut *writer, &event)?;
            writer.write_all(b"\n")?;
        }
    }
    if let Some(writer) = &mut writer {
        writer.flush()?;
    }

    Ok(RuntimeReport {
        model: model.name().to_owned(),
        ticks,
        dt,
        events_written: ticks,
        output: output.map(|path| path.display().to_string()),
        final_state: world.state().clone(),
        note: "ToyCircus is an executable protocol demo; replace SyntheticFlyModel with a calibrated brain/world adapter for biological experiments.".to_owned(),
    })
}

pub fn run_synthetic_circus(
    ticks: u64,
    dt: f64,
    seed: u64,
    output: Option<&Path>,
) -> Result<RuntimeReport> {
    run_circus(SyntheticFlyModel::new(), ticks, dt, seed, output)
}

pub fn run_external_circus(
    command: &Path,
    arguments: &[String],
    ticks: u64,
    dt: f64,
    seed: u64,
    output: Option<&Path>,
) -> Result<RuntimeReport> {
    let model = ExternalModel::start(command, arguments)?;
    run_circus(model, ticks, dt, seed, output)
}

pub fn run_flywire_circus(
    database: &Path,
    root_id: u64,
    ticks: u64,
    dt: f64,
    seed: u64,
    min_synapses: u64,
    output: Option<&Path>,
) -> Result<RuntimeReport> {
    let model = FlyWireGraphModel::from_database(database, root_id, min_synapses)?;
    run_circus(model, ticks, dt, seed, output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_circus_produces_requested_events() {
        let report = run_synthetic_circus(12, 0.01, 3, None).expect("run circus");
        assert_eq!(report.ticks, 12);
        assert_eq!(report.events_written, 12);
        assert!(
            report
                .final_state
                .position
                .iter()
                .all(|value| value.is_finite())
        );
    }

    #[test]
    fn action_contains_valid_hormone_pulses() {
        let mut model = SyntheticFlyModel::new();
        let observation = Observation {
            tick: 1,
            dt: 0.01,
            light: 0.5,
            odor: 0.9,
            taste: 0.0,
            touch: 0.0,
            temperature: 0.5,
            gravity: 1.0,
            proprioception: 0.0,
            position: [0.0; 3],
        };
        let action = model.step(&observation);
        assert!(action.wingbeat_hz > 0.0);
        assert!(action.hormone_pulses.iter().all(|pulse| {
            pulse.amplitude.is_finite() && pulse.amplitude >= 0.0 && pulse.duration_ticks > 0
        }));
    }
}
