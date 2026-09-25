# FlyTest Blender bridge

This directory contains the Blender-side adapter for the FlyTest runtime.

## Headless smoke test

Generate a synthetic event stream:

```bash
cargo run --release -- circus --ticks 600 --output runtime-output/events.jsonl
```

Bake it into a new `.blend` scene:

```bash
blender --background \
  --python blender/flytest_bridge.py -- \
  --events runtime-output/events.jsonl \
  --output runtime-output/flytest.blend \
  --fps 60
```

On Windows, replace `blender` with the full path to `blender.exe`, for example:

```powershell
& "C:\Program Files\Blender Foundation\Blender 4.3\blender.exe" `
  --background `
  --python blender\flytest_bridge.py -- `
  --events runtime-output\events.jsonl `
  --output runtime-output\flytest.blend
```

The bridge:

- reuses an existing scene when possible;
- creates a `FlyTest` collection and a simple fly/arena when objects are absent;
- maps `world.position`, `action.turn`, `action.vertical`, `throttle`, and `wingbeat_hz` to Blender transform keyframes;
- maps observation light to the body emission;
- saves all animation as keyframes, so the scene can be inspected without a live bridge.

## Scene contract

The bridge looks up these object names when a scene already exists:

- `FlyTest_Body`;
- `FlyTest_Wing_L`;
- `FlyTest_Wing_R`;
- `FlyTest_Eye_L`;
- `FlyTest_Eye_R`;
- `FlyTest_Camera`;
- `FlyTest_KeyLight`.

If a scene has no such objects, a simple arena and fly are generated. This makes the repository runnable before the final Blender art is supplied.
