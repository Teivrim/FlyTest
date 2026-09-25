const canvas = document.getElementById('arena');
const $ = (id) => document.getElementById(id);
const state = { data: null, selected: null, histories: new Map(), lastUiTick: -1 };
let renderer;
let scene;
let camera;
let arenaGroup;
let sensorGroup;
let flyVisuals = new Map();
let trailLines = new Map();
let cameraAzimuth = 0.72;
let cameraElevation = 0.48;
let cameraDistance = 12.5;
let pointerDown = false;
let lastPointer = { x: 0, y: 0 };

function escapeHtml(value) { return String(value).replace(/[&<>'"]/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;' }[char])); }

function init3D() {
  if (!window.THREE) {
    $('status-text').textContent = 'WebGL/Three.js недоступен';
    return false;
  }
  renderer = new THREE.WebGLRenderer({ canvas, antialias: true, powerPreference: 'high-performance' });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFSoftShadowMap;
  scene = new THREE.Scene();
  scene.background = new THREE.Color('#070b13');
  scene.fog = new THREE.Fog('#070b13', 12, 26);
  camera = new THREE.PerspectiveCamera(42, 1, 0.1, 100);
  updateCamera();
  const hemi = new THREE.HemisphereLight('#b9ddff', '#080b14', 1.5);
  scene.add(hemi);
  const key = new THREE.DirectionalLight('#ffffff', 3.2);
  key.position.set(4, 9, 5); key.castShadow = true; scene.add(key);
  const rim = new THREE.PointLight('#54d7e8', 18, 18, 2); rim.position.set(-4, 3, -3); scene.add(rim);
  const violet = new THREE.PointLight('#a98bff', 14, 16, 2); violet.position.set(4, 2, 4); scene.add(violet);
  buildArena();
  resizeCanvas();
  canvas.addEventListener('pointerdown', (event) => { pointerDown = true; lastPointer = { x: event.clientX, y: event.clientY }; canvas.setPointerCapture(event.pointerId); });
  canvas.addEventListener('pointermove', (event) => { if (!pointerDown) return; cameraAzimuth -= (event.clientX - lastPointer.x) * 0.008; cameraElevation = Math.max(0.18, Math.min(1.25, cameraElevation + (event.clientY - lastPointer.y) * 0.006)); lastPointer = { x: event.clientX, y: event.clientY }; updateCamera(); });
  canvas.addEventListener('pointerup', () => { pointerDown = false; });
  canvas.addEventListener('pointercancel', () => { pointerDown = false; });
  canvas.addEventListener('wheel', (event) => { event.preventDefault(); cameraDistance = Math.max(7, Math.min(22, cameraDistance + event.deltaY * 0.012)); updateCamera(); }, { passive: false });
  return true;
}

function material(color, emissive = '#000000', intensity = 0) {
  return new THREE.MeshStandardMaterial({ color, emissive, emissiveIntensity: intensity, roughness: 0.38, metalness: 0.08 });
}

