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
let goalMarker;
let actStages = new Map();
let spotlights = null;
let cameraAzimuth = 0.72;
let cameraElevation = 0.48;
let cameraDistance = 17.5;
// The point the camera looks at. WASD walks this around the arena so each act
// can be inspected up close, instead of only ever staring at the middle.
const cameraTarget = { x: 0, y: 1.2, z: 0 };
const heldKeys = new Set();
let lastFrameTime = 0;
let pointerDown = false;
let dragDistance = 0;
let lastPointer = { x: 0, y: 0 };
let raycaster;

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
  raycaster = new THREE.Raycaster();
  updateCamera();
  const hemi = new THREE.HemisphereLight('#b9ddff', '#080b14', 1.5);
  scene.add(hemi);
  const key = new THREE.DirectionalLight('#ffffff', 3.2);
  key.position.set(4, 9, 5); key.castShadow = true; scene.add(key);
  const rim = new THREE.PointLight('#54d7e8', 18, 18, 2); rim.position.set(-4, 3, -3); scene.add(rim);
  const violet = new THREE.PointLight('#a98bff', 14, 16, 2); violet.position.set(4, 2, 4); scene.add(violet);
  buildArena();
  resizeCanvas();
  canvas.addEventListener('pointerdown', (event) => { pointerDown = true; dragDistance = 0; lastPointer = { x: event.clientX, y: event.clientY }; canvas.setPointerCapture(event.pointerId); });
  canvas.addEventListener('pointermove', (event) => { if (!pointerDown) return; const dx = event.clientX - lastPointer.x; const dy = event.clientY - lastPointer.y; dragDistance += Math.abs(dx) + Math.abs(dy); cameraAzimuth -= dx * 0.008; cameraElevation = Math.max(0.18, Math.min(1.25, cameraElevation + dy * 0.006)); lastPointer = { x: event.clientX, y: event.clientY }; updateCamera(); });
  canvas.addEventListener('pointerup', () => { pointerDown = false; });
  canvas.addEventListener('pointercancel', () => { pointerDown = false; });
  canvas.addEventListener('click', (event) => { if (dragDistance > 6 || !raycaster) return; const rect = canvas.getBoundingClientRect(); const pointer = new THREE.Vector2(((event.clientX - rect.left) / rect.width) * 2 - 1, -((event.clientY - rect.top) / rect.height) * 2 + 1); raycaster.setFromCamera(pointer, camera); const hits = raycaster.intersectObjects([...flyVisuals.values()], true); for (const hit of hits) { let object = hit.object; while (object && object.userData.agentId === undefined) object = object.parent; if (object?.userData.agentId) { state.selected = object.userData.agentId; command('select', { id: state.selected }); break; } } });
  canvas.addEventListener('wheel', (event) => { event.preventDefault(); cameraDistance = Math.max(7, Math.min(30, cameraDistance + event.deltaY * 0.016)); updateCamera(); }, { passive: false });
  // Keyboard navigation. Bound on the window so the scene can be walked
  // without first clicking into the canvas.
  window.addEventListener('keydown', onKeyDown);
  window.addEventListener('keyup', onKeyUp);
  window.addEventListener('blur', onWindowBlur);
  canvas.tabIndex = 0;
  return true;
}

function material(color, emissive = '#000000', intensity = 0) {
  return new THREE.MeshStandardMaterial({ color, emissive, emissiveIntensity: intensity, roughness: 0.38, metalness: 0.08 });
}

// =========================================================================
// The circus
//
// Five act stages arranged around a central ring, plus the tent, spotlights
// and banner that make the space read as a circus rather than an arena with
// five pads. Geometry is built once and only animated afterwards, so the
// frame cost stays flat regardless of how many flies are in the ring.
// =========================================================================

const ACT_ORDER = ['main_stage', 'labyrinth', 'garden', 'factory', 'void'];

function buildTent() {
  const tent = new THREE.Group();
  const canvas = document.createElement('canvas');
  canvas.width = 256; canvas.height = 128;
  const ctx = canvas.getContext('2d');
  // Classic alternating circus stripes, drawn once into a texture.
  for (let i = 0; i < 16; i++) {
    ctx.fillStyle = i % 2 === 0 ? '#8c1c3a' : '#f4d9c4';
    ctx.fillRect(i * 16, 0, 16, 128);
  }
  const stripeTexture = new THREE.CanvasTexture(canvas);
  stripeTexture.wrapS = THREE.RepeatWrapping;
  stripeTexture.repeat.set(4, 1);

  const canopy = new THREE.Mesh(
    new THREE.ConeGeometry(11.5, 6.4, 32, 1, true),
    new THREE.MeshStandardMaterial({ map: stripeTexture, side: THREE.BackSide, roughness: 0.85 })
  );
  canopy.position.y = 3.3;
  tent.add(canopy);

  // A crown finial so the tent has a readable top from a low camera.
  const finial = new THREE.Mesh(new THREE.SphereGeometry(0.32, 16, 12), material('#ffd76b', '#ff9f43', 0.6));
  finial.position.y = 6.6;
  tent.add(finial);

  // Proscenium arch at the front, facing the default camera.
  const arch = new THREE.Mesh(new THREE.TorusGeometry(3.2, 0.14, 10, 48, Math.PI), material('#ffd76b', '#ff9f43', 0.35));
  arch.position.set(0, 3.2, 5.2);
  tent.add(arch);

  arenaGroup.add(tent);
  return tent;
}

function buildBanner() {
  const canvas = document.createElement('canvas');
  canvas.width = 1024; canvas.height = 128;
  const ctx = canvas.getContext('2d');
  ctx.fillStyle = '#14060f';
  ctx.fillRect(0, 0, 1024, 128);
  ctx.strokeStyle = '#ffd76b'; ctx.lineWidth = 6;
  ctx.strokeRect(10, 10, 1004, 108);
  ctx.fillStyle = '#ffe6a8';
  ctx.font = '700 62px system-ui';
  ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
  ctx.fillText('THE AMAZING DIGITAL CIRCUS', 512, 66);
  const texture = new THREE.CanvasTexture(canvas);
  const banner = new THREE.Mesh(
    new THREE.PlaneGeometry(9.2, 1.15),
    new THREE.MeshBasicMaterial({ map: texture, transparent: true, side: THREE.DoubleSide })
  );
  banner.position.set(0, 4.35, 4.4);
  arenaGroup.add(banner);
  return banner;
}

