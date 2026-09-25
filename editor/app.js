const canvas = document.getElementById('arena');
const ctx = canvas.getContext('2d');
const state = { data: null, selected: null, histories: new Map(), lastState: 0 };
const $ = (id) => document.getElementById(id);

function resizeCanvas() {
  const ratio = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  canvas.width = Math.max(1, Math.floor(rect.width * ratio));
  canvas.height = Math.max(1, Math.floor(rect.height * ratio));
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
}

function resizeAndDraw() { resizeCanvas(); draw(); }
window.addEventListener('resize', resizeAndDraw);

async function command(action, payload = {}) {
  await fetch('/api/command', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action, ...payload }) });
  await refresh();
}

async function refresh() {
  try {
    const response = await fetch('/api/state', { cache: 'no-store' });
    if (!response.ok) throw new Error('state unavailable');
    state.data = await response.json();
    state.selected = state.data.selected_fly || (state.data.agents[0] && state.data.agents[0].id);
    for (const agent of state.data.agents) {
      if (!state.histories.has(agent.id)) state.histories.set(agent.id, []);
      const history = state.histories.get(agent.id);
      history.push(agent.position);
      if (history.length > 36) history.shift();
    }
    $('connection').className = 'pill online';
    $('connection').textContent = '● live';
    $('status-text').textContent = state.data.running ? 'Runtime работает' : 'Runtime на паузе';
    renderPanels();
  } catch (error) {
    $('connection').className = 'pill offline';
    $('connection').textContent = '● offline';
    $('status-text').textContent = 'Runtime недоступен';
  }
}

function renderPanels() {
  const data = state.data;
  if (!data) return;
  $('fps').textContent = `${Math.round(data.metrics.fps)} FPS`;
  $('tick').textContent = `tick ${data.tick}`;
  $('sim-time').textContent = `t = ${data.metrics.sim_time.toFixed(2)} s`;
  $('model-label').textContent = `model: ${data.model_note.includes('FlyWire') ? 'flywire adapter' : 'lightweight-policy'}`;
  $('pause').textContent = data.running ? 'Пауза' : 'Продолжить';
  $('fly-list').innerHTML = data.agents.map((agent) => `
    <div class="fly-card ${agent.id === state.selected ? 'selected' : ''}" data-id="${agent.id}">
      <div class="fly-avatar">${agent.id}</div>
      <div><strong>${escapeHtml(agent.name)}</strong><small>${agent.channel} · ${agent.neurotransmitter}</small></div>
      <button class="remove" data-remove="${agent.id}" title="Удалить">×</button>
    </div>`).join('');
  document.querySelectorAll('.fly-card').forEach((card) => card.addEventListener('click', (event) => {
    if (event.target.dataset.remove) return;
    state.selected = Number(card.dataset.id);
    command('select', { id: state.selected });
  }));
  document.querySelectorAll('[data-remove]').forEach((button) => button.addEventListener('click', (event) => {
    event.stopPropagation();
    command('remove', { id: Number(button.dataset.remove) });
  }));
  const agent = data.agents.find((item) => item.id === state.selected) || data.agents[0];
  if (!agent) return;
  $('selected-name').textContent = agent.name;
  $('selected-channel').textContent = agent.channel;
  $('input-value').textContent = agent.input_level.toFixed(2);
  $('reaction-value').textContent = agent.reaction_level.toFixed(2);
  $('nt-value').textContent = agent.neurotransmitter;
  $('energy-value').textContent = agent.energy.toFixed(2);
  $('input-meter').style.width = `${agent.input_level * 100}%`;
  $('reaction-meter').style.width = `${agent.reaction_level * 100}%`;
  $('hormone-meter').style.width = `${agent.hormone_level * 100}%`;
  $('energy-meter').style.width = `${agent.energy * 100}%`;
  $('speed').value = data.speed;
  $('speed-value').textContent = `${data.speed.toFixed(2)}×`;
  $('decay').value = data.decay;
  $('decay-value').textContent = data.decay.toFixed(2);
  $('hormone-toggle').textContent = agent.hormone_enabled ? 'ON' : 'OFF';
  $('signal-bar').style.width = `${agent.input_level * 100}%`;
  $('reaction-bar').style.width = `${agent.reaction_level * 100}%`;
  $('hormone-bar').style.width = `${agent.hormone_level * 100}%`;
  $('empty-state').classList.toggle('hidden', Boolean(agent));
}

function escapeHtml(value) { return String(value).replace(/[&<>'"]/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;' }[char])); }