function buildArena() {
  arenaGroup = new THREE.Group(); scene.add(arenaGroup);
  const floor = new THREE.Mesh(new THREE.CircleGeometry(6.3, 96), material('#101827'));
  floor.rotation.x = -Math.PI / 2; floor.receiveShadow = true; arenaGroup.add(floor);
  const grid = new THREE.GridHelper(12, 24, '#25425a', '#142536'); grid.position.y = 0.012; arenaGroup.add(grid);
  const ringMaterial = new THREE.MeshBasicMaterial({ color: '#54d7e8', transparent: true, opacity: 0.65 });
  [2.1, 4.25, 5.8].forEach((radius, index) => { const ring = new THREE.Mesh(new THREE.TorusGeometry(radius, index === 1 ? 0.025 : 0.012, 8, 96), ringMaterial.clone()); ring.rotation.x = Math.PI / 2; ring.position.y = 0.03 + index * 0.006; arenaGroup.add(ring); });
  const center = new THREE.Mesh(new THREE.CylinderGeometry(0.7, 0.85, 0.12, 32), material('#1b3044', '#54d7e8', 0.25)); center.position.y = 0.08; center.castShadow = true; arenaGroup.add(center);
  sensorGroup = new THREE.Group(); arenaGroup.add(sensorGroup);
  const sensorMaterial = new THREE.MeshBasicMaterial({ color: '#ffb86b' });
  [[-4.5, 0.12, -2.4], [4.2, 0.12, 2.1], [0, 0.12, -4.6]].forEach(([x, y, z]) => { const sensor = new THREE.Mesh(new THREE.SphereGeometry(0.09, 12, 8), sensorMaterial); sensor.position.set(x, y, z); sensorGroup.add(sensor); });
  const points = new THREE.Points(new THREE.BufferGeometry(), new THREE.PointsMaterial({ color: '#8ea8ca', size: 0.035, transparent: true, opacity: 0.55 }));
  const starPositions = []; for (let i = 0; i < 180; i++) { const angle = Math.random() * Math.PI * 2; const radius = 7 + Math.random() * 7; starPositions.push(Math.cos(angle) * radius, 1.5 + Math.random() * 6, Math.sin(angle) * radius); }
  points.geometry.setAttribute('position', new THREE.Float32BufferAttribute(starPositions, 3)); arenaGroup.add(points);
}

function updateCamera() {
  if (!camera) return;
  const horizontal = Math.cos(cameraElevation) * cameraDistance;
  camera.position.set(Math.sin(cameraAzimuth) * horizontal, Math.sin(cameraElevation) * cameraDistance, Math.cos(cameraAzimuth) * horizontal);
  camera.lookAt(0, 0.4, 0);
}

function resizeCanvas() {
  if (!renderer) return;
  const rect = canvas.getBoundingClientRect();
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.setSize(Math.max(1, rect.width), Math.max(1, rect.height), false);
  camera.aspect = Math.max(1, rect.width) / Math.max(1, rect.height); camera.updateProjectionMatrix();
}

function labelSprite(text, color) {
  const labelCanvas = document.createElement('canvas'); labelCanvas.width = 256; labelCanvas.height = 64;
  const context = labelCanvas.getContext('2d'); context.clearRect(0, 0, 256, 64); context.font = '600 22px system-ui'; context.fillStyle = color; context.textAlign = 'center'; context.fillText(text, 128, 39);
  const texture = new THREE.CanvasTexture(labelCanvas); texture.needsUpdate = true;
  const sprite = new THREE.Sprite(new THREE.SpriteMaterial({ map: texture, transparent: true, depthTest: false }));
  sprite.scale.set(2.2, 0.55, 1); return sprite;
}

function createFlyVisual(agent) {
  const group = new THREE.Group();
  const color = agent.channel === 'input' ? '#6ee7a8' : (agent.id % 2 ? '#a98bff' : '#54d7e8');
  const body = new THREE.Mesh(new THREE.SphereGeometry(0.22, 20, 12), material(color, color, 0.28)); body.scale.set(0.8, 0.65, 1.25); body.castShadow = true; group.add(body);
  const head = new THREE.Mesh(new THREE.SphereGeometry(0.14, 16, 10), material('#dcecff', '#54d7e8', 0.18)); head.position.set(0, 0.02, -0.24); group.add(head);
  const wingMaterial = new THREE.MeshStandardMaterial({ color: '#bdeaff', transparent: true, opacity: 0.55, side: THREE.DoubleSide, roughness: 0.2 });
  const leftWing = new THREE.Mesh(new THREE.PlaneGeometry(0.48, 0.22), wingMaterial); leftWing.position.set(-0.28, 0.08, 0); leftWing.rotation.x = Math.PI / 2; leftWing.rotation.z = -0.28; group.add(leftWing);
  const rightWing = leftWing.clone(); rightWing.position.x = 0.28; rightWing.rotation.z = 0.28; group.add(rightWing);
  const eyeMaterial = new THREE.MeshBasicMaterial({ color: '#ff5478' });
  const eyeL = new THREE.Mesh(new THREE.SphereGeometry(0.045, 10, 8), eyeMaterial); eyeL.position.set(-0.11, 0.06, -0.33); group.add(eyeL);
  const eyeR = eyeL.clone(); eyeR.position.x = 0.11; group.add(eyeR);
  const selection = new THREE.Mesh(new THREE.TorusGeometry(0.38, 0.018, 8, 32), new THREE.MeshBasicMaterial({ color: '#ffffff', transparent: true, opacity: 0.8 })); selection.rotation.x = Math.PI / 2; selection.position.y = -0.12; group.add(selection);
  const label = labelSprite(agent.name, color); label.position.set(0, 0.6, 0); group.add(label);
  group.userData = { body, leftWing, rightWing, selection, label, color };
  scene.add(group); return group;
}

