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
// One group per act, holding that act's stage, so a stage is built once and then
// only its label and its scenery change.
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

  // The stage's own dimensions come from the server, which is also what the
  // runtime collides against. Keeping a second copy here is how a wall ends up
  // in one place and its collider in another, and the bug is invisible until
  // somebody walks through a wall.
  const stageRadius = act.stage_radius ?? 1.7;
  const stageHeight = act.stage_height ?? 0.17;

  // A raised disc, tinted with the act colour.
  const disc = new THREE.Mesh(
    new THREE.CylinderGeometry(stageRadius, stageRadius * 1.088, stageHeight, 40),
    material('#141d2c', act.color, 0.18)
  );
  disc.position.y = stageHeight * 0.5;
  disc.receiveShadow = true;
  group.add(disc);

  // A low wall of light marking the stage edge.
  const rim = new THREE.Mesh(
    new THREE.TorusGeometry(stageRadius + 0.02, 0.045, 8, 48),
    new THREE.MeshBasicMaterial({ color: act.color, transparent: true, opacity: 0.75 })
  );
  rim.rotation.x = Math.PI / 2;
  rim.position.y = stageHeight + 0.01;
  group.add(rim);

  // Corner posts, placed from the server's collider list so they cannot drift
  // away from the things that stop a character walking through them. The four
  // outermost solids on the ring are the posts.
  const solids = (act.colliders || []).filter((c) => c.radius > 0);
  const ring = Math.max(...solids.map((c) => Math.hypot(c.x, c.z)), 1.5);
  const posts = solids.filter((c) => Math.hypot(c.x, c.z) > ring - 0.3);
  for (let i = 0; i < posts.length; i++) {
    const c = posts[i];
    const post = new THREE.Mesh(new THREE.CylinderGeometry(0.05, 0.06, 1.5, 8), material('#2a3a52'));
    post.position.set(c.x, stageHeight + 0.75, c.z);
    const bulb = new THREE.Mesh(new THREE.SphereGeometry(0.09, 10, 8), new THREE.MeshBasicMaterial({ color: act.color }));
    bulb.position.set(c.x, stageHeight + 1.53, c.z);
    group.add(post); group.add(bulb);
  }

  // Act-specific scenery so the five stages are visually distinct. Each piece
  // is positioned from a collider, so the thing you can see and the thing you
  // bump into are the same object.
  const walls = solids.filter((c) => Math.hypot(c.x, c.z) <= ring - 0.3);
  if (act.id === 'main_stage') {
    for (let i = 0; i < 3; i++) {
      const star = new THREE.Mesh(new THREE.SphereGeometry(0.16, 8, 6), new THREE.MeshBasicMaterial({ color: '#fff2c4' }));
      star.position.set((i - 1) * 0.85, 2.0 + i * 0.18, 0);
      group.add(star);
    }
  } else if (act.id === 'labyrinth') {
    // The colliders are circles standing in for walls, so a wall is drawn as a
    // slab whose long axis follows the ring it sits on.
    for (const c of walls) {
      const wall = new THREE.Mesh(new THREE.BoxGeometry(c.radius * 2, 0.85, 0.1), material('#20344a', act.color, 0.1));
      wall.position.set(c.x, stageHeight + 0.425, c.z);
      wall.rotation.y = -Math.atan2(c.z, c.x);
      group.add(wall);
    }
  } else if (act.id === 'garden') {
    for (const c of walls) {
      const stem = new THREE.Mesh(new THREE.CylinderGeometry(0.02, 0.02, 0.5, 6), material('#3f7a3f'));
      stem.position.set(c.x, stageHeight + 0.25, c.z);
      const bloom = new THREE.Mesh(new THREE.SphereGeometry(0.11, 10, 8), new THREE.MeshBasicMaterial({ color: '#ffd1f0' }));
      bloom.position.set(c.x, stageHeight + 0.53, c.z);
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

/*
 * Limb proportions, in world units.
 *
 * These are the single source of truth. The meshes are built from them and the
 * floor contact is calculated from them, so the two cannot drift apart the way
 * a hard-coded rig height and a hard-coded leg length eventually do.
 *
 * `thigh` and `shin` are also sent to the model, which knows the joint angles
 * but not how long anyone's bones are.
 */
const RIG = {
  leg: {
    thigh: 0.10,
    shin: 0.10,
    // The sole as an ellipsoid hung below the ankle: how far its centre sits,
    // its vertical semi-axis, and its depth semi-axis. The depth is the long
    // one, and it is what makes a point approximation wrong.
    soleC: 0.017,
    soleA: 0.032 * 0.6,
    soleB: 0.032 * 1.35,
    // How far the sole's centre sits ahead of the ankle. A shoe is mounted
    // forward, and that offset swings it down as the ankle tilts, which is
    // what stops a merely tilted leg from sinking into the floor.
    soleF: 0.020,
    // Where the hip pivot sits relative to the rig root. The root is the floor
    // contact point, so the body hangs above it by this much.
    hipHeight: 0.035,
  },
  arm: { upper: 0.075, fore: 0.068, splay: 0.0 },
};

function createFlyVisual(agent) {
  const group = new THREE.Group();
  const index = agent.id - 1;
  const hair = HAIR_COLORS[index % HAIR_COLORS.length];
  const outfit = OUTFITS[index % OUTFITS.length];

  // Rig root sits at the hips so the legs can swing from one pivot.
  const rig = new THREE.Group();
  group.add(rig);

  // The torso is its own group so the spine can bend without dragging the
  // head and arms along with it. Without this there is nowhere to put a
  // posture channel, and a frightened character can only hunch her shoulders
  // in her face.
  const torso = new THREE.Group();
  rig.add(torso);

  // ---- torso: a tapered dress, wider at the hem ----
  const body = new THREE.Mesh(
    new THREE.CylinderGeometry(0.10, 0.19, 0.34, 16),
    material(outfit, outfit, 0.16)
  );
  body.position.y = 0.17;
  body.castShadow = true;
  torso.add(body);

  const collar = new THREE.Mesh(new THREE.TorusGeometry(0.075, 0.02, 6, 14), material('#fff4f8'));
  collar.rotation.x = Math.PI / 2;
  collar.position.y = 0.35;
  torso.add(collar);

  // ---- head ----
  // The neck is a separate pivot so the head can turn and tilt while the
  // spine stays put. One rigid group for both would make every eye movement
  // look like a whole-body flinch.
  const neck = new THREE.Group();
  neck.position.y = 0.40;
  torso.add(neck);
  const head = new THREE.Group();
  neck.add(head);
  const face = new THREE.Mesh(new THREE.SphereGeometry(0.115, 20, 16), material(SKIN, '#ffd7c2', 0.05));
  face.scale.set(1.0, 0.95, 0.92);
  face.castShadow = true;
  head.add(face);

  // Anime eyes: a large dark iris under a highlight, set wide on the face.
  // Each eye is a group so the pupil can slide inside the whites and the
  // lids can drop over the top, which is what makes an expression read.
  const eyeWhiteMaterial = new THREE.MeshBasicMaterial({ color: '#ffffff' });
  const irisMaterial = new THREE.MeshBasicMaterial({ color: '#2b1a3d' });
  const shineMaterial = new THREE.MeshBasicMaterial({ color: '#ffffff' });
  function makeEye(sign) {
    const g = new THREE.Group();
    g.position.set(sign * 0.048, 0.012, 0.098);
    const white = new THREE.Mesh(new THREE.SphereGeometry(0.034, 14, 12), eyeWhiteMaterial);
    white.scale.set(1.15, 0.9, 0.45);
    g.add(white);
    const iris = new THREE.Mesh(new THREE.SphereGeometry(0.021, 14, 12), irisMaterial);
    iris.scale.set(1, 1, 0.45);
    iris.position.z = 0.018;
    g.add(iris);
    const shine = new THREE.Mesh(new THREE.SphereGeometry(0.008, 8, 6), shineMaterial);
    shine.position.set(-0.008 * sign, 0.010, 0.032);
    g.add(shine);
    // Upper lid: a skin-coloured dome that rotates down over the eye.
    const lid = new THREE.Mesh(new THREE.SphereGeometry(0.037, 14, 10, 0, Math.PI * 2, 0, Math.PI * 0.5), material(SKIN));
    lid.rotation.x = -1.35;
    lid.position.z = 0.006;
    g.add(lid);
    return { group: g, white, iris, shine, lid };
  }
  const eyeL = makeEye(-1);
  const eyeR = makeEye(1);
  head.add(eyeL.group);
  head.add(eyeR.group);

  // ---- brows, driven by the brow channel ----
  const browMaterial = material(hair, hair, 0.1);
  const browL = new THREE.Mesh(new THREE.BoxGeometry(0.042, 0.008, 0.012), browMaterial);
  browL.position.set(-0.050, 0.052, 0.104);
  head.add(browL);
  const browR = new THREE.Mesh(new THREE.BoxGeometry(0.042, 0.008, 0.012), browMaterial);
  browR.position.set(0.050, 0.052, 0.104);
  head.add(browR);

  // ---- mouth, driven by the mouth and mouth_open channels ----
  const mouthMesh = new THREE.Mesh(
    new THREE.SphereGeometry(0.020, 12, 8),
    new THREE.MeshBasicMaterial({ color: '#8a2f45' })
  );
  mouthMesh.scale.set(1, 0.4, 0.3);
  mouthMesh.position.set(0, -0.042, 0.100);
  head.add(mouthMesh);
  // A lower lip so an open mouth reads as an opening rather than a blob.
  const lip = new THREE.Mesh(new THREE.SphereGeometry(0.018, 10, 8), material('#e0708a'));
  lip.scale.set(1, 0.5, 0.3);
  lip.position.set(0, -0.052, 0.098);
  head.add(lip);

  // Blush, the cheapest way to make a face read as friendly.
  const blushMaterial = new THREE.MeshBasicMaterial({ color: '#ff9ab5', transparent: true, opacity: 0.35 });
  const blushL = new THREE.Mesh(new THREE.CircleGeometry(0.022, 10), blushMaterial);
  blushL.position.set(-0.072, -0.026, 0.100); head.add(blushL);
  const blushR = blushL.clone(); blushR.position.x = 0.072; head.add(blushR);

  // Tears and sweat: small drops that appear only when the model says so.
  const dropMaterial = new THREE.MeshBasicMaterial({ color: '#bfe9ff', transparent: true, opacity: 0.85 });
  const tearL = new THREE.Mesh(new THREE.SphereGeometry(0.007, 8, 6), dropMaterial);
  tearL.position.set(-0.048, -0.020, 0.112); head.add(tearL);
  const tearR = tearL.clone(); tearR.position.x = 0.048; head.add(tearR);
  const sweatMaterial = new THREE.MeshBasicMaterial({ color: '#dff4ff', transparent: true, opacity: 0.8 });
  const sweatDrop = new THREE.Mesh(new THREE.SphereGeometry(0.006, 8, 6), sweatMaterial);
  sweatDrop.position.set(0.090, 0.040, 0.090);
  head.add(sweatDrop);

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
  // The trailing hair hangs off its own pivot so it can lag behind the head.
  // Hair that is welded to the skull looks painted on the moment the head
  // moves, and nothing else in the rig sells motion as cheaply.
  const hairSwing = new THREE.Group();
  head.add(hairSwing);
  const lockGeometry = new THREE.CapsuleGeometry(0.028, 0.14, 4, 8);
  const lockL = new THREE.Mesh(lockGeometry, hairMaterial);
  lockL.position.set(-0.105, -0.035, 0.02); lockL.rotation.z = 0.2; hairSwing.add(lockL);
  const lockR = new THREE.Mesh(lockGeometry, hairMaterial);
  lockR.position.set(0.105, -0.035, 0.02); lockR.rotation.z = -0.2; hairSwing.add(lockR);
  // Twin tails are the strongest silhouette cue at this scale.
  const tail = new THREE.Mesh(new THREE.CapsuleGeometry(0.032, 0.17, 4, 8), hairMaterial);
  tail.position.set(-0.125, 0.045, -0.06); tail.rotation.set(0.5, 0, 0.55); hairSwing.add(tail);
  const tailR = tail.clone(); tailR.position.x = 0.125; tailR.rotation.z = -0.55; hairSwing.add(tailR);

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
  torso.add(leftWing);
  const rightWing = leftWing.clone();
  rightWing.position.x = 0.12;
  rightWing.rotation.set(0.2, -0.5, -1.15);
  torso.add(rightWing);

  // ---- arms: shoulder, elbow, wrist ----
  // A chain of pivots, not one mesh. A single capsule can swing from the
  // shoulder and that is all it can do, so an arm is either straight or
  // rigidly waved. With a real elbow the forearm can fold in, which is the
  // difference between reaching out and pulling somebody closer.
  //
  // Every segment hangs below its own pivot, so a rotation at a joint moves
  // the whole limb beyond it. Segment lengths live in RIG so the floor
  // calculation and the geometry cannot drift apart.
  const armGeometry = new THREE.CapsuleGeometry(0.020, 0.062, 4, 8);
  function makeArm(sign) {
    const shoulder = new THREE.Group();
    shoulder.position.set(sign * (0.105 + 0.02 * RIG.arm.splay), 0.20, 0.01);
    shoulder.rotation.z = -sign * 0.22;

    const upper = new THREE.Mesh(armGeometry, material(SKIN));
    upper.position.y = -RIG.arm.upper * 0.5;
    shoulder.add(upper);

    const elbow = new THREE.Group();
    elbow.position.y = -RIG.arm.upper;
    shoulder.add(elbow);

    const fore = new THREE.Mesh(
      new THREE.CapsuleGeometry(0.017, 0.052, 4, 8),
      material(SKIN)
    );
    fore.position.y = -RIG.arm.fore * 0.5;
    elbow.add(fore);

    const wrist = new THREE.Group();
    wrist.position.y = -RIG.arm.fore;
    elbow.add(wrist);

    const hand = new THREE.Mesh(new THREE.SphereGeometry(0.024, 8, 6), material(SKIN));
    hand.scale.set(0.85, 1.15, 0.7);
    hand.position.y = -0.022;
    wrist.add(hand);

    return { shoulder, elbow, wrist };
  }
  const armChainL = makeArm(-1);
  const armChainR = makeArm(1);
  const armL = armChainL.shoulder;
  const armR = armChainR.shoulder;
  torso.add(armL);
  torso.add(armR);

  // ---- legs: hip, knee, ankle ----
  // The part being learned, so it gets the most joints.
  const thighGeometry = new THREE.CapsuleGeometry(0.026, RIG.leg.thigh - 0.052, 4, 8);
  const shinGeometry = new THREE.CapsuleGeometry(0.022, RIG.leg.shin - 0.044, 4, 8);
  const shoeMaterial = material(outfit, outfit, 0.2);
  function makeLeg(sign) {
    const hip = new THREE.Group();
    hip.position.set(sign * 0.055, 0.035, 0);

    const thigh = new THREE.Mesh(thighGeometry, material(SKIN));
    thigh.position.y = -RIG.leg.thigh * 0.5;
    hip.add(thigh);

    const knee = new THREE.Group();
    knee.position.y = -RIG.leg.thigh;
    hip.add(knee);

    const shin = new THREE.Mesh(shinGeometry, material(SKIN));
    shin.position.y = -RIG.leg.shin * 0.5;
    knee.add(shin);

    const ankle = new THREE.Group();
    ankle.position.y = -RIG.leg.shin;
    knee.add(ankle);

    const shoe = new THREE.Mesh(new THREE.SphereGeometry(0.032, 10, 8), shoeMaterial);
    shoe.scale.set(0.85, 0.6, 1.35);
    // Sit the shoe so its centre is exactly RIG.leg.soleC below the ankle; the
    // floor calculation then works out where its underside actually lands.
    shoe.position.set(0, -RIG.leg.soleC, RIG.leg.soleF);
    ankle.add(shoe);

    return { hip, knee, ankle };
  }
  const legChainL = makeLeg(-1);
  const legChainR = makeLeg(1);
  const legL = legChainL.hip;
  const legR = legChainR.hip;
  rig.add(legL);
  rig.add(legR);

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
    torso, neck, hairSwing,
    leftWing, rightWing, armL, armR, legL, legR,
    armChainL, armChainR, legChainL, legChainR,
    selection, label, puffGroup,
    eyes: [eyeL, eyeR], brows: [browL, browR], mouth: mouthMesh, lip,
    blushes: [blushL, blushR], tears: [tearL, tearR], sweatDrop,
    // Spring state for the trailing hair, so it can lag instead of snapping.
    hairVelX: 0, hairVelZ: 0,
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
    // These characters are on the ground. How high the body sits is decided
    // further down, from where the feet actually are.
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

    // There is deliberately no vertical bob added to the root.
    //
    // It looks like an obvious thing to add and it is already there: the pelvis
    // rises and falls during a walk because the stance knee flexes, and the
    // root height is derived from the leg, so the oscillation is in the
    // trajectory. Adding a sine on top counts it twice, and the feet leave the
    // floor by most of a bob on every step.

    /* ---- layered pose ----
     * Every layer below only ever *adds* to this accumulator. Nothing writes
     * a joint directly, because a direct write silently cancels the layer
     * underneath it: an arm set absolutely for a wave stops counter-swinging
     * with the walk, so the character appears to freeze mid-stride whenever
     * she gestures. Adding instead means she can walk and wave at once. */
    const pose = newPose();
    const now = performance.now() * 0.001 + index;

    // Layer 1: the walk, which everything else is expressed against.
    //
    // The joint trajectory comes from the model, so the knees and elbows are
    // its answer rather than a guess made here. There is deliberately no extra
    // sine on the hips: that used to be layered on top, and the floor
    // calculation could not see it, so the feet sank by a fifth of the
    // character's height.
    poseLimbs(pose, agent.limbs);
    // A little torso roll, growing with sway: an unstable walk looks unstable.
    pose.spineZ += Math.sin(phase) * 0.05 * sway;
    // The hip drops on the swing leg, which is most of what sells a walk.
    pose.hipRoll = Math.sin(phase) * 0.04 * sway;
    // An idle head turn, so a character standing still is not a mannequin.
    // The two rates are deliberately incommensurate: a single sine would read
    // as a metronome.
    pose.headY += Math.sin(now * 0.29) * 0.13 + Math.sin(now * 0.11) * 0.06;
    // The head goes down as the eyes close, and comes back up as they open.
    // Without this the lids move and nothing else does, and a character
    // falling asleep on her feet looks like a bug rather than like sleep.
    {
      const faceNow = agent.face || {};
      const shut = 1 - Math.max(0, Math.min(1, faceNow.eye_open ?? 1));
      const waking = Math.max(0, 1 - (faceNow.wake_timer ?? 99) / 1.4);
      // While waking she is still raising her chin, so the droop has to let go
      // faster than it arrives.
      const droop = shut * (1 - waking);
      pose.neckX += droop * 0.30;
      pose.headX += droop * 0.16;
      pose.headZ += droop * 0.05;
    }

    // Layer 2: posture, from how she is carrying herself.
    posePosture(pose, agent.posture || {});

    // Layer 3: the gesture, as an offset on top of the walk.
    poseGesture(pose, agent.social || {});

    // Losing her balance makes her sink, and a real person sinks by bending
    // the knees rather than by dropping through the floor. The root height is
    // derived from these angles further down, so folding here shortens the leg
    // and the body comes down with it, staying in contact.
    if (balance < 0.999) {
      const sink = (1 - balance) * 0.9;
      pose.legKnee[0] += sink;
      pose.legKnee[1] += sink;
      pose.legAnkle[0] -= sink * 0.5;
      pose.legAnkle[1] -= sink * 0.5;
    }

    // Layer 4: the expression, mostly in the face but with a little in the
    // spine, because a smile that does not reach the shoulders reads as a
    // mask rather than a face.
    poseExpression(pose, agent.affect || [], agent.face || {}, now);

    // Now, and only now, the accumulator reaches the rig.
    //
    // Each chain is driven joint by joint. The old flat arm fields are folded
    // in here rather than written directly, so a gesture that only knows about
    // the shoulder still works on an arm that has an elbow.
    const chains = [
      { leg: ud.legChainL, arm: ud.armChainL, sign: -1 },
      { leg: ud.legChainR, arm: ud.armChainR, sign: 1 },
    ];
    // Shoulders ride up and in when she is braced, and out when she is open.
    const lift = pose.shoulder * 0.012;
    for (let i = 0; i < 2; i++) {
      const c = chains[i];
      const flatLegX = i === 0 ? pose.legLx : pose.legRx;
      const flatLegZ = i === 0 ? pose.legLz : pose.legRz;
      const flatArmX = i === 0 ? pose.armLx : pose.armRx;
      const flatArmZ = i === 0 ? pose.armLz : pose.armRz;

      const hipAngle = pose.legRoot[i] + flatLegX;
      c.leg.hip.rotation.x = hipAngle;
      c.leg.hip.rotation.z = flatLegZ + pose.legSplay[i] * c.sign;
      c.leg.knee.rotation.x = pose.legKnee[i];
      // The model reports the ankle's *accumulated* angle, which is what the
      // floor calculation uses. The joint here is local to the knee, so the
      // two angles above have to come off again. Applying the accumulated value
      // as a local one tips the foot by the whole hip and knee on top of itself,
      // and the character walks with her toes through the floor.
      c.leg.ankle.rotation.x = pose.legAnkle[i] - hipAngle - pose.legKnee[i];

      c.arm.shoulder.rotation.x = pose.armRoot[i] + flatArmX;
      c.arm.shoulder.rotation.z =
        -c.sign * (0.22 - pose.shoulder * 0.18) + flatArmZ - pose.armSplay[i] * c.sign;
      c.arm.shoulder.position.x = c.sign * (0.105 + 0.03 * pose.armSplay[i]);
      c.arm.shoulder.position.y = 0.20 + lift;
      c.arm.elbow.rotation.x = -pose.armElbow[i];
      c.arm.wrist.rotation.x = pose.armWrist[i] - pose.armRoot[i] - pose.armElbow[i];
    }

    /* ---- standing on the floor ----
     * The root sits at exactly the height that puts the lower foot on the
     * floor, and the height is recomputed from the angles actually applied.
     *
     * A fixed root height cannot work. The foot rises during the swing and
     * falls during the stance, so a constant height buries her to the ankle on
     * the forward step and leaves her hovering on the back one. Nor can the
     * drop be taken from the snapshot alone, because the crouch is a rendering
     * term applied after the model has already answered: folding the knees
     * shortens the leg, and the root has to come down with it.
     *
     * The hip roll has to be in here too. It rotates the whole body about the
     * root, which shortens the leg's vertical reach by exactly cos(roll), so
     * leaving it out slides the feet by a couple of centimetres at every
     * stride. The forward lean is a different matter: it belongs on the torso,
     * because a person leans from the waist and not from the floor.
     *
     * This mirrors TFootDrop in the core, which answers the same question from
     * the model's own angles. A test asserts the two agree, so they cannot
     * drift apart. */
    {
      const roll = pose.hipRoll;
      // cos of a small roll, guarded so a degenerate value cannot divide out.
      const upright = Math.max(0.2, Math.cos(roll));
      const drop =
        Math.min(
          footDrop(pose.legRoot[0], pose.legKnee[0], pose.legAnkle[0]),
          footDrop(pose.legRoot[1], pose.legKnee[1], pose.legAnkle[1])
        ) / upright;
      // The hip offset matters: `drop` is measured from the hip, and the hip
      // sits above the rig root, so the root goes a little lower than the drop
      // alone would suggest. Getting this wrong leaves the feet hovering.
      //
      // `ground` is the surface she is standing on, not always the tent floor.
      // A stage is a raised disc, and a character placed at a flat zero stands
      // ankle-deep inside every stage she walks onto.
      ud.rig.position.y = (agent.ground || 0) - drop - RIG.leg.hipHeight;
    }

    ud.torso.rotation.x = pose.spineX + pose.lean;
    ud.torso.rotation.z = pose.spineZ;
    ud.torso.rotation.y = pose.spineY;
    ud.rig.rotation.z = pose.hipRoll;
    ud.neck.rotation.x = pose.neckX;
    ud.neck.rotation.y = pose.neckY;
    ud.neck.rotation.z = pose.neckZ;

    // The wings stay folded. They are vestigial: these flies cannot fly, and
    // all they get is a small idle flutter that picks up with excitement.
    const flutter = Math.sin(performance.now() * 0.003 + index) * 0.05 * (1 + pose.excite);
    ud.leftWing.rotation.x = 0.2 + flutter;
    ud.rightWing.rotation.x = 0.2 - flutter;

    ud.head.rotation.y = pose.headY;
    ud.head.rotation.z = pose.headZ;
    ud.head.rotation.x = pose.headX;

    // Layer 5: the hair lags. A spring rather than a copy, so it overshoots
    // slightly and settles, which is what hair actually does.
    {
      const dt = 1 / 60;
      const k = 42.0, damp = 7.0;
      const prevX = ud.hairSwing.rotation.x;
      const prevZ = ud.hairSwing.rotation.z;
      ud.hairVelX += (-(prevX - pose.headX) * k - ud.hairVelX * damp) * dt;
      ud.hairVelZ += (-(prevZ - pose.headZ) * k - ud.hairVelZ * damp) * dt;
      ud.hairSwing.rotation.x = prevX + ud.hairVelX * dt + pose.spineX * 0.4;
      ud.hairSwing.rotation.z = prevZ + ud.hairVelZ * dt;
    }

    // ---- face ----
    // Every channel below comes straight from the model. The rig does not
    // decide anything, it only shows what the fly is already feeling.
    const face = agent.face || {};
    const blink = Math.max(0, Math.min(1, face.blink ?? 0));
    const gazeX = Math.max(-1, Math.min(1, face.gaze_x ?? 0));
    const gazeY = Math.max(-1, Math.min(1, face.gaze_y ?? 0));
    const pupil = Math.max(0.5, Math.min(1.6, face.pupil ?? 1));
    const brow = Math.max(-1, Math.min(1, face.brow ?? 0));
    const smile = Math.max(-1, Math.min(1, face.mouth ?? 0));
    const open = Math.max(0, Math.min(1, face.mouth_open ?? 0));
    // The voluntary channel, kept separate from the reflex. The rig only needs
    // the union, which the model has already folded into `blink`; what it uses
    // the voluntary channel for is the squint shape, because an eye narrowed
    // against the light is not the same shape as one being blinked.
    const adapt = Math.max(0, Math.min(1, face.eye_adapt ?? 1));
    const waking = Math.max(0, 1 - (face.wake_timer ?? 99) / 1.4);

    // Lids rotate down over the eye. Closed is a full -90 degrees from the
    // raised position.
    for (const eye of ud.eyes) {
      eye.lid.rotation.x = -1.35 + blink * 1.35;
      // The pupil slides inside the whites, and dilates with fear.
      const range = 0.012 * (1 - blink);
      eye.iris.position.x = gazeX * range;
      eye.iris.position.y = gazeY * range;
      eye.iris.scale.setScalar(pupil);
      eye.shine.position.x = gazeX * range - 0.008 * Math.sign(eye.iris.position.x || 1);
      eye.shine.position.y = gazeY * range + 0.010;
      eye.shine.visible = blink < 0.7 && pupil > 0.7;
      // Squinting narrows the eye without closing it.
      const squint = (1 - adapt) * 0.10 + waking * 0.06;
      eye.white.scale.y = 0.9 * (1 - squint * (1 - blink));
    }
    // Brows: raised when the model says so, drawn together when frowning.
    for (const br of ud.brows) {
      br.position.y = 0.052 + brow * 0.014;
      br.rotation.z = brow * 0.35;
    }
    // Mouth: a smile curves, a frown inverts, an open mouth drops the jaw.
    ud.mouth.scale.set(1 + Math.abs(smile) * 0.25, 0.4 + open * 1.6, 0.3);
    ud.mouth.position.y = -0.042 - open * 0.016;
    ud.lip.position.y = -0.052 - open * 0.026;
    ud.lip.scale.set(1 + Math.abs(smile) * 0.2, 0.5, 0.3);

    // Blush, tears and sweat fade with their own channels.
    for (const b of ud.blushes) {
      b.material.opacity = 0.15 + (face.blush ?? 0) * 0.7;
      b.scale.setScalar(0.8 + (face.blush ?? 0) * 0.5);
    }
    const wet = face.tears ?? 0;
    for (const tear of ud.tears) {
      tear.visible = wet > 0.05;
      tear.scale.setScalar(0.5 + wet);
      // Tears slide down the cheek as they build.
      tear.position.y = -0.020 - wet * 0.016;
    }
    const nervous = face.sweat ?? 0;
    ud.sweatDrop.visible = nervous > 0.05;
    ud.sweatDrop.scale.setScalar(0.5 + nervous);
    ud.sweatDrop.position.y = 0.040 - nervous * 0.012;

    // Head tilts with the mood: a raised brow tips the head, a frown drops it.
    // The head is already driven by the pose layers above, so the expression
    // only leans on the neck here. Writing head.rotation here as well would
    // fight the spine and neck, and the character would end up looking
    // somewhere between the two.

    ud.selection.visible = agent.selected;
    ud.body.material.emissiveIntensity = 0.16 + agent.hormone_level * 1.2;
    // Affect tints the outfit: joy warms it, fear cools it.
    //
    // This reads the same reduced mood the body pose uses, rather than poking
    // at the affect array as if it were an object. It used to do the latter,
    // and since the array is a list of readings, `.joy` was always undefined
    // and the dress never changed colour at all.
    const tint = affectMood(agent.affect);
    const mood = tint.joy;
    const dread = tint.fear;
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
  drawCourses();
}

// The course: the maze, the food, and the breadcrumbs the characters leave.
//
// Rebuilt only when the maze changes, which the seed tells us. The walls are drawn
// from the same segments the runtime collides against, so what is on screen is what
// stops a character; redrawing them every frame would be sixty allocations a second
// for a picture that does not change.

// The courses: a maze on every stage, the food at the end of each, and the
// breadcrumbs the characters leave behind them.
//
// Rebuilt only when a maze actually changes, which the seed tells us. The walls
// are drawn from the very segments the runtime collides against, so what is on
// screen is what stops a character. Redrawing them every frame would be a
// handful of allocations per frame for a picture that does not move, which is the
// same mistake the runtime was making on the other side of the socket.
const courseScenery = new Map();
const courseTrails = new Map();
const courseFood = new Map();

function actById(id) {
  return (state.data.acts || []).find((act) => act.id === id);
}

function drawCourses() {
  const courses = state.data.courses || [];
  const live = new Set();
  for (const course of courses) {
    const act = (state.data.acts || [])[course.act];
    if (!act) continue;
    live.add(course.act);
    const [ox, oz] = act.origin;
    const height = act.stage_height ?? 0.17;
    const key = `${course.act}:${course.seed}`;

    let scenery = courseScenery.get(course.act);
    if (!scenery || scenery.userData.key !== key) {
      if (scenery) {
        while (scenery.children.length) {
          const child = scenery.children.pop();
          child.geometry?.dispose();
          child.material?.dispose();
        }
        scene.remove(scenery);
      }
      scenery = new THREE.Group();
      scenery.userData.key = key;
      for (const wall of course.walls) {
        const slab = new THREE.Mesh(
          new THREE.BoxGeometry(wall.half_len * 2, height * 0.66, wall.half_thick * 2),
          material('#243349', act.color, 0.35)
        );
        slab.position.set(ox + wall.x, height * 0.33, oz + wall.z);
        slab.rotation.y = -wall.angle;
        slab.castShadow = true;
        slab.receiveShadow = true;
        scenery.add(slab);
      }
      scene.add(scenery);
      courseScenery.set(course.act, scenery);

      // The food and a ring under it, so it reads from across the tent.
      let food = courseFood.get(course.act);
      if (food) { scene.remove(food); food.children.forEach((c) => { c.geometry?.dispose(); c.material?.dispose(); }); }
      food = new THREE.Group();
      const crumb = new THREE.Mesh(new THREE.SphereGeometry(0.075, 14, 10), new THREE.MeshBasicMaterial({ color: '#ffd36b' }));
      crumb.position.y = 0.1;
      const ring = new THREE.Mesh(new THREE.TorusGeometry(0.17, 0.016, 6, 26), new THREE.MeshBasicMaterial({ color: '#ffd36b', transparent: true, opacity: 0.85 }));
      ring.rotation.x = Math.PI / 2;
      ring.position.y = 0.02;
      food.add(crumb, ring);
      food.userData.ring = ring;
      scene.add(food);
      courseFood.set(course.act, food);

      for (const [id, line] of courseTrails) {
        if (line.userData.act !== course.act) continue;
        scene.remove(line);
        line.geometry.dispose();
        line.material.dispose();
        courseTrails.delete(id);
      }
    }

    const eaten = course.count > 0 && course.fed >= course.count;
    const food = courseFood.get(course.act);
    if (food) {
      food.visible = !eaten;
      if (!eaten) {
        food.position.set(ox + course.food[0], height + 0.02, oz + course.food[1]);
        food.scale.setScalar(1 + Math.sin(performance.now() * 0.005 + course.act) * 0.16);
      }
    }

    // One line per character, in that character's colour, so the first one
    // through is visibly the one the others are following.
    for (const runner of course.runners) {
      let line = courseTrails.get(runner.id);
      if (!line || line.userData.act !== course.act) {
        if (line) { scene.remove(line); line.geometry.dispose(); line.material.dispose(); }
        const geometry = new THREE.BufferGeometry();
        geometry.setAttribute('position', new THREE.BufferAttribute(new Float32Array(64 * 3), 3));
        const colour = (state.data.agents.find((a) => a.id === runner.id) || {}).color || '#8fd3ff';
        line = new THREE.Line(geometry, new THREE.LineBasicMaterial({ color: colour, transparent: true, opacity: 0.7 }));
        line.frustumCulled = false;
        line.userData.act = course.act;
        scene.add(line);
        courseTrails.set(runner.id, line);
      }
      const points = runner.trail;
      const array = line.geometry.attributes.position.array;
      const count = Math.min(points.length, 64);
      for (let i = 0; i < 64; i++) {
        const point = points[Math.max(0, count - 64 + i)] || points[count - 1] || course.start;
        array[i * 3] = ox + point[0];
        array[i * 3 + 1] = height + 0.03;
        array[i * 3 + 2] = oz + point[1];
      }
      line.geometry.attributes.position.needsUpdate = true;
      line.material.opacity = runner.fed ? 0.95 : 0.4;
    }
  }
  for (const [act, scenery] of courseScenery) {
    if (live.has(act)) continue;
    scene.remove(scenery);
    scenery.children.forEach((c) => { c.geometry?.dispose(); c.material?.dispose(); });
    courseScenery.delete(act);
  }
  checkCoursesDrew(live.size);
}

// Complain once, out loud, if the mazes did not make it into the scene.
//
// This exists because the alternative is the failure mode this project keeps
// producing: a function that is called every frame, throws nothing, and quietly
// draws nothing. The panel says the maze is there, the numbers are right, the
// console is clean, and the stage is bare floor. A missing declaration is not a
// syntax error, so `node --check` passes it, and a screenshot needs a visible
// window that a headless run does not have. So the page checks its own work.
let coursesChecked = false;
function checkCoursesDrew(expected) {
  if (coursesChecked || !expected) return;
  coursesChecked = true;
  const drawn = courseScenery.size;
  if (drawn !== expected) {
    console.error(`course: ${expected} mazes reported by the runtime, ${drawn} drawn`);
  }
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
const INTENT_LABELS = { explore: 'разведка вокруг', approach_odor: 'подойти к источнику запаха', seek_light: 'идти к источнику света', avoid_contact: 'избегать опасного сближения', steer_toward_odor: 'вести к источнику запаха', break_contact: 'прервать контакт и уйти', climb: 'набрать высоту', maintain_course: 'удерживать курс' };
function intentLabel(value) { return INTENT_LABELS[value] || value; }
function renderPanels() {
  const data = state.data; if (!data) return;
  $('fps').textContent = `${Math.round(data.metrics.fps)} FPS`; $('tick').textContent = `tick ${data.tick}`; $('sim-time').textContent = `t = ${data.metrics.sim_time.toFixed(2)} s`; $('model-label').textContent = 'model: lightweight-policy'; $('pause').textContent = data.running ? 'Пауза' : 'Продолжить';
  $('fly-list').innerHTML = data.agents.map((agent) => `<div class="fly-card" data-id="${agent.id}"><div class="fly-avatar">${agent.id}</div><div><strong>${escapeHtml(agent.name)}</strong><small>${agent.channel} · ${agent.neurotransmitter}</small></div><button class="remove" data-remove="${agent.id}" title="удалить персонажа">✕</button></div>`).join('');
  document.querySelectorAll('.fly-card').forEach((card) => card.addEventListener('click', (event) => { if (event.target.dataset.remove) return; state.selected = Number(card.dataset.id); command('select', { id: state.selected }); }));
  document.querySelectorAll('[data-remove]').forEach((button) => button.addEventListener('click', (event) => { event.stopPropagation(); command('remove', { id: Number(button.dataset.remove) }); }));
  const agent = data.agents.find((item) => item.id === state.selected) || data.agents[0]; if (!agent) return;
  $('selected-name').textContent = agent.name; $('selected-channel').textContent = agent.channel; $('input-value').textContent = agent.input_level.toFixed(2); $('reaction-value').textContent = agent.reaction_level.toFixed(2); $('nt-value').textContent = agent.neurotransmitter; $('energy-value').textContent = agent.energy.toFixed(2); $('input-meter').style.width = `${agent.input_level * 100}%`; $('reaction-meter').style.width = `${agent.reaction_level * 100}%`; $('hormone-meter').style.width = `${agent.hormone_level * 100}%`; $('energy-meter').style.width = `${agent.energy * 100}%`; $('speed').value = data.speed; $('speed-value').textContent = `${data.speed.toFixed(2)}×`; $('decay').value = data.decay; $('decay-value').textContent = data.decay.toFixed(2); $('hormone-toggle').textContent = agent.hormone_enabled ? 'ON' : 'OFF'; $('signal-bar').style.width = `${agent.input_level * 100}%`; $('reaction-bar').style.width = `${agent.reaction_level * 100}%`; $('hormone-bar').style.width = `${agent.hormone_level * 100}%`;
  $('drive').textContent = `${intentLabel(agent.drive)} · ${agent.drive}`; $('decision').textContent = intentLabel(agent.decision); $('attention').textContent = agent.attention; $('confidence').textContent = `confidence ${Math.round(agent.confidence * 100)}%`;
  const course = (data.courses || [])[data.training.current_act] || {};
  $('course-round').textContent = `раунд ${course.round || 0}`;
  $('course-goal').textContent = course.count
    ? `дошли до еды: ${course.fed} из ${course.count}`
    : 'дойти до еды в конце лабиринта';
  $('course-count').textContent = `${course.fed || 0} / ${course.count || 0}`;
  $('course-time').value = Math.max(0, Math.min(1, (course.time_left || 0) / (course.round_length || 1)));
  $('course-seed').textContent = `лабиринт seed ${course.seed || 0} · длина хода ${(course.path_length || 0).toFixed(1)}`;
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
  const signed = (v) => `${v >= 0 ? '+' : '−'}${Math.abs(Math.round(v * 100))}`;
  $('brain-learning').textContent = brain.learning || '— → —';
  $('brain-thought').textContent = brain.thought;
  $('brain-level').textContent = `ур. ${brain.level}`;
  // The weight is the headline, because the weight is what "learned" means. The
  // hit rate sits beside it and is labelled as guessing, because a policy
  // reading a flat matrix keeps answering correctly by luck and a reader who
  // sees one percentage next to another cannot tell which is which.
  $('brain-mastery').textContent = pct(brain.mastery);
  $('brain-mastery-meter').style.width = pct(brain.mastery);
  $('brain-left').textContent = brain.trials_left === 0
    ? 'урок выучен'
    : brain.trials_left == null
      ? 'вес не движется'
      : `осталось проб: ~${brain.trials_left}`;
  $('brain-weight').textContent = pct(brain.hit_rate ?? 0);
  $('brain-weight-meter').style.width = pct(brain.hit_rate ?? 0);
  $('brain-gate').textContent = pct(brain.gate);
  $('brain-gate-meter').style.width = pct(brain.gate);
  $('brain-plasticity').textContent = pct(brain.plasticity);
  $('brain-plasticity-meter').style.width = pct(brain.plasticity);
  // The modulator, term by term, so a gate at 12% can be read instead of
  // guessed at. Re-rendered only when the numbers change, because this is the
  // one part of the panel with more rows than the eye wants to track.
  const terms = (brain.gate_terms || [])
    .map((t) => `${t.name} ${signed(t.contribution)}`)
    .join(' · ');
  if (terms !== renderBrain.lastTerms) {
    $('gate-terms').textContent = terms;
    renderBrain.lastTerms = terms;
  }
  $('brain-trials').textContent = String(brain.trials);
  $('brain-correct').textContent = String(brain.correct);
  $('brain-xp').textContent = String(brain.xp);
  $('brain-memory').textContent = String(brain.memory);
  $('brain-mood').textContent = brain.mood;
  $('brain-drive').textContent = brain.drive;
  $('brain-outcome').textContent = brain.last_outcome;
  const rate = brain.learn_rate ?? 1;
  if ($('learnrate') !== document.activeElement) $('learnrate').value = String(rate);
  $('learnrate-value').textContent = `x${Number(rate).toFixed(1)}`;
  drawCurve(brain.curve || []);
  renderLog(data.training.log || [], data.training.log_path);
  // The agent is resolved here rather than assumed: renderBrain only receives
  // the snapshot, and reaching for an outer `agent` would throw and take the
  // whole panel down with it.
  const agent = (data.agents || []).find((item) => item.id === state.selected) || (data.agents || [])[0];
  if (agent) renderGaitAndAffect(agent);
}

// ---- pose layers --------------------------------------------------------
/*
 * A character is doing four things at once: walking, holding herself some way,
 * making a gesture, and wearing an expression. Each of those is a layer that
 * adds to one accumulator, and the accumulator is written to the rig once.
 *
 * The alternative, writing joints directly per system, looks fine until two
 * systems touch the same joint: the second one silently erases the first. That
 * is how a character ends up waving with both arms glued to her sides whenever
 * she happens to be walking.
 */
function newPose() {
  return {
    // Leg chains, indexed 0 = left, 1 = right.
    legRoot: [0, 0], legKnee: [0, 0], legAnkle: [0, 0], legSplay: [0, 0],
    // Arm chains, same order.
    armRoot: [0, 0], armElbow: [0, 0], armWrist: [0, 0], armSplay: [0, 0],
    // Old flat fields the posture and gesture layers still add to.
    legLx: 0, legRx: 0, legLz: 0, legRz: 0,
    armLx: 0, armRx: 0, armLz: 0, armRz: 0,
    // Spine and torso.
    spineX: 0, spineY: 0, spineZ: 0,
    // Hips and lean, on the root.
    hipRoll: 0, lean: 0,
    // Neck, then head on top of it.
    neckX: 0, neckY: 0, neckZ: 0,
    headX: 0, headY: 0, headZ: 0,
    // 0 down .. 1 shrugged.
    shoulder: 0,
    // Carried out to the expression layer.
    excite: 0,
  };
}

/*
 * The model's joint trajectory, applied to all four chains.
 *
 * The knee and elbow are separate joints with separate angles, which is the
 * whole point: a leg that only swings keeps the shin on the floor through the
 * forward step and the character skates. A knee that folds on the swing phase
 * lifts the foot clear, exactly as a real one does.
 *
 * This is the only layer that writes the hip and shoulder angles, because it is
 * the only one the floor calculation knows about. Anything else added here
 * would be a movement the ground could not follow.
 */
function poseLimbs(pose, limbs) {
  if (!limbs) return;
  const legs = [limbs.leg_l, limbs.leg_r];
  const arms = [limbs.arm_l, limbs.arm_r];
  for (let i = 0; i < 2; i++) {
    const leg = legs[i];
    if (leg) {
      pose.legRoot[i] = leg.root;
      pose.legKnee[i] = leg.middle;
      pose.legAnkle[i] = leg.end;
      pose.legSplay[i] = leg.spread;
    }
    const arm = arms[i];
    if (arm) {
      pose.armRoot[i] = arm.root;
      pose.armElbow[i] = arm.middle;
      pose.armWrist[i] = arm.end;
      pose.armSplay[i] = arm.spread;
    }
  }
}

/*
 * Posture: how she carries her body. The spine curls in under fear, straightens
 * under pride, and the shoulders come up when she is braced. This is the layer
 * that makes a mood readable from across the tent, where a face is too small.
 */
function posePosture(pose, posture) {
  const spine = clampf(posture.spine ?? 0, -1, 1);
  const shoulder = clampf(posture.shoulder ?? 0, 0, 1);
  const lean = clampf(posture.lean ?? 0, -1, 1);

  // Curling in is a bend forward, not a shrink.
  pose.spineX += -spine * 0.18;
  // Straightening up also pushes the chest out a little.
  pose.spineX += Math.max(0, spine) * 0.06;
  pose.shoulder += shoulder;
  // Fear tips her back; curiosity tips her forward.
  pose.lean += -lean * 0.10;
  // A hunched character also tucks her head in, which is why the neck reads
  // separately from the spine.
  pose.neckX += -spine * 0.10;
  pose.neckZ += spine * 0.04;
}

// Gesture ids, matching T_GES_* in TFLY.h.
const GESTURE_IDS = {
  'приветствие': 1, 'поклон': 2, 'хлопки': 3, 'указание': 4,
  'утешение': 5, 'плечики': 6, 'прошу обнять': 7, 'покой': 0,
};

/*
 * A gesture is an offset on the arms and spine, not a replacement pose, so it
 * composes with the walk instead of cancelling it.
 */
function poseGesture(pose, social) {
  const id = GESTURE_IDS[social.gesture];
  const s = clampf(social.strength || 0, 0, 1);
  if (id === undefined || id === 0 || s <= 0.01) return;
  // The phase gives repeatable motion inside the gesture, so a wave moves
  // rather than merely appearing.
  const p = social.phase || 0;
  const swing = Math.sin(p * Math.PI * 4);

  switch (id) {
    case 1: // wave: one arm raised above the head and oscillating
      pose.armRx += -2.0 * s;
      pose.armRz += 0.3 * swing * s;
      pose.neckZ += -0.05 * s;
      break;
    case 2: // bow: the whole upper body dips forward
      pose.spineX += 0.55 * s;
      pose.neckX += 0.18 * s;
      pose.armLx += -0.3 * s;
      pose.armRx += -0.3 * s;
      pose.armLz += 0.15 * s;
      pose.armRz += -0.15 * s;
      break;
    case 3: { // clap: hands meet in front, twice per gesture
      const clap = Math.abs(Math.sin(p * Math.PI * 4));
      pose.armLx += -1.2 * s;
      pose.armRx += -1.2 * s;
      // The hands come together and part again, so the gap varies.
      pose.armLz += 0.7 * (1 - clap) * s;
      pose.armRz += -0.7 * (1 - clap) * s;
      pose.spineX += 0.05 * s;
      break;
    }
    case 4: // point: one arm forward, firm
      pose.armRx += -1.4 * s;
      pose.spineY += 0.12 * s;
      pose.neckY += 0.10 * s;
      break;
    case 5: // comfort: hands drawn to the chest
      pose.armLx += -1.0 * s;
      pose.armRx += -1.0 * s;
      pose.armLz += 0.5 * s;
      pose.armRz += -0.5 * s;
      pose.spineX += 0.10 * s;
      pose.neckX += 0.08 * s;
      break;
    case 6: // shrug: arms out, palms up, shoulders up
      pose.shoulder += 0.8 * s;
      pose.armLz += 0.6 * s;
      pose.armRz += -0.6 * s;
      pose.armLx += -0.2 * s;
      pose.armRx += -0.2 * s;
      pose.neckX += 0.10 * s;
      break;
    case 7: // asking to be held: both arms reach forward and up
      pose.armLx += -1.6 * s;
      pose.armRx += -1.6 * s;
      pose.armLz += 0.15 * s;
      pose.armRz += -0.15 * s;
      pose.spineX += -0.08 * s;
      pose.neckX += -0.10 * s;
      break;
    default:
      break;
  }
}

/*
 * The expression layer. Most of the face is driven directly from the model
 * channels further down; what belongs here is the part of a feeling that lives
 * in the body rather than the features. A smile that does not reach the
 * shoulders, or a head that does not tilt into a question, reads as a mask.
 */
function poseExpression(pose, affect, face, now) {
  const mood = affectMood(affect);

  // Excitement straightens her up and gets her moving.
  pose.excite = mood.excitement;
  pose.spineX += mood.excitement * 0.06;
  pose.neckX += -mood.excitement * 0.05;

  // A question goes into the head: she tips it and holds it there, and the
  // tip is asymmetric because a real head tilt is.
  if (mood.confusion > 0.25) {
    const tip = (mood.confusion - 0.25) * 0.5;
    pose.headZ += 0.16 * tip;
    pose.headX += 0.06 * tip;
  }
  // Embarrassment makes her shrink and look away.
  if (mood.shyness > 0.2) {
    const shy = (mood.shyness - 0.2) * 0.6;
    pose.spineX += 0.10 * shy;
    pose.neckX += 0.08 * shy;
    pose.headY += -0.12 * shy * Math.sign(face.gaze_x || 1 || 1);
  }
  // Sadness drops the shoulders and the chin.
  if (mood.sadness > 0.3) {
    const sad = (mood.sadness - 0.3) * 0.5;
    pose.shoulder += sad * 0.5;
    pose.neckX += sad * 0.20;
    pose.armLz += sad * 0.10;
    pose.armRz += -sad * 0.10;
  }
  // Pride pushes the chin up and the shoulders back.
  if (mood.pride > 0.3) {
    const proud = (mood.pride - 0.3) * 0.6;
    pose.neckX += -proud * 0.16;
    pose.lean += -proud * 0.06;
  }
  // Contentment sways rather than doing anything.
  if (mood.contentment > 0.4) {
    pose.hipRoll += Math.sin(now * 0.6) * 0.02 * mood.contentment;
  }
}

/*
 * Reduce the full 26-emotion vector to the handful the body needs, so the
 * expression layer does not have to re-derive it every frame. Emotions with
 * the same name as a field collapse; the rest are the ones named here.
 */
const MOOD_FIELDS = [
  'excitement', 'confusion', 'shyness', 'sadness', 'pride', 'contentment',
  'joy', 'fear', 'anger', 'surprise', 'hope', 'affection', 'boredom',
];
/*
 * Reduce the full 26-emotion vector to the handful the body needs. Entries are
 * read by their stable `key`, never by the Russian label: a label is for people
 * and gets reworded, and a reworded label would silently stop moving the rig.
 * There is no cache here either, because the snapshot hands over a fresh array
 * every frame and a memo keyed on it could never hit.
 */
function affectMood(affect) {
  const mood = {};
  for (const field of MOOD_FIELDS) mood[field] = 0;
  if (!Array.isArray(affect)) return mood;
  for (const reading of affect) {
    if (!reading) continue;
    const key = reading.key;
    const value = reading.value;
    if (key in mood && value > mood[key]) mood[key] = value;
  }
  return mood;
}

function clampf(v, lo, hi) {
  return v < lo ? lo : (v > hi ? hi : v);
}

/*
 * How far below the hip the foot of one leg reaches, given the angles that are
 * about to be applied.
 *
 * This is forward kinematics down the leg chain, and it mirrors TFootDrop in
 * the core. Two implementations of one question is a risk, so the values are
 * cross-checked against the model in the test suite rather than trusted.
 *
 * The sole hangs below the ankle, which is why a leg with a perfectly straight
 * knee still reaches the floor at all.
 */
function footDrop(hipAngle, kneeAngle, ankleAngle) {
  const knee = hipAngle + kneeAngle;
  // The ankle angle is absolute, so it is used as given, exactly as the rig
  // applies it. Anything that adds to it in one place and not the other puts
  // the foot somewhere nobody asked for.
  const theta = ankleAngle;
  // The sole is a long ellipsoid, and its true vertical reach is the support
  // distance of that shape, not the length of a single radius. Treating it as a
  // point overestimates the foot's height by most of a foot once the ankle
  // tilts, and the character visibly hovers with a bent knee.
  const vertical = RIG.leg.soleA * Math.cos(theta);
  const along = RIG.leg.soleB * Math.sin(theta);
  return -(
    RIG.leg.thigh * Math.cos(hipAngle) +
    RIG.leg.shin * Math.cos(knee) +
    (RIG.leg.soleC * Math.cos(theta) + RIG.leg.soleF * Math.sin(theta)) +
    Math.hypot(vertical, along)
  );
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
  $('affect-count').textContent = `${affect.filter((e) => e.value > 0.01).length} активных`;
  $('affect-list').innerHTML = affect.map((e) =>
    `<span class="affect-row">${escapeHtml(e.name)}<i style="width:${pct(e.value)}"></i><b>${Math.round(e.value * 100)}</b></span>`
  ).join('');

  // Face gauges, straight from the model.
  const face = agent.face || {};
  const blinkValue = face.blink ?? 0;
  $('face-blink').textContent = blinkValue > 0.7 ? 'глаза закрыты' : (blinkValue > 0.2 ? 'моргает' : 'глаза открыты');
  $('face-blink-meter').style.width = pct(blinkValue);
  $('face-pupil-meter').style.width = pct(((face.pupil ?? 1) - 0.5) / 1.1);
  // Brow and mouth run -1..1, so shift them into a 0..1 bar.
  $('face-brow-meter').style.width = pct(((face.brow ?? 0) + 1) / 2);
  $('face-mouth-meter').style.width = pct(((face.mouth ?? 0) + 1) / 2);
  $('face-blush-meter').style.width = pct(face.blush);
  $('face-tears-meter').style.width = pct(face.tears);

  // Social.
  const social = agent.social || {};
  $('social-gesture').textContent = social.gesture || 'покой';
  $('social-partner').textContent = social.partner ? `с #${social.partner}` : 'нет пары';
  $('social-bond-meter').style.width = pct(social.bond);
  $('social-last').textContent = social.last_encounter || '—';

  // Posture. The spine and the lean are signed, so their meters grow out from
  // the centre; a plain left-anchored bar would read "curled in" and
  // "stretched up" as the same number.
  const posture = agent.posture || {};
  setSignedMeter($('posture-spine-meter'), posture.spine ?? 0);
  setSignedMeter($('posture-lean-meter'), posture.lean ?? 0);
  $('posture-shoulder-meter').style.width = pct(posture.shoulder ?? 0);

  // Eyes. The voluntary channel and the reflex are reported apart, because
  // "the lids are down" and "she closed them on purpose" are different facts.
  const eyeOpen = clampf(face.eye_open ?? 1, 0, 1);
  const woke = face.wake_timer ?? 99;
  $('face-eyes-state').textContent =
    eyeOpen < 0.2 ? 'спит'
        : woke < 1.4 ? `просыпается ${(1.4 - woke).toFixed(1)} с`
        : (face.blink ?? 0) > 0.7 ? 'моргает'
          : eyeOpen < 0.98 ? 'прищурена' : 'открыты';
  $('sleep-toggle').textContent = eyeOpen < 0.2 ? 'Разбудить' : 'Усыпить';
  const gazeLock = (agent.social || {}).drive ?? 0;
  $('face-gaze-meter').style.width = pct(Math.abs(face.gaze_x ?? 0) * (0.3 + 0.7 * gazeLock));
}

/* Grow a bar out from its centre, left for negative and right for positive. */
function setSignedMeter(el, value) {
  const v = clampf(value, -1, 1);
  const half = Math.abs(v) * 50;
  el.classList.toggle('to-left', v < 0);
  el.classList.toggle('to-right', v >= 0);
  el.style.width = `${half}%`;
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
    // A line gets its author's id in front of it, because most of them are
    // about one character. A line about two already opens with both ids, so
    // printing the author's again reads as "#1 #1 и #2".
    const opens = e.fly ? new RegExp(`^#${e.fly}\\b`).test(e.text) : false;
    const who = e.fly && !opens ? `#${e.fly} ` : '';
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
$('course-button').addEventListener('click', () => command('course'));

// Training controls operate on the selected fly.
$('train-burst').addEventListener('click', () => command('train', { value: 200 }));
$('training-toggle').addEventListener('click', () => command('training', { enabled: !(state.data && state.data.training && state.data.training.enabled) }));
$('epsilon').addEventListener('input', (event) => command('epsilon', { value: Number(event.target.value) }));
$('learnrate').addEventListener('input', (event) => command('learnrate', { value: Number(event.target.value) }));
$('brain-hurt').addEventListener('click', () => { if (state.selected != null) command('hurt', { id: state.selected, value: 0.8 }); });
$('brain-dopamine').addEventListener('click', () => { if (state.selected != null) command('hormone', { id: state.selected, name: 'dopamine', value: 0.8 }); });
$('brain-unlearn').addEventListener('click', () => { if (state.selected != null) command('unlearn', { id: state.selected }); });
$('walk-burst').addEventListener('click', () => command('walk', { value: 200 }));
// Social controls. The meet button needs a partner, so it defaults to the
// nearest other fly.
$('gesture-cycle').addEventListener('click', () => {
  if (state.selected == null) return;
  const order = ['покой', 'приветствие', 'поклон', 'хлопки', 'указание', 'утешение', 'плечики', 'прошу обнять'];
  const agent = state.data && state.data.agents.find((item) => item.id === state.selected);
  const current = agent && agent.social ? order.indexOf(agent.social.gesture) : -1;
  command('gesture', { id: state.selected, value: ((current < 0 ? 0 : current) + 1) % 8 });
});
$('meet-button').addEventListener('click', () => {
  if (state.selected == null || !state.data) return;
  const other = state.data.agents.find((item) => item.id !== state.selected);
  if (!other) return;
  command('meet', { id: state.selected, value: other.id });
});
// Sleep and startle. Together they are the only way to watch the waking
// sequence, which is otherwise reachable only by waiting for the energy to
// run out over minutes of simulated time.
$('sleep-toggle').addEventListener('click', () => {
  if (state.selected == null) return;
  const agent = state.data && state.data.agents.find((item) => item.id === state.selected);
  const asleep = agent ? (agent.face.eye_open ?? 1) < 0.2 : false;
  command('sleep', { id: state.selected, value: asleep ? 0 : 1 });
});
$('startle-button').addEventListener('click', () => {
  if (state.selected == null) return;
  command('startle', { id: state.selected, value: 0.95 });
});
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
