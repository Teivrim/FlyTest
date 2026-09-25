# FlyTest

FlyTest — локальный цифровой цирк для виртуальной мухи: Rust runtime с сенсорным вводом, управляющими действиями и гормональными импульсами, плюс слой анализа FlyWire FAFB v783.

Цель проекта — сделать замкнутый контур:

```text
виртуальный мир → Observation → BrainModel → Action → Blender/мир → новый Observation
```

## Что уже реализовано

- `SyntheticFlyModel` и `ToyCircus` — автономный демонстрационный цикл;
- JSONL-протокол для обмена с внешними моделями;
- Blender bridge, который создаёт простую сцену и запекает анимацию;
- каталог адаптеров для PyTorch/ONNX/spiking/RL-моделей;
- импорт и анализ FlyWire-графа через SQLite;
- нормализация каналов `input/output/internal/endocrine`;
- нормализация нейромедиаторов, нейропептидов и гормональных стандартов;
- эвристический `signal → reaction → signal` по структурному графу.

## Быстрый запуск цирка

```bash
cargo run --release -- circus --ticks 600 --output runtime-output/events.jsonl
```

На Windows можно запустить smoke-demo одной командой:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run_demo.ps1
```

Для Blender добавь `-WithBlender` (и укажи путь к executable через `-Blender`, если `blender` не находится в `PATH`).

Это не требует Blender и создаёт полностью воспроизводимый поток событий. В каждой строке JSONL находятся:

- сенсорное наблюдение;
- действие мухи;
- гормональные импульсы;
- состояние виртуального мира.

## Blender-сцена

Установи Blender 3.6+ и выполни:

```bash
blender --background \
  --python blender/flytest_bridge.py -- \
  --events runtime-output/events.jsonl \
  --output runtime-output/flytest.blend \
  --fps 60
\]

Bridge создаёт `FlyTest` collection, простую арену, тело мухи, крылья, глаза, камеру и свет. Если объекты с именами `FlyTest_Body`, `FlyTest_Wing_L`, `FlyTest_Wing_R` уже существуют, он использует их.

Подробности: [`blender/README.md`](blender/README.md).

## FlyWire-инструменты

Данные не коммитятся в Git. Укажи локальный каталог через `--data-dir` или переменную окружения:

```bash
cargo run --release -- \
  --data-dir "D:\SOOBSHESTVA\AUTOMATIC\NeiroHub\data\flywire\fafb-v783" \
  import --force
```

После импорта создаётся локальный индекс `data/flywire/fafb-v783/flytest.sqlite`. Команды:

```bash
cargo run --release -- stats
cargo run --release -- channels --kind input
cargo run --release -- ligands
cargo run --release -- graph 720575940599457990 --format svg --output graph.svg
cargo run --release -- simulate --input 720575940599457990 --ligand ACH
cargo run --release -- circus --model flywire --input-root 720575940599457990 --ticks 600
```

## Подключение своей модели

Контракт находится в [`docs/runtime-protocol.md`](docs/runtime-protocol.md). Внешняя модель получает `Observation` и возвращает `Action`; ей не нужно знать о Blender или FlyWire.

Внешняя модель запускается напрямую:

```bash
cargo run --release -- circus \
  --model external \
  --model-command python \
  --model-arg models/example_policy.py \
  --ticks 600 \
  --output runtime-output/events.jsonl
```

В репозитории есть минимальный пример [`models/example_policy.py`](models/example_policy.py). Его можно заменить на:

- PyTorch policy;
- ONNX Runtime;
- spiking neural network;
- reinforcement-learning агента;
- модель, обученную на собственных данных.

## Научное ограничение

FlyWire даёт структурный коннектом и coarse-аннотации, но не содержит временных рядов мембранного потенциала, концентраций гормонов, состояния рецепторов и поведенческих измерений. Поэтому встроенная модель — воспроизводимый структурный прототип, а не перенос субъективного сознания или количественная биологическая симуляция.

Подробности: [`docs/signal-model.md`](docs/signal-model.md).

## Проверки

```bash
cargo fmt --all -- --check
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
python -m py_compile blender/flytest_bridge.py models/example_policy.py
```