function removeFlyVisual(id) {
  const visual = flyVisuals.get(id); if (visual) scene.remove(visual); flyVisuals.delete(id);
  const trail = trailLines.get(id); if (trail) { scene.remove(trail); trail.geometry.dispose(); trail.material.dispose(); } trailLines.delete(id);
}

function syncScene() {
  if (!state.data || !scene) return;
  const ids = new Set(state.data.agents.map((agent) => agent.id));
  for (const id of flyVisuals.keys()) if (!ids.has(id)) removeFlyVisual(id);
  for (const id of trailLines.keys()) if (!ids.has(id)) removeFlyVisual(id);
  state.data.agents.forEach((agent, index) => {
    let visual = flyVisuals.get(agent.id); if (!visual) { visual = createFlyVisual(agent); flyVisuals.set(agent.id, visual); }
    visual.position.set(agent.position[0], agent.position[2], agent.position[1]);
    const velocity = agent.velocity; visual.rotation.y = Math.atan2(velocity[0], velocity[2]);
    const flap = Math.sin(performance.now() * 0.018 + index) * 0.45; visual.userData.leftWing.rotation.x = Math.PI / 2 + flap; visual.userData.rightWing.rotation.x = Math.PI / 2 - flap;
    visual.userData.selection.visible = agent.selected;
    visual.userData.body.material.emissiveIntensity = 0.2 + agent.hormone_level * 1.5;
    visual.userData.body.scale.setScalar(0.9 + agent.energy * 0.12);
    let history = state.histories.get(agent.id); if (!history) { history = []; state.histories.set(agent.id, history); } history.push(agent.position); if (history.length > 36) history.shift();
    let trail = trailLines.get(agent.id);
    if (!trail) { const geometry = new THREE.BufferGeometry(); geometry.setAttribute('position', new THREE.BufferAttribute(new Float32Array(36 * 3), 3)); const material = new THREE.LineBasicMaterial({ color: agent.selected ? '#ffffff' : visual.userData.color, transparent: true, opacity: agent.selected ? 0.65 : 0.25 }); trail = new THREE.Line(geometry, material); trail.frustumCulled = false; scene.add(trail); trailLines.set(agent.id, trail); }
    const positions = trail.geometry.attributes.position.array; for (let i = 0; i < 36; i++) { const point = history[Math.max(0, history.length - 36 + i)] || agent.position; positions[i * 3] = point[0]; positions[i * 3 + 1] = point[2] + 0.04; positions[i * 3 + 2] = point[1]; } trail.geometry.attributes.position.needsUpdate = true;
  });
}

