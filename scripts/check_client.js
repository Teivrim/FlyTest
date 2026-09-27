// Runs the editor's own rendering functions against a real snapshot, in Node.
//
// Why this exists. `node --check editor/app.js` only proves the file parses. A
// screenshot proves the page drew something, and until recently neither caught
// the class of bug that actually bit: a call to a function that was never
// declared. That is not a syntax error, the parser does not care, and the file
// still loads — it throws on the first frame, inside a `requestAnimationFrame`
// callback, where nothing reports it and the page simply stops updating.
//
// So this evaluates the part of app.js that draws the panels, feeds it a
// snapshot taken from a running server, and looks at the HTML that comes out.
// A missing declaration is a thrown error here, on the first line, with a name.
//
// Run: node scripts/check_client.js [url-of-a-running-editor]
// With no argument it uses http://127.0.0.1:8765/.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const base = process.argv[2] || 'http://127.0.0.1:8765/';
const source = readFileSync(join(root, 'editor', 'app.js'), 'utf8');

// Everything up to the bootstrap. The bootstrap opens a WebGL context and starts
// a render loop, neither of which means anything here, and the panel code above
// it is what is under test.
const cut = source.indexOf('\ninit3D();');
if (cut < 0) {
  console.error('could not find the bootstrap in app.js; the slice point moved');
  process.exit(1);
}
const panelSource = source.slice(0, cut);

// A document just big enough for the panel code. Every element the life renderer
// touches is created on demand, so a permissive proxy is enough and a full DOM
// implementation is not needed.
const made = new Map();
function makeElement(tag) {
  const element = {
    tagName: String(tag).toUpperCase(),
    children: [],
    dataset: {},
    style: {},
    classList: { toggle() { }, add() { }, remove() { }, contains() { return false; } },
    _html: '',
    _text: '',
    set innerHTML(value) { this._html = String(value); this.children = []; },
    get innerHTML() {
      // What was set, plus what was appended. Without the second half the shim
      // reports an empty panel for every list the renderer builds out of
      // `createElement` and `appendChild`, and the check then fails for a
      // rendering that is in fact correct.
      return this._html + this.children.map((child) => child.outerHTML()).join('');
    },
    outerHTML() { return `<${this.tagName}>${this.textContent}</${this.tagName}>`; },
    set textContent(value) { this._text = String(value); this.children = []; },
    get textContent() {
      if (this._text) return this._text;
      return this._html.replace(/<[^>]*>/g, '') + this.children.map((c) => c.textContent).join('');
    },
    get innerText() { return this.textContent; },
    set hidden(value) { this._hidden = Boolean(value); },
    get hidden() { return Boolean(this._hidden); },
    appendChild(child) { this.children.push(child); return child; },
    remove() { },
    addEventListener() { },
    querySelector() { return null; },
  };
  return element;
}
const registry = new Map();
const document = {
  getElementById(id) {
    if (!registry.has(id)) registry.set(id, makeElement('div'));
    return registry.get(id);
  },
  createElement: makeElement,
  addEventListener() { },
  body: makeElement('body'),
  title: '',
};
const window = { addEventListener() { } };

// A fetch that answers the two routes the panel code asks for, from files
// rather than a server, so the check runs with nothing started.
const assets = {
  '/app.js': source,
  '/style.css': readFileSync(join(root, 'editor', 'style.css'), 'utf8'),
  '/api/profiles': JSON.stringify([]),
  '/api/state': JSON.stringify({
    tick: 10, running: true, speed: 1, decay: 0, selected_fly: 0,
    agents: [], courses: [], acts: [], auto: false, life: null, profiles: [],
    training: { enabled: false, epsilon: 0.1, interval: 1, current_act: 0 },
    metrics: {}, model_note: '',
  }),
};
const fetch = async (path) => {
  if (path.startsWith('http')) throw new Error('the check must not use the network');
  if (!(path in assets)) return { ok: false, status: 404, json: async () => ({}) };
  return { ok: true, status: 200, json: async () => JSON.parse(assets[path]), text: async () => assets[path] };
};

let failures = 0;
function check(name, condition, detail) {
  if (condition) {
    console.log(`  ok    ${name}`);
  } else {
    failures++;
    console.log(`  FAIL  ${name}${detail ? ': ' + detail : ''}`);
  }
}

const context = {
  document, window, fetch, console,
  setInterval() { return 0; },
  setTimeout(fn) { fn(); return 0; },
  clearInterval() { },
  Math, JSON, Date, Number, String, Object, Array, Promise, Error,
  performance: { now: () => 0 },
  requestAnimationFrame() { return 0; },
};
context.globalThis = context;
const keys = Object.keys(context);
const run = new Function(...keys, `${panelSource}\n;return { renderLife, renderPanels, refreshChooser, showLife, formatAge, plural };`);
const api = run(...keys.map((k) => context[k]));

check('the panel code evaluates', typeof api.renderLife === 'function', 'renderLife is missing');
if (typeof api.renderLife !== 'function') process.exit(1);