function buildSeatRing() {
  const seats = new THREE.Group();
  const rows = 3;
  for (let row = 0; row < rows; row++) {
    const radius = 8.0 + row * 0.85;
    const count = 22 + row * 6;
    for (let i = 0; i < count; i++) {
      const angle = (i / count) * Math.PI * 2;
      // Leave a gap at the front so the camera can see into the ring.
      if (Math.abs(Math.atan2(Math.sin(angle), Math.cos(angle))) < 0.45) continue;
      const chair = new THREE.Mesh(
        new THREE.BoxGeometry(0.3, 0.34, 0.3),
        material(row === 0 ? '#5c2340' : '#3a1730', '#000000', 0)
      );
      chair.position.set(Math.cos(angle) * radius, 0.18 + row * 0.42, Math.sin(angle) * radius);
      chair.lookAt(0, chair.position.y, 0);
      seats.add(chair);
    }
  }
  arenaGroup.add(seats);
  return seats;
}

function buildSpotlights() {
  const group = new THREE.Group();
  const lamps = [];
  for (let i = 0; i < 5; i++) {
    const angle = (i / 5) * Math.PI * 2 + 0.3;
    const color = '#ffe9b0';
    const head = new THREE.Mesh(new THREE.SphereGeometry(0.16, 12, 10), new THREE.MeshBasicMaterial({ color }));
    head.position.set(Math.cos(angle) * 7.2, 5.4, Math.sin(angle) * 7.2);
    // A cone of light, open at the bottom, aimed at the ring.
    const cone = new THREE.Mesh(
      new THREE.ConeGeometry(1.5, 6.2, 20, 1, true),
      new THREE.MeshBasicMaterial({ color, transparent: true, opacity: 0.055, side: THREE.DoubleSide, depthWrite: false })
    );
    cone.position.copy(head.position);
    cone.lookAt(0, 0, 0);
    cone.rotateX(Math.PI / 2);
    lamps.push({ head, cone, phase: i * 1.3 });
    group.add(head); group.add(cone);
  }
  arenaGroup.add(group);
  return group;
}

function buildActStage(act) {
  const group = new THREE.Group();
  const [x, z] = act.origin;
  group.position.set(x, 0, z);

  // A raised disc, tinted with the act colour.
  const disc = new THREE.Mesh(
    new THREE.CylinderGeometry(1.7, 1.85, 0.16, 40),
    material('#141d2c', act.color, 0.18)
  );
  disc.position.y = 0.09;
  disc.receiveShadow = true;
  group.add(disc);

  // A low wall of light marking the stage edge.
  const rim = new THREE.Mesh(
    new THREE.TorusGeometry(1.72, 0.045, 8, 48),
    new THREE.MeshBasicMaterial({ color: act.color, transparent: true, opacity: 0.75 })
  );
  rim.rotation.x = Math.PI / 2;
  rim.position.y = 0.18;
  group.add(rim);

  // Four corner posts with a glowing top, like a ring frame.
  const posts = [];
  for (let i = 0; i < 4; i++) {
    const a = (i / 4) * Math.PI * 2 + Math.PI / 4;
    const post = new THREE.Mesh(new THREE.CylinderGeometry(0.05, 0.06, 1.5, 8), material('#2a3a52'));
    post.position.set(Math.cos(a) * 1.5, 0.8, Math.sin(a) * 1.5);
    const bulb = new THREE.Mesh(new THREE.SphereGeometry(0.09, 10, 8), new THREE.MeshBasicMaterial({ color: act.color }));
    bulb.position.set(Math.cos(a) * 1.5, 1.58, Math.sin(a) * 1.5);
    posts.push({ post, bulb, phase: i * 0.7 });
    group.add(post); group.add(bulb);
  }

  // Act-specific scenery so the five stages are visually distinct.
  if (act.id === 'main_stage') {
    for (let i = 0; i < 3; i++) {
      const star = new THREE.Mesh(new THREE.SphereGeometry(0.16, 8, 6), new THREE.MeshBasicMaterial({ color: '#fff2c4' }));
      star.position.set((i - 1) * 0.85, 2.0 + i * 0.18, 0);
      group.add(star);
    }
  } else if (act.id === 'labyrinth') {
    for (let i = 0; i < 5; i++) {
      const wall = new THREE.Mesh(new THREE.BoxGeometry(1.1, 0.85, 0.1), material('#20344a', act.color, 0.1));
      const a = i * 1.05;
      wall.position.set(Math.cos(a) * 0.95, 0.5, Math.sin(a) * 0.95);
      wall.rotation.y = -a;
      group.add(wall);
    }
  } else if (act.id === 'garden') {
    for (let i = 0; i < 7; i++) {
      const a = i * 0.9;
      const stem = new THREE.Mesh(new THREE.CylinderGeometry(0.02, 0.02, 0.5, 6), material('#3f7a3f'));
      stem.position.set(Math.cos(a) * 1.15, 0.4, Math.sin(a) * 1.15);
      const bloom = new THREE.Mesh(new THREE.SphereGeometry(0.11, 10, 8), new THREE.MeshBasicMaterial({ color: i % 2 ? '#ffd1f0' : '#ffe58a' }));
      bloom.position.set(stem.position.x, 0.68, stem.position.z);
      group.add(stem); group.add(bloom);
    }
  } else if (act.id === 'factory') {
    for (let i = 0; i < 3; i++) {
      const gear = new THREE.Mesh(new THREE.TorusGeometry(0.3 - i * 0.07, 0.07, 6, 14), material('#5a4028', act.color, 0.2));
      gear.position.set((i - 1) * 0.6, 1.0 + i * 0.25, 0);
      gear.userData.spin = i % 2 === 0 ? 1 : -1;
      group.add(gear);
    }
  } else if (act.id === 'void') {
    for (let i = 0; i < 26; i++) {
      const dot = new THREE.Mesh(new THREE.SphereGeometry(0.02, 6, 4), new THREE.MeshBasicMaterial({ color: act.color }));
      const a = i * 2.4;
      const r = 0.4 + (i % 5) * 0.32;
      dot.position.set(Math.cos(a) * r, 0.3 + (i % 7) * 0.22, Math.sin(a) * r);
      group.add(dot);
    }
  }

  const label = labelSprite(act.name, act.color);
  label.position.set(0, 2.35, 0);
  label.scale.set(2.6, 0.65, 1);
  group.add(label);

  group.userData = { act, rim, posts, label };
  arenaGroup.add(group);
  actStages.set(act.id, group);
  return group;
}

function animateActStages() {
  const t = performance.now() * 0.001;
  for (const group of actStages.values()) {
    const { act, rim, posts } = group.userData;
    // The current act pulses faster than the others, so the broadcast target
    // is obvious from the geometry alone.
    const active = state.data && state.data.training && act.id === state.data.acts
      && state.data.training.current_act >= 0
      && (state.data.acts[state.data.training.current_act] || {}).id === act.id;
    const speed = active ? 2.2 : 0.7;
    if (rim) rim.material.opacity = 0.35 + 0.4 * (0.5 + 0.5 * Math.sin(t * speed));
    for (const p of posts) {
      p.bulb.material.color.set(act.color);
      p.bulb.material.color.multiplyScalar(0.55 + 0.45 * (0.5 + 0.5 * Math.sin(t * speed + p.phase)));
    }
    group.rotation.y = Math.sin(t * 0.15) * 0.05;
  }
}