function draw() {
  if (!canvas || !ctx) return;
  const width = canvas.clientWidth;
  const height = canvas.clientHeight;
  ctx.clearRect(0, 0, width, height);
  const background = ctx.createLinearGradient(0, 0, width, height);
  background.addColorStop(0, '#0b1220'); background.addColorStop(1, '#05070d');
  ctx.fillStyle = background; ctx.fillRect(0, 0, width, height);
  const cx = width / 2; const cy = height / 2 + 10;
  const scaleX = width / 13.5; const scaleY = height / 8.5;
  ctx.save();
  ctx.strokeStyle = 'rgba(125, 160, 210, .08)'; ctx.lineWidth = 1;
  for (let x = -6; x <= 6; x += 0.5) { ctx.beginPath(); ctx.moveTo(cx + x * scaleX, 30); ctx.lineTo(cx + x * scaleX, height - 25); ctx.stroke(); }
  for (let y = -4; y <= 4; y += 0.5) { ctx.beginPath(); ctx.moveTo(25, cy + y * scaleY); ctx.lineTo(width - 25, cy + y * scaleY); ctx.stroke(); }
  ctx.strokeStyle = 'rgba(84, 215, 232, .28)'; ctx.lineWidth = 2;
  ctx.beginPath(); ctx.ellipse(cx, cy, 5.2 * scaleX, 3.35 * scaleY, 0, 0, Math.PI * 2); ctx.stroke();
  ctx.strokeStyle = 'rgba(169, 139, 255, .16)'; ctx.setLineDash([4, 8]);
  ctx.beginPath(); ctx.ellipse(cx, cy, 3.2 * scaleX, 2.1 * scaleY, 0, 0, Math.PI * 2); ctx.stroke(); ctx.setLineDash([]);
  const sensor = state.data ? 0.5 + 0.35 * Math.sin(state.data.metrics.sim_time * .7) : .5;
  const sensorGradient = ctx.createRadialGradient(cx, cy, 4, cx, cy, 5.5 * scaleX);
  sensorGradient.addColorStop(0, `rgba(84,215,232,${.05 + sensor * .08})`); sensorGradient.addColorStop(1, 'rgba(84,215,232,0)');
  ctx.fillStyle = sensorGradient; ctx.fillRect(0, 0, width, height);
  if (state.data) {
    for (const agent of state.data.agents) {
      const history = state.histories.get(agent.id) || [];
      ctx.strokeStyle = agent.id === state.selected ? 'rgba(255,255,255,.32)' : 'rgba(84,215,232,.14)';
      ctx.lineWidth = agent.id === state.selected ? 1.5 : 1;
      ctx.beginPath();
      history.forEach((point, index) => { const x = cx + point[0] * scaleX; const y = cy + point[1] * scaleY; if (index === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y); });
      ctx.stroke();
      const x = cx + agent.position[0] * scaleX; const y = cy + agent.position[1] * scaleY;
      const color = agent.id === state.selected ? '#ffffff' : (agent.channel === 'input' ? '#6ee7a8' : '#54d7e8');
      ctx.save(); ctx.translate(x, y); ctx.rotate(Math.atan2(agent.velocity[1], agent.velocity[0]));
      ctx.fillStyle = color; ctx.shadowColor = color; ctx.shadowBlur = agent.id === state.selected ? 18 : 9;
      ctx.beginPath(); ctx.ellipse(0, 0, 10, 6, 0, 0, Math.PI * 2); ctx.fill();
      ctx.fillStyle = 'rgba(255,255,255,.72)';
      ctx.beginPath(); ctx.ellipse(-2, -2, 5, 2.3, -.5, 0, Math.PI * 2); ctx.fill();
      ctx.beginPath(); ctx.ellipse(3, 3, 4, 1.8, .5, 0, Math.PI * 2); ctx.fill();
      ctx.restore();
      ctx.fillStyle = '#dce8fb'; ctx.font = '600 11px system-ui'; ctx.fillText(agent.name, x + 13, y - 9);
      ctx.fillStyle = '#8796b0'; ctx.font = '9px ui-monospace, monospace'; ctx.fillText(`${agent.neurotransmitter} · ${(agent.hormone_level * 100).toFixed(0)}%`, x + 13, y + 4);
    }
  }
  ctx.restore();
}

$('pause').addEventListener('click', () => command(state.data && state.data.running ? 'pause' : 'resume'));
$('reset').addEventListener('click', () => command('reset'));
$('add-fly').addEventListener('click', () => command('add'));
$('speed').addEventListener('input', (event) => command('speed', { value: Number(event.target.value) }));
$('decay').addEventListener('input', (event) => command('decay', { value: Number(event.target.value) }));
$('hormone-toggle').addEventListener('click', () => {
  const agent = state.data && state.data.agents.find((item) => item.id === state.selected);
  if (agent) command('hormone', { id: agent.id, enabled: !agent.hormone_enabled });
});
$('open-bridge').addEventListener('click', () => window.alert('Для полной 3D-сцены запусти: scripts/run_demo.ps1 -WithBlender'));
setInterval(refresh, 100);
resizeCanvas(); refresh(); requestAnimationFrame(function loop() { draw(); requestAnimationFrame(loop); });