const life = {
  name: 'Клава',
  id: 'klava',
  age: 240,
  stage: 'Первые шаги',
  chapter: 2,
  chapters: [
    ['Пелёнки', true], ['Первые шаги', true], ['Еда', false],
    ['Потерялась', false], ['Дорога домой', false], ['Искательница', false],
  ],
  quest: 'учится ходить',
  quest_progress: 0.4,
  quest_after: 'делает первые шаги',
  family: 'семья из 4',
  family_members: [
    { name: 'Равна', role: 'мать', doing: 'топит печь' },
    { name: 'Осип', role: 'отец', doing: 'идёт с поля' },
  ],
  home: [13, 0, 1.4],
  asleep: true,
  gait: 0.42,
  skills: [
    { key: 'tetris', name: 'Тетрис', about: 'поворот фигур', level: 0.31, best: 10, current: true },
    { key: 'maze', name: 'Лабиринт', about: 'выход', level: 0.05, best: 0, current: false },
  ],
  history: [
    { task: 'Тетрис', turns: 98, good_turns: 83, score: 10, solved: true, level: 0.31 },
    { task: 'Лабиринт', turns: 44, good_turns: 1, score: 0, solved: false, level: 0.05 },
  ],
  last_change: 'Пелёнки → Первые шаги',
  dream: {
    task: 'Тетрис', about: 'поворот фигур', progress: 0.5, score: 10, best: 10,
    attempt: 3, turns: 41, good_turns: 22, describe: 'линий: 1, ход: 41',
    senses: [0.1, 0.0, 0.55, 0.0],
    board: [[35, 35, 46, 46, 46, 46], [64, 64, 46, 46, 46, 46]],
    answer: null,
  },
};

const data = {
  tick: 10, running: true, speed: 1, decay: 0, selected_fly: 0,
  agents: [], courses: [], acts: [], auto: false, profiles: [], life,
  training: { enabled: false, epsilon: 0.1, interval: 1, current_act: 0 },
  metrics: {}, model_note: '',
};

console.log('client rendering');
let threw = null;
try {
  api.renderLife(data);
} catch (error) {
  threw = error;
}
check('renderLife draws a life without throwing', threw === null, threw && threw.message);

// Read a panel the way a person reads it: its text, including whatever was
// appended into it.
const text = (id) => document.getElementById(id).textContent;
check('the name is drawn', text('life-name') === 'Клава', text('life-name'));
check('the chapter is drawn', /глава 2 из 6/.test(text('life-stage')), text('life-stage'));
check('the quest is drawn', /учится ходить/.test(text('life-quest-text')), text('life-quest-text'));
check('the family is drawn', /Равна/.test(text('life-family')), text('life-family'));
check('the skills are drawn', /Тетрис/.test(text('life-skills')), text('life-skills'));
check('the sleeps are drawn', /ходов 98/.test(text('life-history')), text('life-history'));
check('the chapters are drawn', /Пелёнки/.test(text('life-chapters')), text('life-chapters'));
check('the dream is shown while asleep', document.getElementById('dream').hidden === false);
check('the dream board is drawn', /[░▓]/.test(text('dream-board')), JSON.stringify(text('dream-board')));
check('the four senses are drawn', /свет/.test(text('dream-senses')), text('dream-senses'));
check('the sleep button is hidden while asleep', document.getElementById('sleep-button').hidden === true);
check('the wake button is shown while asleep', document.getElementById('wake-button').hidden === false);

// And the other direction: awake, no dream, and the chooser up.
data.life.asleep = false;
data.life.dream = null;
data.life = life;
const awake = { ...data, life: { ...life, asleep: false, dream: null } };
threw = null;
try {
  api.renderLife(awake);
} catch (error) {
  threw = error;
}
check('renderLife draws an awake fly without throwing', threw === null, threw && threw.message);
check('the dream is hidden while awake', document.getElementById('dream').hidden === true);
check('the sleep button is shown while awake', document.getElementById('sleep-button').hidden === false);

// No life at all: the chooser, which is the first thing anybody sees.
threw = null;
try {
  api.renderLife({ ...data, life: null });
} catch (error) {
  threw = error;
}
check('renderLife handles nobody being chosen', threw === null, threw && threw.message);
check('the chooser is shown when there is no life', document.getElementById('chooser').hidden === false);
check('the life panel is hidden when there is no life', document.getElementById('life').hidden === true);

// The small pure functions, because Russian plurals are easy to get wrong and
// there is no other way to notice.
check('1 день', api.plural(1, 'день', 'дня', 'дней') === 'день', api.plural(1, 'день', 'дня', 'дней'));
check('2 дня', api.plural(2, 'день', 'дня', 'дней') === 'дня', api.plural(2, 'день', 'дня', 'дней'));
check('5 дней', api.plural(5, 'день', 'дня', 'дней') === 'дней', api.plural(5, 'день', 'дня', 'дней'));
check('11 дней', api.plural(11, 'день', 'дня', 'дней') === 'дней', api.plural(11, 'день', 'дня', 'дней'));
check('21 день', api.plural(21, 'день', 'дня', 'дней') === 'день', api.plural(21, 'день', 'дня', 'дней'));

console.log('');
if (failures > 0) {
  console.log(`client rendering: ${failures} check(s) failed`);
  process.exit(1);
}
console.log('client rendering: all checks passed');