function buildArena() {
  arenaGroup = new THREE.Group(); scene.add(arenaGroup);
  const floor = new THREE.Mesh(new THREE.CircleGeometry(10.6, 96), material('#0d1522'));
  floor.rotation.x = -Math.PI / 2; floor.receiveShadow = true; arenaGroup.add(floor);
  const grid = new THREE.GridHelper(20, 40, '#1d3348', '#121f2e'); grid.position.y = 0.012; arenaGroup.add(grid);

  // Central circus ring, brighter than the surrounding floor.
  const ring = new THREE.Mesh(new THREE.CircleGeometry(4.2, 64), material('#1a1030', '#ff5c7a', 0.12));
  ring.rotation.x = -Math.PI / 2; ring.position.y = 0.02; arenaGroup.add(ring);
  const ringMaterial = new THREE.MeshBasicMaterial({ color: '#ff5c7a', transparent: true, opacity: 0.55 });
  [2.1, 4.25, 4.35].forEach((radius, index) => {
    const r = new THREE.Mesh(new THREE.TorusGeometry(radius, index === 1 ? 0.03 : 0.014, 8, 96), ringMaterial.clone());
    r.rotation.x = Math.PI / 2; r.position.y = 0.035 + index * 0.006; arenaGroup.add(r);
  });
  const center = new THREE.Mesh(new THREE.CylinderGeometry(0.7, 0.85, 0.12, 32), material('#1b3044', '#54d7e8', 0.25));
  center.position.y = 0.08; center.castShadow = true; arenaGroup.add(center);

  sensorGroup = new THREE.Group(); arenaGroup.add(sensorGroup);

  goalMarker = new THREE.Mesh(new THREE.SphereGeometry(0.18, 16, 10), new THREE.MeshBasicMaterial({ color: '#ffb86b' }));
  goalMarker.position.set(0, 0.3, 0); arenaGroup.add(goalMarker);
  const goalRing = new THREE.Mesh(new THREE.TorusGeometry(0.42, 0.025, 8, 32), new THREE.MeshBasicMaterial({ color: '#ffb86b', transparent: true, opacity: 0.8 }));
  goalRing.rotation.x = Math.PI / 2; goalRing.position.y = 0.05; goalMarker.userData.ring = goalRing; arenaGroup.add(goalRing);

  // Circus dressing, built before the act stages so the stages sit on top.
  buildTent();
  buildBanner();
  buildSeatRing();
  spotlights = buildSpotlights();

  // Stage scenery for all five acts, whether or not data has arrived yet.
  const fallback = [
    { id: 'main_stage', name: 'Главная арена', color: '#ff5c7a', origin: [0, 0] },
    { id: 'labyrinth', name: 'Лабиринт', color: '#54d7e8', origin: [6.4, 0] },
    { id: 'garden', name: 'Чародейный сад', color: '#8ce06a', origin: [0, 6] },
    { id: 'factory', name: 'Фабрика чудес', color: '#ffb86b', origin: [-6.4, 0] },
    { id: 'void', name: 'Пустота', color: '#a98bff', origin: [0, -6] },
  ];
  for (const act of fallback) if (!actStages.has(act.id)) buildActStage(act);
  if (state.data) syncActStages();

  // Keep the starfield, but push it out past the tent.
  const points = new THREE.Points(new THREE.BufferGeometry(), new THREE.PointsMaterial({ color: '#8ea8ca', size: 0.035, transparent: true, opacity: 0.5 }));
  const starPositions = [];
  for (let i = 0; i < 220; i++) {
    const angle = Math.random() * Math.PI * 2;
    const radius = 14 + Math.random() * 10;
    starPositions.push(Math.cos(angle) * radius, 1.5 + Math.random() * 9, Math.sin(angle) * radius);
  }
  points.geometry.setAttribute('position', new THREE.Float32BufferAttribute(starPositions, 3));
  arenaGroup.add(points);
}

// Recolour and relabel the stages once the server has told us the real acts.
function syncActStages() {
  if (!state.data || !state.data.acts) return;
  for (const act of state.data.acts) {
    let group = actStages.get(act.id);
    if (!group) group = buildActStage(act);
    group.userData.act = act;
    const label = group.userData.label;
    if (label && label.userData.text !== act.name) {
      label.userData.text = act.name;
      const canvas = document.createElement('canvas');
      canvas.width = 256; canvas.height = 64;
      const ctx = canvas.getContext('2d');
      ctx.clearRect(0, 0, 256, 64);
      ctx.font = '600 22px system-ui'; ctx.fillStyle = act.color;
      ctx.textAlign = 'center'; ctx.fillText(act.name, 128, 39);
      const texture = new THREE.CanvasTexture(canvas);
      texture.needsUpdate = true;
      label.material.map = texture;
      label.material.needsUpdate = true;
    }
  }
}

function updateCamera() {
  if (!camera) return;
  const horizontal = Math.cos(cameraElevation) * cameraDistance;
  camera.position.set(
    cameraTarget.x + Math.sin(cameraAzimuth) * horizontal,
    cameraTarget.y + Math.sin(cameraElevation) * cameraDistance,
    cameraTarget.z + Math.cos(cameraAzimuth) * horizontal
  );
  camera.lookAt(cameraTarget.x, cameraTarget.y, cameraTarget.z);
}

// ---- keyboard navigation --------------------------------------------
const MOVE_KEYS = new Set(['KeyW', 'KeyA', 'KeyS', 'KeyD', 'KeyQ', 'KeyE', 'KeyC', 'ShiftLeft', 'ShiftRight']);

// True when the user is typing, so WASD must not steal the keystroke.
function isTypingTarget(target) {
  if (!target || !target.tagName) return false;
  const tag = target.tagName.toLowerCase();
  return tag === 'input' || tag === 'select' || tag === 'textarea' || target.isContentEditable;
}

function onKeyDown(event) {
  if (isTypingTarget(event.target)) return;
  // Number keys jump the camera to an act, and also send the troupe there,
  // so the keyboard and the act buttons do the same things.
  const digit = event.code.match(/^Digit([1-5])$/);
  if (digit) {
    event.preventDefault();
    focusAct(Number(digit[1]) - 1);
    return;
  }
  // C is a tap, not a hold. Recentre here rather than in the per-frame loop,
  // otherwise a quick tap is released before a frame ever observes it.
  if (event.code === 'KeyC') {
    event.preventDefault();
    cameraTarget.x = 0;
    cameraTarget.y = 1.2;
    cameraTarget.z = 0;
    updateCamera();
    return;
  }
  if (!MOVE_KEYS.has(event.code)) return;
  // Stop the page from scrolling under the canvas while navigating.
  event.preventDefault();
  heldKeys.add(event.code);
}