async function command(action, payload = {}) { await fetch('/api/command', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action, ...payload }) }); await refresh(); }
async function refresh() {
  try {
    const response = await fetch('/api/state', { cache: 'no-store' }); if (!response.ok) throw new Error('state unavailable'); state.data = await response.json(); state.selected = state.data.selected_fly || (state.data.agents[0] && state.data.agents[0].id);
    $('connection').className = 'pill online'; $('connection').textContent = '● live'; $('status-text').textContent = state.data.running ? 'Runtime работает' : 'Runtime на паузе';
    if (state.data.tick - state.lastUiTick >= 3 || state.lastUiTick < 0) { renderPanels(); state.lastUiTick = state.data.tick; }
  } catch (error) { $('connection').className = 'pill offline'; $('connection').textContent = '● offline'; $('status-text').textContent = 'Runtime недоступен'; }
}
function renderPanels() {
  const data = state.data; if (!data) return;
  $('fps').textContent = `${Math.round(data.metrics.fps)} FPS`; $('tick').textContent = `tick ${data.tick}`; $('sim-time').textContent = `t = ${data.metrics.sim_time.toFixed(2)} s`; $('model-label').textContent = 'model: lightweight-policy'; $('pause').textContent = data.running ? 'Пауза' : 'Продолжить';
  $('fly-list').innerHTML = data.agents.map((agent) => `<div class="fly-card ${agent.id === state.selected ? 'selected' : ''}" data-id="${agent.id}"><div class="fly-avatar">${agent.id}</div><div><strong>${escapeHtml(agent.name)}</strong><small>${agent.channel} · ${agent.neurotransmitter}</small></div><button class="remove" data-remove="${agent.id}" title="Удалить">×</button></div>`).join('');
  document.querySelectorAll('.fly-card').forEach((card) => card.addEventListener('click', (event) => { if (event.target.dataset.remove) return; state.selected = Number(card.dataset.id); command('select', { id: state.selected }); }));
  document.querySelectorAll('[data-remove]').forEach((button) => button.addEventListener('click', (event) => { event.stopPropagation(); command('remove', { id: Number(button.dataset.remove) }); }));
  const agent = data.agents.find((item) => item.id === state.selected) || data.agents[0]; if (!agent) return;
  $('selected-name').textContent = agent.name; $('selected-channel').textContent = agent.channel; $('input-value').textContent = agent.input_level.toFixed(2); $('reaction-value').textContent = agent.reaction_level.toFixed(2); $('nt-value').textContent = agent.neurotransmitter; $('energy-value').textContent = agent.energy.toFixed(2); $('input-meter').style.width = `${agent.input_level * 100}%`; $('reaction-meter').style.width = `${agent.reaction_level * 100}%`; $('hormone-meter').style.width = `${agent.hormone_level * 100}%`; $('energy-meter').style.width = `${agent.energy * 100}%`; $('speed').value = data.speed; $('speed-value').textContent = `${data.speed.toFixed(2)}×`; $('decay').value = data.decay; $('decay-value').textContent = data.decay.toFixed(2); $('hormone-toggle').textContent = agent.hormone_enabled ? 'ON' : 'OFF'; $('signal-bar').style.width = `${agent.input_level * 100}%`; $('reaction-bar').style.width = `${agent.reaction_level * 100}%`; $('hormone-bar').style.width = `${agent.hormone_level * 100}%`;
}
function render3D() { if (!renderer) return; syncScene(); sensorGroup.rotation.y += 0.0015; renderer.render(scene, camera); requestAnimationFrame(render3D); }
$('pause').addEventListener('click', () => command(state.data && state.data.running ? 'pause' : 'resume')); $('reset').addEventListener('click', () => command('reset')); $('add-fly').addEventListener('click', () => command('add')); $('speed').addEventListener('input', (event) => command('speed', { value: Number(event.target.value) })); $('decay').addEventListener('input', (event) => command('decay', { value: Number(event.target.value) })); $('hormone-toggle').addEventListener('click', () => { const agent = state.data && state.data.agents.find((item) => item.id === state.selected); if (agent) command('hormone', { id: agent.id, enabled: !agent.hormone_enabled }); }); $('open-bridge').addEventListener('click', () => window.alert('Для полной 3D-сцены запусти: scripts/run_demo.ps1 -WithBlender'));
init3D(); refresh(); setInterval(refresh, 100); requestAnimationFrame(render3D); window.addEventListener('resize', resizeCanvas);
