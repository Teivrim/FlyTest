# Signal model and channel normalization

## Channel classes

The tool derives a structural channel class from the FlyWire classification table:

- `input`: `flow=afferent` or sensory/ascending/visual-projection classes;
- `output`: `flow=efferent`, motor, or descending classes;
- `endocrine`: endocrine/neurosecretory classes;
- `internal`: intrinsic neurons;
- `unknown`: metadata does not support a safe classification.

This is a routing/standardization layer, not a claim that every neuron in a class performs the same biological function.

## Ligand normalization

Ligand labels are canonicalized to stable IDs (`ACH`, `GABA`, `GLUT`, `DA`, `SER`, `OCT`, `DILP`, `DH31`, `ITP`, `CRZ`, `CAPA`, and others). The catalog distinguishes:

- neurotransmitters;
- neuropeptides;
- hormones such as ecdysone and juvenile hormone.

`ligands` reports only associations found in the FAFB tables separately from catalog entries that are standards but were not observed in this snapshot. A label or cell type is evidence of an association, **not** a measured secretion rate, receptor activity, hormone concentration, or production flux.

## Signal → reaction → signal

`simulate` accepts a source root ID, optional ligand, intensity, decay, depth, and synapse threshold. It walks the directed structural graph and emits:

1. an input `SignalEvent`;
2. `ReactionStep` records for each weighted edge;
3. propagated output `SignalEvent` records.

The deliberately transparent default transfer function is:

```text
edge_strength = 1 - exp(-syn_count / 10)
next_intensity = current_intensity * decay * edge_strength
```

The `effect` label is also a coarse heuristic (`GABA` → inhibitory, `ACH`/`GLUT` → excitatory, monoamines/peptides → modulatory); it is not a receptor-level measurement.

This is a heuristic graph-propagation model. FAFB v783 does not contain time-resolved membrane voltage, calcium, neurotransmitter release, hormone concentration, receptor state, or behavioral measurements. Therefore the output is a reproducible exploratory signal pathway, not a validated electrophysiological or endocrine simulation.

To build a quantitative model, add measured time-series/reaction parameters as a separate dataset and replace the transfer function with a calibrated dynamical model.