function onKeyUp(event) { heldKeys.delete(event.code); }

// Losing focus must not leave a key stuck down, or the camera drifts forever.
function onWindowBlur() { heldKeys.clear(); }

function updateCameraFromKeys(now) {
  if (!camera) return;
  const dt = lastFrameTime ? Math.min((now - lastFrameTime) / 1000, 0.1) : 0;
  lastFrameTime = now;
  if (dt === 0 || heldKeys.size === 0) return;

  // Camera-relative movement: forward is where the camera looks, projected onto
  // the floor, so W always walks away from the viewer regardless of azimuth.
  const forward = { x: -Math.sin(cameraAzimuth), z: -Math.cos(cameraAzimuth) };
  const right = { x: Math.cos(cameraAzimuth), z: -Math.sin(cameraAzimuth) };
  const fast = heldKeys.has('ShiftLeft') || heldKeys.has('ShiftRight');
  const speed = (fast ? 13 : 5.5) * dt;

  let fx = 0;
  let fz = 0;
  if (heldKeys.has('KeyW')) { fx += forward.x; fz += forward.z; }
  if (heldKeys.has('KeyS')) { fx -= forward.x; fz -= forward.z; }
  if (heldKeys.has('KeyD')) { fx += right.x; fz += right.z; }
  if (heldKeys.has('KeyA')) { fx -= right.x; fz -= right.z; }

  if (fx !== 0 || fz !== 0) {
    const length = Math.hypot(fx, fz) || 1;
    cameraTarget.x += (fx / length) * speed;
    cameraTarget.z += (fz / length) * speed;
  }

  if (heldKeys.has('KeyE')) cameraTarget.y += speed * 1.4;
  if (heldKeys.has('KeyQ')) cameraTarget.y -= speed * 1.4;

  // Keep the observer inside the tent, or the fog swallows the scene.
  const limit = 9.0;
  cameraTarget.x = Math.max(-limit, Math.min(limit, cameraTarget.x));
  cameraTarget.z = Math.max(-limit, Math.min(limit, cameraTarget.z));
  cameraTarget.y = Math.max(0.2, Math.min(5.5, cameraTarget.y));
  updateCamera();
}

