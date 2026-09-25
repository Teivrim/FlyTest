# FlyTest runtime protocol

The runtime is deliberately model-agnostic. A world emits an `Observation`, a brain/model emits an `Action`, and the runtime writes both to JSONL for Blender or another client.

## Observation

```json
{
  "tick": 42,
  "dt": 0.0166667,
  "light": 0.5,
  "odor": 0.7,
  "taste": 0.0,
  "touch": 0.0,
  "temperature": 0.5,
  "gravity": 1.0,
  "proprioception": 0.1,
  "position": [0.0, 0.0, 1.0]
}
```

## Action

```json
{
  "tick": 42,
  "throttle": 0.6,
  "turn": -0.2,
  "vertical": 0.0,
  "wingbeat_hz": 110.0,
  "hormone_pulses": [
    {"ligand": "DA", "amplitude": 0.1, "duration_ticks": 8}
  ]
}
```

## Model adapter contract

A Rust model implements `BrainModel`:

```rust
pub trait BrainModel {
    fn name(&self) -> &str;
    fn reset(&mut self);
    fn step(&mut self, observation: &Observation) -> Action;
}
```

External models can use the same JSONL protocol without linking into Rust. Start one with:

```bash
cargo run --release -- circus \
  --model external \
  --model-command python \
  --model-arg models/example_policy.py \
  --ticks 600 \
  --output runtime-output/events.jsonl
```

The executable must read one observation per line from stdin and write one action per line to stdout. A Python/PyTorch/ONNX model can use this interface; a Blender client can consume the resulting events independently.

The built-in `SyntheticFlyModel` is only a deterministic protocol demo. It is not a biological claim about fly behavior.
