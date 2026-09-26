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

## The course, as a client sees it

The editor broadcasts one course per act in `EditorSnapshot.courses`, because all five
stages are on screen at once. Each is stage-local, so a client places it on the stage it
already builds and does not need to know where the stages are:

```json
{
  "act": 0,
  "round": 2,
  "seed": 712164981,
  "time_left": 41.6,
  "round_length": 62.0,
  "food": [1.17, -0.39],
  "start": [-0.39, -1.17],
  "cell": 0.782,
  "path_length": 8.6,
  "walls": [
    {"x": -0.72, "z": -0.39, "half_len": 0.39, "half_thick": 0.086, "angle": 0.0}
  ],
  "fed": 1,
  "count": 3,
  "runners": [
    {"id": 1, "fed": false, "distance": 2.22, "progress": -0.1, "score": 1,
     "trail": [[-1.17, -0.39], [-1.05, -0.21]]}
  ]
}
```

`walls` are the maze, as oriented slabs. Collision is derived from these same segments, so
a wall a client draws and a wall that stops a character cannot be two different objects.
`runners` carries only the characters on that act, with their trails stage-local and
newest last; the trail is a lure for the others and a breadcrumb for the renderer.

`solution` is deliberately absent. It is kept server-side for scoring and is never sent:
handed the route, a character walks it in a straight line and looks like a cursor.

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