// Point the camera at an act and send the troupe there.
function focusAct(index) {
  const act = state.data && state.data.acts ? state.data.acts[index] : null;
  if (act) {
    cameraTarget.x = act.origin[0];
    cameraTarget.z = act.origin[1];
    cameraTarget.y = 1.6;
    cameraDistance = Math.min(cameraDistance, 11);
    updateCamera();
  }
  command('act', { name: (act ? act.id : ACT_ORDER[index]) });
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

// =========================================================================
// The character
//
// These are not flies any more. They walk, they cannot fly, and they are
// built as stylised anime figures: chibi proportions, large eyes, a dress,
// and a pair of translucent vestigial wings as the only reminder of what
// they used to be. The rig is deliberately simple and driven entirely by the
// gait numbers coming from the C core, so what the fly learned is visible in
// how it walks.
// =========================================================================

const HAIR_COLORS = ['#ff6fa5', '#8ce0ff', '#c9a6ff', '#ffd76b', '#7dffc4', '#ff9f7a'];
const SKIN = '#ffe0cf';
const OUTFITS = ['#ff5c7a', '#54d7e8', '#a98bff', '#8ce06a', '#ffb86b'];

function createFlyVisual(agent) {
  const group = new THREE.Group();
  const index = agent.id - 1;
  const hair = HAIR_COLORS[index % HAIR_COLORS.length];
  const outfit = OUTFITS[index % OUTFITS.length];

  // Rig root sits at the hips so the legs can swing from one pivot.
  const rig = new THREE.Group();
  group.add(rig);

  // ---- torso: a tapered dress, wider at the hem ----
  const body = new THREE.Mesh(
    new THREE.CylinderGeometry(0.10, 0.19, 0.34, 16),
    material(outfit, outfit, 0.16)
  );
  body.position.y = 0.17;
  body.castShadow = true;
  rig.add(body);

  const collar = new THREE.Mesh(new THREE.TorusGeometry(0.075, 0.02, 6, 14), material('#fff4f8'));
  collar.rotation.x = Math.PI / 2;
  collar.position.y = 0.35;
  rig.add(collar);

  // ---- head ----
  const head = new THREE.Group();
  head.position.y = 0.40;
  rig.add(head);
  const face = new THREE.Mesh(new THREE.SphereGeometry(0.115, 20, 16), material(SKIN, '#ffd7c2', 0.05));
  face.scale.set(1.0, 0.95, 0.92);
  face.castShadow = true;
  head.add(face);

  // Anime eyes: a large dark iris under a highlight, set wide on the face.
  const eyeWhite = new THREE.Mesh(new THREE.SphereGeometry(0.034, 12, 10), new THREE.MeshBasicMaterial({ color: '#ffffff' }));
  eyeWhite.scale.set(1.15, 0.9, 0.5);
  const eyeL = eyeWhite.clone(); eyeL.position.set(-0.048, 0.012, 0.100); head.add(eyeL);
  const eyeR = eyeWhite.clone(); eyeR.position.set(0.048, 0.012, 0.100); head.add(eyeR);
  const irisMaterial = new THREE.MeshBasicMaterial({ color: '#2b1a3d' });
  const irisL = new THREE.Mesh(new THREE.SphereGeometry(0.021, 12, 10), irisMaterial);
  irisL.scale.set(1, 1, 0.5); irisL.position.set(-0.048, 0.010, 0.118); head.add(irisL);
  const irisR = irisL.clone(); irisR.position.x = 0.048; head.add(irisR);
  const shineMaterial = new THREE.MeshBasicMaterial({ color: '#ffffff' });
  const shineL = new THREE.Mesh(new THREE.SphereGeometry(0.008, 8, 6), shineMaterial);
  shineL.position.set(-0.056, 0.028, 0.132); head.add(shineL);
  const shineR = shineL.clone(); shineR.position.x = 0.040; head.add(shineR);
  // Blush, the cheapest way to make a face read as friendly.
  const blushMaterial = new THREE.MeshBasicMaterial({ color: '#ff9ab5', transparent: true, opacity: 0.5 });
  const blushL = new THREE.Mesh(new THREE.CircleGeometry(0.022, 10), blushMaterial);
  blushL.position.set(-0.072, -0.026, 0.100); head.add(blushL);
  const blushR = blushL.clone(); blushR.position.x = 0.072; head.add(blushR);

  // ---- hair: cap, fringe, side locks, twin tails ----
  const hairMaterial = material(hair, hair, 0.12);
  const cap = new THREE.Mesh(
    new THREE.SphereGeometry(0.122, 18, 14, 0, Math.PI * 2, 0, Math.PI * 0.58),
    hairMaterial
  );
  cap.position.y = 0.012;
  head.add(cap);
  const fringe = new THREE.Mesh(new THREE.BoxGeometry(0.19, 0.055, 0.03), hairMaterial);
  fringe.position.set(0, 0.062, 0.098);
  fringe.rotation.x = 0.18;
  head.add(fringe);
  const lockGeometry = new THREE.CapsuleGeometry(0.028, 0.14, 4, 8);
  const lockL = new THREE.Mesh(lockGeometry, hairMaterial);
  lockL.position.set(-0.105, -0.035, 0.02); lockL.rotation.z = 0.2; head.add(lockL);
  const lockR = new THREE.Mesh(lockGeometry, hairMaterial);
  lockR.position.set(0.105, -0.035, 0.02); lockR.rotation.z = -0.2; head.add(lockR);
  // Twin tails are the strongest silhouette cue at this scale.
  const tail = new THREE.Mesh(new THREE.CapsuleGeometry(0.032, 0.17, 4, 8), hairMaterial);
  tail.position.set(-0.125, 0.045, -0.06); tail.rotation.set(0.5, 0, 0.55); head.add(tail);
  const tailR = tail.clone(); tailR.position.x = 0.125; tailR.rotation.z = -0.55; head.add(tailR);

  // ---- antennae, the fly inheritance ----
  const antennaMaterial = new THREE.MeshBasicMaterial({ color: '#3a2b46' });
  const stalkGeometry = new THREE.CylinderGeometry(0.004, 0.004, 0.09, 5);
  const stalkL = new THREE.Mesh(stalkGeometry, antennaMaterial);
  stalkL.position.set(-0.05, 0.10, -0.01); stalkL.rotation.set(0.3, 0, 0.5); head.add(stalkL);
  const stalkR = new THREE.Mesh(stalkGeometry, antennaMaterial);
  stalkR.position.set(0.05, 0.10, -0.01); stalkR.rotation.set(0.3, 0, -0.5); head.add(stalkR);
  const beadMaterial = new THREE.MeshBasicMaterial({ color: hair });
  const beadL = new THREE.Mesh(new THREE.SphereGeometry(0.014, 8, 6), beadMaterial);
  beadL.position.set(-0.09, 0.145, -0.01); head.add(beadL);
  const beadR = beadL.clone(); beadR.position.x = 0.09; head.add(beadR);

  // ---- vestigial wings: folded, because these flies cannot fly ----
  const wingMaterial = new THREE.MeshStandardMaterial({
    color: '#dff4ff', transparent: true, opacity: 0.38, side: THREE.DoubleSide,
    roughness: 0.15, depthWrite: false
  });
  const leftWing = new THREE.Mesh(new THREE.PlaneGeometry(0.22, 0.13), wingMaterial);
  leftWing.position.set(-0.12, 0.27, -0.10);
  leftWing.rotation.set(0.2, 0.5, 1.15);
  rig.add(leftWing);
  const rightWing = leftWing.clone();
  rightWing.position.x = 0.12;
  rightWing.rotation.set(0.2, -0.5, -1.15);
  rig.add(rightWing);

  // ---- arms ----
  const armGeometry = new THREE.CapsuleGeometry(0.022, 0.11, 4, 8);
  const armL = new THREE.Mesh(armGeometry, material(SKIN));
  armL.position.set(-0.105, 0.20, 0.01); armL.rotation.z = 0.22; rig.add(armL);
  const armR = new THREE.Mesh(armGeometry, material(SKIN));
  armR.position.set(0.105, 0.20, 0.01); armR.rotation.z = -0.22; rig.add(armR);

  // ---- legs: the part being learned ----
  const legGeometry = new THREE.CapsuleGeometry(0.028, 0.13, 4, 8);
  const shoeMaterial = material(outfit, outfit, 0.2);
  function makeLeg() {
    const pivot = new THREE.Group();
    const upper = new THREE.Mesh(legGeometry, material(SKIN));
    upper.position.y = -0.09;
    pivot.add(upper);
    const shoe = new THREE.Mesh(new THREE.SphereGeometry(0.034, 10, 8), shoeMaterial);
    shoe.scale.set(0.85, 0.6, 1.25);
    shoe.position.set(0, -0.175, 0.018);
    pivot.add(shoe);
    return pivot;
  }
  const legL = makeLeg(); legL.position.set(-0.055, 0.035, 0); rig.add(legL);
  const legR = makeLeg(); legR.position.set(0.055, 0.035, 0); rig.add(legR);

  const selection = new THREE.Mesh(
    new THREE.TorusGeometry(0.26, 0.014, 8, 36),
    new THREE.MeshBasicMaterial({ color: '#ffffff', transparent: true, opacity: 0.85 })
  );
  selection.rotation.x = Math.PI / 2;
  selection.position.y = 0.015;
  group.add(selection);

  const label = labelSprite(agent.name, hair);
  label.position.set(0, 0.78, 0);
  label.scale.set(1.7, 0.42, 1);
  group.add(label);

  const puffGroup = new THREE.Group();
  puffGroup.position.set(0, 0.18, 0.14);
  puffGroup.visible = false;
  for (let i = 0; i < 3; i++) {
    const puffMaterial = new THREE.MeshBasicMaterial({ color: '#b6f36b', transparent: true, opacity: 0, depthWrite: false });
    const puff = new THREE.Mesh(new THREE.SphereGeometry(0.06 + i * 0.018, 10, 8), puffMaterial);
    puff.position.set(i * 0.05, i * 0.03, i * 0.04);
    puffGroup.add(puff);
  }
  group.add(puffGroup);

  group.userData = {
    agentId: agent.id, body, head, face, hair, outfit, rig,
    leftWing, rightWing, armL, armR, legL, legR,
    selection, label, puffGroup,
  };
  scene.add(group);
  return group;
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
    const ud = visual.userData;
    // These characters are on the ground. `height` is the body height above
    // the floor, and the walk cycle adds its bob on top.
    const gait = agent.gait || {};
    const phase = gait.phase || 0;
    const speed = Math.hypot(agent.velocity[0], agent.velocity[1]);
    const walking = speed > 0.05;
    const stride = Math.max(0, Math.min(1, gait.stride ?? 0.5));
    const sway = Math.max(0, Math.min(1, gait.sway ?? 0.3));
    const balance = Math.max(0, Math.min(1, gait.balance ?? 1));

    visual.position.set(agent.position[0], 0, agent.position[1]);
    const heading = Math.atan2(agent.velocity[0], agent.velocity[1]);
    // Ease the turn so the figure does not snap between facings.
    visual.rotation.y += (heading - visual.rotation.y) * 0.25;

    // Vertical bob at twice the step rate, which is what a walk actually is.
    const bob = walking ? Math.abs(Math.sin(phase)) * 0.035 * stride : 0;
    const crouch = (1 - balance) * 0.06;
    ud.rig.position.y = (agent.height || 0) + bob - crouch;
    // A little torso roll, growing with sway: an unstable walk looks unstable.
    ud.rig.rotation.z = Math.sin(phase) * 0.05 * sway;

    // Legs swing in opposition, amplitude set by the learned stride.
    const swing = walking ? Math.sin(phase) * 0.55 * stride : 0;
    ud.legL.rotation.x = swing;
    ud.legR.rotation.x = -swing;
    ud.legL.rotation.z = Math.max(0, swing) * 0.25;
    ud.legR.rotation.z = Math.max(0, -swing) * 0.25;

    // Arms counter-swing, damped when the character is worn out.
    const armSwing = walking ? Math.sin(phase) * 0.35 * stride * (0.5 + 0.5 * agent.energy) : 0.05;
    ud.armL.rotation.x = -armSwing;
    ud.armR.rotation.x = armSwing;

    // The wings stay folded. They are vestigial: these flies cannot fly, and
    // all they get is a small idle flutter.
    const flutter = Math.sin(performance.now() * 0.003 + index) * 0.05;
    ud.leftWing.rotation.x = 0.2 + flutter;
    ud.rightWing.rotation.x = 0.2 - flutter;

    ud.head.rotation.y = Math.sin(performance.now() * 0.0009 + index) * 0.18;

    ud.selection.visible = agent.selected;
    ud.body.material.emissiveIntensity = 0.16 + agent.hormone_level * 1.2;
    // Affect tints the outfit: joy warms it, fear cools it.
    const mood = (agent.affect && agent.affect.joy) || 0;
    const dread = (agent.affect && agent.affect.fear) || 0;
    ud.body.material.emissive.setRGB(
      0.35 + mood * 0.4,
      0.20 + mood * 0.2 - dread * 0.1,
      0.30 - dread * 0.15 + mood * 0.1
    );
    ud.body.scale.setScalar(0.94 + agent.energy * 0.08);
    let history = state.histories.get(agent.id); if (!history) { history = []; state.histories.set(agent.id, history); } history.push(agent.position); if (history.length > 36) history.shift();
    let trail = trailLines.get(agent.id);
    if (!trail) { const geometry = new THREE.BufferGeometry(); geometry.setAttribute('position', new THREE.BufferAttribute(new Float32Array(36 * 3), 3)); const material = new THREE.LineBasicMaterial({ color: agent.selected ? '#ffffff' : visual.userData.color, transparent: true, opacity: agent.selected ? 0.65 : 0.25 }); trail = new THREE.Line(geometry, material); trail.frustumCulled = false; scene.add(trail); trailLines.set(agent.id, trail); }
    const positions = trail.geometry.attributes.position.array; for (let i = 0; i < 36; i++) { const point = history[Math.max(0, history.length - 36 + i)] || agent.position; positions[i * 3] = point[0]; positions[i * 3 + 1] = point[2] + 0.04; positions[i * 3 + 2] = point[1]; } trail.geometry.attributes.position.needsUpdate = true;
    const puffGroup = visual.userData.puffGroup; const puffLevel = agent.puff_level; puffGroup.visible = puffLevel > 0.02; if (puffGroup.visible) { const drift = 1 - puffLevel; puffGroup.scale.setScalar(0.55 + drift * 2.1); puffGroup.position.set(drift * 0.5, 0.1 + drift * 0.35, 0.4 + drift * 0.5); puffGroup.children.forEach((puff, puffIndex) => { puff.material.opacity = puffLevel * (0.55 - puffIndex * 0.12); puff.position.x = puffIndex * 0.09 * (1 + drift) + Math.sin(performance.now() * 0.005 + puffIndex) * 0.05; puff.position.z = puffIndex * 0.07 + drift * 0.3; }); }
  });
  if (goalMarker && state.data.adventure) { const quest = state.data.adventure.id !== 'free_flight'; goalMarker.visible = quest; goalMarker.userData.ring.visible = quest; if (quest) { goalMarker.position.set(state.data.adventure.target[0], 0.32, state.data.adventure.target[1]); goalMarker.userData.ring.position.set(state.data.adventure.target[0], 0.06, state.data.adventure.target[1]); const pulse = 1 + Math.sin(performance.now() * 0.006) * 0.18; goalMarker.scale.setScalar(pulse); goalMarker.userData.ring.scale.setScalar(pulse); } }
}

