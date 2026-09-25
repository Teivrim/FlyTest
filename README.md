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

## FlyEditor

Запусти графический редактор:

```bash
cargo run --release -- editor --port 8765 --flies 3
```

Открой `http://127.0.0.1:8765`. В центре — арена с 2–3 мухами; слева — список агентов, справа — input/reaction/hormone/energy inspector и управление скоростью/decay. Runtime работает на fixed timestep 60 Hz, использует лёгкую политику и не пишет каждый кадр в SQLite/JSONL. Это режим, рассчитанный на 2–3 мух на текущем ПК.

FlyEditor не запускает полный FlyWire-граф в каждом тике: для него используется облегчённый realtime-адаптер. Полный граф подключается через `circus --model flywire` и Blender bridge.

### Цифровой цирк

В правой панели FlyEditor есть блок `DIGITAL CIRCUS` — переключаемые приключения с подвижной целью в 3D-сцене:

| Приключение | Что происходит |
| --- | --- |
| `free_flight` | Свободный полёт, цель скрыта |
| `odor_trail` | Цель следует по синусоидальному запаховому следу |
| `ring_circuit` | Цель обходит арену по кольцу |
| `hormone_calibration` | Цель движется медленно, проверяется сбор сигнала |

Муха получает `+0.08` reward, когда пролетает ближе 0.85 к цели. Счёт, прогресс и цель видны и в панели, и в сцене (оранжевый маркер с пульсацией).

### Обучение

Каждая муха имеет собственный адаптивный policy-вектор:

- `reward` — EMA полученной награды, растёт при выполнении цели и по кнопке `Награда`;
- `novelty` — EMA исследовательской новизны, растёт при низкой награде;
- `strategy` — переключается между `balanced`, `exploit_reward` и `explore_novelty` по порогам `reward > 0.6` и `novelty > 0.68`.

Кнопка `Награда` штрафует/поощряет выбранную муху вручную, `Puff` вызывает событие. Счётчики событий: `metrics.total_puff_events`, `metrics.learning_updates`.

Это адаптивная эвристика поведения, а не обучение нейросети: веса обновляются онлайн по сигналу награды, без backpropagation.

### «Puff»

Триггерное событие: муха периодически (раз в ~6 с sim-времени, со своим фазовым сдвигом) выпускает зелёное облако, которое рассеивается ~2 с и слегка повышает её `stress`. В 3D это три полупрозрачных сферы, растущих и растворяющихся за корпусом. Кнопка `Puff` вызывает событие вручную.

Это стилизованная механика цирка, не биологическая модель выделения газов у насекомых.


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
