# Model adapters

The runtime does not require a specific ML framework. A model adapter only needs to:

1. read an `Observation` JSON object;
2. return an `Action` JSON object;
3. preserve the same field names as `docs/runtime-protocol.md`.

`example_policy.py` is a dependency-free reference adapter. Replace its `step` method with a PyTorch, ONNX, spiking, reinforcement-learning, or FlyWire-backed policy.

The adapter boundary is intentionally small so the brain model can be swapped without rewriting the Blender scene.