async function command(action, payload = {}) { await fetch('/api/command', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action, ...payload }) }); await refresh(); }
async function refresh() {
  try {
    const response = await fetch('/api/state', { cache: 'no-store' }); if (!response.ok) throw new Error('state unavailable'); state.data = await response.json(); state.selected = state.data.selected_fly || (state.data.agents[0] && state.data.agents[0].id);
    $('connection').className = 'pill online'; $('connection').textContent = '● live'; $('status-text').textContent = state.data.running ? 'Runtime работает' : 'Runtime на паузе';
    syncActStages();
    if (state.data.tick - state.lastUiTick >= 3 || state.lastUiTick < 0) { renderPanels(); state.lastUiTick = state.data.tick; }
  } catch (error) { $('connection').className = 'pill offline'; $('connection').textContent = '● offline'; $('status-text').textContent = 'Runtime недоступен'; }
}
const INTENT_LABELS = { explore: 'исследование', approach_odor: 'следовать за запахом', seek_light: 'поиск света', avoid_contact: 'избегание контакта', steer_toward_odor: 'поворот к запаху', break_contact: 'разрыв контакта', climb: 'набор высоты', maintain_course: 'удержание курса' };
function intentLabel(value) { return INTENT_LABELS[value] || value; }
function renderPanels() {
  const data = state.data; if (!data) return;
  $('fps').textContent = `${Math.round(data.metrics.fps)} FPS`; $('tick').textContent = `tick ${data.tick}`; $('sim-time').textContent = `t = ${data.metrics.sim_time.toFixed(2)} s`; $('model-label').textContent = 'model: lightweight-policy'; $('pause').textContent = data.running ? 'Пауза' : 'Продолжить';
  $('fly-list').innerHTML = data.agents.map((agent) => `<div class="fly-card ${agent.id === state.selected ? 'selected' : ''}" data-id="${agent.id}"><div class="fly-avatar">${agent.id}</div><div><strong>${escapeHtml(agent.name)}</strong><small>${agent.channel} · ${agent.neurotransmitter}</small></div><button class="remove" data-remove="${agent.id}" title="Удалить">×</button></div>`).join('');
  document.querySelectorAll('.fly-card').forEach((card) => card.addEventListener('click', (event) => { if (event.target.dataset.remove) return; state.selected = Number(card.dataset.id); command('select', { id: state.selected }); }));
  document.querySelectorAll('[data-remove]').forEach((button) => button.addEventListener('click', (event) => { event.stopPropagation(); command('remove', { id: Number(button.dataset.remove) }); }));
  const agent = data.agents.find((item) => item.id === state.selected) || data.agents[0]; if (!agent) return;
  $('selected-name').textContent = agent.name; $('selected-channel').textContent = agent.channel; $('input-value').textContent = agent.input_level.toFixed(2); $('reaction-value').textContent = agent.reaction_level.toFixed(2); $('nt-value').textContent = agent.neurotransmitter; $('energy-value').textContent = agent.energy.toFixed(2); $('input-meter').style.width = `${agent.input_level * 100}%`; $('reaction-meter').style.width = `${agent.reaction_level * 100}%`; $('hormone-meter').style.width = `${agent.hormone_level * 100}%`; $('energy-meter').style.width = `${agent.energy * 100}%`; $('speed').value = data.speed; $('speed-value').textContent = `${data.speed.toFixed(2)}×`; $('decay').value = data.decay; $('decay-value').textContent = data.decay.toFixed(2); $('hormone-toggle').textContent = agent.hormone_enabled ? 'ON' : 'OFF'; $('signal-bar').style.width = `${agent.input_level * 100}%`; $('reaction-bar').style.width = `${agent.reaction_level * 100}%`; $('hormone-bar').style.width = `${agent.hormone_level * 100}%`;
  $('drive').textContent = `${intentLabel(agent.drive)} · ${agent.drive}`; $('decision').textContent = intentLabel(agent.decision); $('attention').textContent = agent.attention; $('confidence').textContent = `confidence ${Math.round(agent.confidence * 100)}%`;
  $('adventure-name').textContent = data.adventure.name; $('adventure-objective').textContent = data.adventure.objective; $('adventure-score').textContent = `score ${data.adventure.score}`; $('adventure-progress').value = data.adventure.progress; $('adventure-select').value = data.adventure.id;
  $('strategy-value').textContent = agent.strategy; $('reward-value').textContent = `${Math.round(agent.reward * 100)}%`; $('novelty-value').textContent = `${Math.round(agent.novelty * 100)}%`; $('puff-value').textContent = `${agent.puff_count} (${data.metrics.total_puff_events})`; $('learning-value').textContent = String(data.metrics.learning_updates);
  renderActs(data);
  renderBrain(data);
}

// ---- act list -------------------------------------------------------
function renderActs(data) {
  if (!data.acts) return;
  const current = data.acts[data.training ? data.training.current_act : 0] || null;
  $('act-list').innerHTML = data.acts.map((act, index) => {
    const pct = Math.round(act.mastery * 100);
    const active = current && current.id === act.id ? 'active' : '';
    const style = `--act-color:${act.color}`;
    return `<button class="act-card ${active}" style="${style}" data-act="${escapeHtml(act.id)}" data-index="${index}" title="${escapeHtml(act.subtitle)}">
      <span class="act-num">${index + 1}</span>
      <span><span class="act-name">${escapeHtml(act.name)}</span><span class="act-lesson">${escapeHtml(act.lesson)} · ${pct}%</span>
      <span class="act-bar"><i style="width:${pct}%"></i></span></span>
    </button>`;
  }).join('');
  document.querySelectorAll('.act-card').forEach((card) => {
    card.addEventListener('click', () => {
      // Solo mode sends only the selected fly; otherwise the whole troupe
      // travels together, which is what a circus act actually looks like.
      const payload = { name: card.dataset.act };
      if ($('solo-mode').checked && state.selected != null) payload.id = state.selected;
      command('act', payload);
    });
  });
}

// ---- brain / training panel ----------------------------------------
function renderBrain(data) {
  const brain = data.brain;
  const training = data.training || {};
  $('training-toggle').textContent = training.enabled ? 'Авто: ВКЛ' : 'Авто: ВЫКЛ';
  if ($('epsilon') !== document.activeElement) $('epsilon').value = String(training.epsilon ?? 0.25);
  if (!brain) {
    $('brain-thought').textContent = 'муха не выбрана';
    return;
  }
  const pct = (v) => `${Math.round(Math.max(0, Math.min(1, v)) * 100)}%`;
  $('brain-thought').textContent = brain.thought;
  $('brain-level').textContent = `ур. ${brain.level}`;
  $('brain-mastery').textContent = pct(brain.mastery);
  $('brain-mastery-meter').style.width = pct(brain.mastery);
  $('brain-weight').textContent = pct(brain.weight);
  $('brain-weight-meter').style.width = pct(brain.weight);
  $('brain-gate').textContent = pct(brain.gate);
  $('brain-gate-meter').style.width = pct(brain.gate);
  $('brain-plasticity').textContent = pct(brain.plasticity);
  $('brain-plasticity-meter').style.width = pct(brain.plasticity);
  $('brain-trials').textContent = String(brain.trials);
  $('brain-correct').textContent = String(brain.correct);
  $('brain-xp').textContent = String(brain.xp);
  $('brain-memory').textContent = String(brain.memory);
  $('brain-mood').textContent = brain.mood;
  $('brain-drive').textContent = brain.drive;
  $('brain-outcome').textContent = brain.last_outcome;
  drawCurve(brain.curve || []);
  renderLog(data.training.log || [], data.training.log_path);
  renderGaitAndAffect(agent);
}

// ---- gait and affect -------------------------------------------------
function renderGaitAndAffect(agent) {
  const gait = agent.gait || {};
  const pct = (v) => `${Math.round(Math.max(0, Math.min(1, v || 0)) * 100)}%`;
  $('gait-preset').textContent = gait.preset || '—';
  $('gait-steps').textContent = `${gait.steps || 0} шагов`;
  $('gait-balance').textContent = `устойчивость ${pct(gait.balance)}`;
  $('gait-stride').style.width = pct(gait.stride);
  $('gait-cadence').style.width = pct(gait.cadence);
  $('gait-sway').style.width = pct(gait.sway);

  const weights = gait.weights || [];
  const best = weights.reduce((acc, w) => Math.max(acc, w[1]), 0);
  $('gait-weights').innerHTML = weights.map(([name, value]) => {
    const cls = value >= best - 1e-6 && value > 0.01 ? 'best' : '';
    return `<span class="gait-weight ${cls}">${escapeHtml(name)}<b></b><i style="width:${pct(value)}"></i></span>`;
  }).join('');

  // Every emotion, not just the strongest, so a character reads as a mix.
  const affect = agent.affect || [];
  $('affect-count').textContent = `${affect.filter(([, v]) => v > 0.01).length} активных`;
  $('affect-list').innerHTML = affect.map(([name, value]) =>
    `<span class="affect-row">${escapeHtml(name)}<i style="width:${pct(value)}"></i><b>${Math.round(value * 100)}</b></span>`
  ).join('');
}

// ---- learning curve sparkline ----------------------------------------
function drawCurve(points) {
  const canvas = $('curve');
  if (!canvas || !canvas.getContext) return;
  const ctx = canvas.getContext('2d');
  // Match the backing store to the CSS box so the line is not stretched.
  const rect = canvas.getBoundingClientRect();
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const w = Math.max(1, Math.round((rect.width || 260) * dpr));
  const h = Math.max(1, Math.round((rect.height || 46) * dpr));
  if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
  ctx.clearRect(0, 0, w, h);

  $('curve-label').textContent = points.length ? `${points.length} проб` : 'нет данных';
  if (points.length < 2) return;

  const pad = 3 * dpr;
  const innerW = w - pad * 2;
  const innerH = h - pad * 2;
  const series = [
    { key: 'mastery', color: '#ff5c7a' },
    { key: 'accuracy', color: '#6ee7a8' },
    { key: 'gate', color: '#a98bff' },
  ];
  for (const s of series) {
    ctx.beginPath();
    for (let i = 0; i < points.length; i++) {
      const x = pad + (i / (points.length - 1)) * innerW;
      // Mastery is the only value that can go negative (a weight below zero),
      // so it is drawn against a centred axis rather than a 0..1 floor.
      const v = Math.max(-1, Math.min(1, points[i][s.key] ?? 0));
      const y = v >= 0 ? pad + innerH * (1 - v) : pad + innerH;
      if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
    }
    ctx.strokeStyle = s.color;
    ctx.lineWidth = 1.4 * dpr;
    ctx.globalAlpha = 0.9;
    ctx.stroke();
  }
  // Zero line, so a negative weight is visibly below the axis.
  ctx.globalAlpha = 0.25;
  ctx.strokeStyle = '#8ea8ca';
  ctx.lineWidth = 1 * dpr;
  ctx.beginPath();
  ctx.moveTo(pad, pad + innerH);
  ctx.lineTo(pad + innerW, pad + innerH);
  ctx.stroke();
  ctx.globalAlpha = 1;
}

// ---- event log --------------------------------------------------------
function renderLog(entries, path) {
  $('log-path').textContent = path ? 'jsonl' : 'память';
  const list = $('log-list');
  const html = entries.slice().reverse().slice(0, 40).map((e) => {
    const stamp = `${Math.floor(e.t / 60)}:${String(Math.floor(e.t % 60)).padStart(2, '0')}`;
    const who = e.fly ? `#${e.fly} ` : '';
    return `<li class="${escapeHtml(e.kind)}"><time>${stamp}</time><span>${who}${escapeHtml(e.text)}</span></li>`;
  }).join('');
  // Only rewrite when the content changed, so the list does not flicker.
  if (list.dataset.sig !== html) { list.innerHTML = html; list.dataset.sig = html; }
}
function render3D() {
  if (!renderer) return;
  const now = performance.now();
  updateCameraFromKeys(now);
  syncScene();
  if (sensorGroup) sensorGroup.rotation.y += 0.0015;
  animateActStages();
  if (spotlights) {
    const t = now * 0.001;
    for (const lamp of spotlights.children) {
      if (lamp.geometry && lamp.geometry.type === 'ConeGeometry') {
        lamp.material.opacity = 0.035 + 0.03 * (0.5 + 0.5 * Math.sin(t * 1.7));
      }
    }
  }
  renderer.render(scene, camera);
  requestAnimationFrame(render3D);
}
$('pause').addEventListener('click', () => command(state.data && state.data.running ? 'pause' : 'resume')); $('reset').addEventListener('click', () => command('reset')); $('add-fly').addEventListener('click', () => command('add')); $('speed').addEventListener('input', (event) => command('speed', { value: Number(event.target.value) })); $('decay').addEventListener('input', (event) => command('decay', { value: Number(event.target.value) })); $('hormone-toggle').addEventListener('click', () => { const agent = state.data && state.data.agents.find((item) => item.id === state.selected); if (agent) command('hormone', { id: agent.id, enabled: !agent.hormone_enabled }); }); $('open-bridge').addEventListener('click', () => window.alert('Для полной 3D-сцены запусти: scripts/run_demo.ps1 -WithBlender'));
$('adventure-select').addEventListener('change', (event) => command('adventure', { name: event.target.value }));

// Training controls operate on the selected fly.
$('train-burst').addEventListener('click', () => command('train', { value: 200 }));
$('training-toggle').addEventListener('click', () => command('training', { enabled: !(state.data && state.data.training && state.data.training.enabled) }));
$('epsilon').addEventListener('input', (event) => command('epsilon', { value: Number(event.target.value) }));
$('brain-hurt').addEventListener('click', () => { if (state.selected != null) command('hurt', { id: state.selected, value: 0.8 }); });
$('brain-dopamine').addEventListener('click', () => { if (state.selected != null) command('hormone', { id: state.selected, name: 'dopamine', value: 0.8 }); });
$('brain-unlearn').addEventListener('click', () => { if (state.selected != null) command('unlearn', { id: state.selected }); });
$('walk-burst').addEventListener('click', () => command('walk', { value: 200 }));
$('gait-cycle').addEventListener('click', () => {
  if (state.selected == null) return;
  const agent = state.data && state.data.agents.find((item) => item.id === state.selected);
  const current = agent && agent.gait ? (agent.gait.weights || []).indexOf(
    (agent.gait.weights || []).find((w) => w[0] === agent.gait.preset)
  ) : 0;
  command('gait', { id: state.selected, value: ((current < 0 ? 0 : current) + 1) % 4 });
});
$('log-toggle').addEventListener('click', () => {
  const on = state.data && state.data.training && state.data.training.log_path;
  // The server appends, so re-clicking just reopens the same sink.
  command('logfile', { name: on ? 'off' : 'runtime-output/training-log.jsonl' });
});
$('reward-button').addEventListener('click', () => { const agent = state.data && state.data.agents.find((item) => item.id === state.selected); if (agent) command('reward', { id: agent.id }); });
$('puff-button').addEventListener('click', () => { const agent = state.data && state.data.agents.find((item) => item.id === state.selected); if (agent) command('puff', { id: agent.id }); });
init3D(); refresh(); setInterval(refresh, 100); requestAnimationFrame(render3D); window.addEventListener('resize', resizeCanvas);
