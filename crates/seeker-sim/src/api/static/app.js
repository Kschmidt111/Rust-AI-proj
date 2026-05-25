/**
 * SeekerSim sim viewer (Phase 6C).
 * Fetches frame snapshots from POST /v1/sim/run and animates on canvas.
 * All guidance math runs on the server — this file only renders.
 */

const CANVAS_PADDING = 28;
const SEEKER_RADIUS = 6;
const TARGET_RADIUS = 7;

/** @type {ReturnType<typeof setTimeout> | null} */
let replayAnimationTimer = null;

/** @type {{ frames: object[], bounds: object, intervalMs: number } | null} */
let replayState = null;

function cssVar(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

async function checkHealth() {
  const el = document.getElementById("health-status");
  if (!el) return;

  try {
    const res = await fetch("/health");
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = await res.json();
    el.textContent = `API ${data.status} · ${data.service}`;
    el.classList.add("ok");
  } catch (err) {
    el.textContent = `API unreachable (${err.message})`;
    el.classList.add("err");
  }
}

/**
 * @param {HTMLFormElement} form
 */
function readFormPayload(form) {
  const fd = new FormData(form);
  return {
    target_x: Number(fd.get("target_x")),
    target_y: Number(fd.get("target_y")),
    target_vx: Number(fd.get("target_vx")),
    target_vy: Number(fd.get("target_vy")),
    law: String(fd.get("law")),
  };
}

/**
 * @param {object} payload
 */
async function fetchSimRun(payload) {
  const res = await fetch("/v1/sim/run", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });

  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || `HTTP ${res.status}`);
  }

  return res.json();
}

/**
 * @param {Array<{ interceptor: { x: number, y: number }, target: { x: number, y: number } }>} frames
 */
function computeBounds(frames) {
  let minX = Infinity;
  let maxX = -Infinity;
  let minY = Infinity;
  let maxY = -Infinity;

  for (const frame of frames) {
    for (const pt of [frame.interceptor, frame.target]) {
      minX = Math.min(minX, pt.x);
      maxX = Math.max(maxX, pt.x);
      minY = Math.min(minY, pt.y);
      maxY = Math.max(maxY, pt.y);
    }
  }

  if (!Number.isFinite(minX)) {
    return { minX: -100, maxX: 100, minY: -100, maxY: 100 };
  }

  const padX = Math.max(20, (maxX - minX) * 0.08);
  const padY = Math.max(20, (maxY - minY) * 0.08);

  return {
    minX: minX - padX,
    maxX: maxX + padX,
    minY: minY - padY,
    maxY: maxY + padY,
  };
}

/**
 * Map sim coords (y up) to canvas pixels (y down).
 * @param {number} x
 * @param {number} y
 * @param {{ minX: number, maxX: number, minY: number, maxY: number }} bounds
 * @param {number} w
 * @param {number} h
 */
function simToCanvas(x, y, bounds, w, h) {
  const rangeX = bounds.maxX - bounds.minX || 1;
  const rangeY = bounds.maxY - bounds.minY || 1;
  const innerW = w - CANVAS_PADDING * 2;
  const innerH = h - CANVAS_PADDING * 2;
  const scale = Math.min(innerW / rangeX, innerH / rangeY);

  const cx = CANVAS_PADDING + (x - bounds.minX) * scale;
  const cy = h - CANVAS_PADDING - (y - bounds.minY) * scale;
  return { x: cx, y: cy };
}

/**
 * @param {CanvasRenderingContext2D} ctx
 * @param {number} w
 * @param {number} h
 */
function clearCanvas(ctx, w, h) {
  ctx.fillStyle = cssVar("--canvas-bg");
  ctx.fillRect(0, 0, w, h);

  ctx.strokeStyle = cssVar("--border");
  ctx.lineWidth = 1;
  ctx.strokeRect(0.5, 0.5, w - 1, h - 1);
}

/**
 * @param {CanvasRenderingContext2D} ctx
 * @param {Array<{ interceptor: { x: number, y: number }, target: { x: number, y: number } }>} frames
 * @param {number} frameIndex
 * @param {{ minX: number, maxX: number, minY: number, maxY: number }} bounds
 * @param {number} w
 * @param {number} h
 */
function drawSimulationFrame(ctx, frames, frameIndex, bounds, w, h) {
  clearCanvas(ctx, w, h);

  const seekerColor = cssVar("--seeker");
  const targetColor = cssVar("--target");
  const trailColor = cssVar("--muted");

  const slice = frames.slice(0, frameIndex + 1);

  if (slice.length > 1) {
    ctx.strokeStyle = trailColor;
    ctx.lineWidth = 1.5;
    ctx.globalAlpha = 0.45;

    ctx.beginPath();
    const firstS = simToCanvas(slice[0].interceptor.x, slice[0].interceptor.y, bounds, w, h);
    ctx.moveTo(firstS.x, firstS.y);
    for (let i = 1; i < slice.length; i++) {
      const p = simToCanvas(slice[i].interceptor.x, slice[i].interceptor.y, bounds, w, h);
      ctx.lineTo(p.x, p.y);
    }
    ctx.stroke();

    ctx.beginPath();
    const firstT = simToCanvas(slice[0].target.x, slice[0].target.y, bounds, w, h);
    ctx.moveTo(firstT.x, firstT.y);
    for (let i = 1; i < slice.length; i++) {
      const p = simToCanvas(slice[i].target.x, slice[i].target.y, bounds, w, h);
      ctx.lineTo(p.x, p.y);
    }
    ctx.stroke();

    ctx.globalAlpha = 1;
  }

  const frame = frames[frameIndex];
  const seeker = simToCanvas(frame.interceptor.x, frame.interceptor.y, bounds, w, h);
  const target = simToCanvas(frame.target.x, frame.target.y, bounds, w, h);

  ctx.fillStyle = seekerColor;
  ctx.beginPath();
  ctx.arc(seeker.x, seeker.y, SEEKER_RADIUS, 0, Math.PI * 2);
  ctx.fill();

  ctx.fillStyle = targetColor;
  ctx.beginPath();
  ctx.arc(target.x, target.y, TARGET_RADIUS, 0, Math.PI * 2);
  ctx.fill();

  ctx.fillStyle = cssVar("--text");
  ctx.font = "12px Segoe UI, system-ui, sans-serif";
  ctx.textAlign = "left";
  ctx.fillText(frame.hud || `t=${frame.time_s.toFixed(2)}s  miss=${frame.miss_distance.toFixed(1)}`, 8, 18);
}

/**
 * @param {string} text
 */
function parseCsv(text) {
  const lines = text.trim().split(/\r?\n/).filter((l) => l.length > 0);
  if (lines.length < 2) return [];

  const header = lines[0].split(",").map((h) => h.trim());
  return lines.slice(1).map((line) => {
    const cols = line.split(",");
    /** @type {Record<string, string>} */
    const row = {};
    header.forEach((key, i) => {
      row[key] = (cols[i] ?? "").trim();
    });
    return row;
  });
}

/**
 * @param {Record<string, string>[]} simRows
 * @param {Record<string, string>[]} trackRows
 */
function mergeReplayFrames(simRows, trackRows) {
  const tracksByFrame = new Map();
  for (const row of trackRows) {
    tracksByFrame.set(Number(row.frame_index), row);
  }

  return simRows.map((sim) => {
    const track = tracksByFrame.get(Number(sim.frame_index));
    const frame = {
      frame_index: Number(sim.frame_index),
      time_s: Number(sim.time_s),
      interceptor: {
        x: Number(sim.interceptor_x),
        y: Number(sim.interceptor_y),
      },
      target: {
        x: Number(sim.target_x),
        y: Number(sim.target_y),
      },
      miss_distance: Number(sim.miss_distance),
    };

    if (track) {
      frame.hud =
        `frame ${frame.frame_index} · t=${frame.time_s.toFixed(2)}s · miss=${frame.miss_distance.toFixed(1)} · track px (${Number(track.pos_x).toFixed(0)}, ${Number(track.pos_y).toFixed(0)})`;
    } else {
      frame.hud = `frame ${frame.frame_index} · t=${frame.time_s.toFixed(2)}s · miss=${frame.miss_distance.toFixed(1)}`;
    }

    return frame;
  });
}

/**
 * @param {string} runId
 */
async function loadReplayRun(runId) {
  const id = runId.trim();
  const statusRes = await fetch(`/v1/runs/${encodeURIComponent(id)}`);
  if (!statusRes.ok) {
    const body = await statusRes.json().catch(() => ({}));
    throw new Error(body.error || `HTTP ${statusRes.status}`);
  }

  const status = await statusRes.json();
  const simUrl = status.artifacts?.sim_csv;
  if (!simUrl) {
    throw new Error("Run has no sim.csv — use an intercept run");
  }

  const fetches = [fetch(simUrl).then((r) => {
    if (!r.ok) throw new Error(`sim.csv HTTP ${r.status}`);
    return r.text();
  })];

  const tracksUrl = status.artifacts?.tracks_csv;
  if (tracksUrl) {
    fetches.push(
      fetch(tracksUrl).then((r) => {
        if (!r.ok) throw new Error(`tracks.csv HTTP ${r.status}`);
        return r.text();
      })
    );
  }

  const texts = await Promise.all(fetches);
  const simRows = parseCsv(texts[0]);
  const trackRows = tracksUrl ? parseCsv(texts[1]) : [];

  const frames = mergeReplayFrames(simRows, trackRows);
  if (frames.length === 0) {
    throw new Error("No frames in sim.csv");
  }

  let intervalMs = 33;
  if (frames.length > 1) {
    const dt = (frames[1].time_s - frames[0].time_s) * 1000;
    if (dt > 0) intervalMs = Math.max(16, Math.round(dt));
  }

  return { status, frames, intervalMs };
}

function stopReplayAnimation() {
  if (replayAnimationTimer !== null) {
    clearTimeout(replayAnimationTimer);
    replayAnimationTimer = null;
  }
}

function drawReplayIdleCanvas() {
  const canvas = document.getElementById("replay-canvas");
  if (!canvas) return;

  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  const w = canvas.width;
  const h = canvas.height;
  clearCanvas(ctx, w, h);

  ctx.fillStyle = cssVar("--muted");
  ctx.font = "14px Segoe UI, system-ui, sans-serif";
  ctx.textAlign = "center";
  ctx.fillText("Enter a run_id and click Load", w / 2, h / 2);
}

/**
 * @param {number} index
 */
function showReplayFrame(index) {
  if (!replayState) return;

  const canvas = document.getElementById("replay-canvas");
  if (!canvas) return;

  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  const w = canvas.width;
  const h = canvas.height;
  const clamped = Math.max(0, Math.min(index, replayState.frames.length - 1));

  drawSimulationFrame(ctx, replayState.frames, clamped, replayState.bounds, w, h);

  const scrub = document.getElementById("replay-scrub");
  const scrubVal = document.getElementById("replay-scrub-val");
  if (scrub) scrub.value = String(clamped);
  if (scrubVal) {
    scrubVal.textContent = `${clamped} / ${replayState.frames.length - 1}`;
  }
}

function toggleReplayPlay() {
  if (!replayState) return;

  const playBtn = document.getElementById("replay-play");
  if (replayAnimationTimer !== null) {
    stopReplayAnimation();
    if (playBtn) playBtn.textContent = "Play";
    return;
  }

  if (playBtn) playBtn.textContent = "Pause";

  let index = Number(document.getElementById("replay-scrub")?.value || 0);

  function step() {
    if (!replayState) return;

    showReplayFrame(index);
    index += 1;

    if (index >= replayState.frames.length) {
      stopReplayAnimation();
      if (playBtn) playBtn.textContent = "Play";
      return;
    }

    replayAnimationTimer = setTimeout(step, replayState.intervalMs);
  }

  step();
}

/**
 * @param {SubmitEvent} event
 */
async function onReplaySubmit(event) {
  event.preventDefault();
  stopReplayAnimation();

  const form = /** @type {HTMLFormElement} */ (event.currentTarget);
  const loadBtn = document.getElementById("replay-load");
  const statusEl = document.getElementById("replay-status");
  const controls = document.getElementById("replay-controls");
  const playBtn = document.getElementById("replay-play");

  if (!statusEl) return;

  const runId = String(new FormData(form).get("run_id") || "").trim();
  if (!runId) return;

  if (loadBtn) loadBtn.disabled = true;
  statusEl.textContent = "Loading run…";
  statusEl.classList.remove("err");

  try {
    const { status, frames, intervalMs } = await loadReplayRun(runId);
    replayState = {
      frames,
      bounds: computeBounds(frames),
      intervalMs,
    };

    const scrub = document.getElementById("replay-scrub");
    if (scrub) {
      scrub.min = "0";
      scrub.max = String(frames.length - 1);
      scrub.value = "0";
    }

    if (controls) controls.hidden = false;
    if (playBtn) playBtn.textContent = "Play";

    const mode = status.mode ? status.mode.toUpperCase() : "RUN";
    const minMiss =
      status.min_miss_distance != null
        ? ` · min miss ${Number(status.min_miss_distance).toFixed(1)}`
        : "";
    statusEl.textContent = `Loaded ${runId} · ${mode} · ${frames.length} frames${minMiss}`;

    showReplayFrame(0);
  } catch (err) {
    replayState = null;
    if (controls) controls.hidden = true;
    statusEl.textContent = `Error: ${err.message}`;
    statusEl.classList.add("err");
    drawReplayIdleCanvas();
  } finally {
    if (loadBtn) loadBtn.disabled = false;
  }
}

function stopAnimation() {
  if (animationTimer !== null) {
    clearTimeout(animationTimer);
    animationTimer = null;
  }
}

/**
 * @param {object} result — SimRunResponse JSON
 * @param {HTMLElement} statusEl
 * @param {HTMLButtonElement} goBtn
 */
function playAnimation(result, statusEl, goBtn) {
  stopAnimation();

  const canvas = document.getElementById("sim-canvas");
  if (!canvas) return;

  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  const w = canvas.width;
  const h = canvas.height;
  const frames = result.frames;

  if (!frames || frames.length === 0) {
    statusEl.textContent = "No frames returned.";
    goBtn.disabled = false;
    return;
  }

  const bounds = computeBounds(frames);
  const intervalMs = Math.max(16, Math.round(result.dt_seconds * 1000));
  let index = 0;

  statusEl.textContent = `Running ${result.law.toUpperCase()} · ${result.frame_count} frames…`;

  function step() {
    drawSimulationFrame(ctx, frames, index, bounds, w, h);
    index += 1;

    if (index < frames.length) {
      animationTimer = setTimeout(step, intervalMs);
    } else {
      animationTimer = null;
      statusEl.textContent =
        `Done · ${result.law.toUpperCase()} · min miss ${result.min_miss_distance.toFixed(1)} · ${result.frame_count} frames`;
      goBtn.disabled = false;
    }
  }

  step();
}

function drawIdleCanvas() {
  const canvas = document.getElementById("sim-canvas");
  if (!canvas) return;

  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  const w = canvas.width;
  const h = canvas.height;
  clearCanvas(ctx, w, h);

  ctx.fillStyle = cssVar("--muted");
  ctx.font = "14px Segoe UI, system-ui, sans-serif";
  ctx.textAlign = "center";
  ctx.fillText("Set initial conditions and click Go", w / 2, h / 2);
}

/**
 * @param {SubmitEvent} event
 */
async function onSimSubmit(event) {
  event.preventDefault();

  const form = /** @type {HTMLFormElement} */ (event.currentTarget);
  const goBtn = document.getElementById("sim-go");
  const statusEl = document.getElementById("sim-status");

  if (!goBtn || !statusEl) return;

  stopAnimation();
  goBtn.disabled = true;
  statusEl.textContent = "Running simulation…";
  statusEl.classList.remove("err");

  try {
    const payload = readFormPayload(form);
    const result = await fetchSimRun(payload);
    playAnimation(result, statusEl, goBtn);
  } catch (err) {
    statusEl.textContent = `Error: ${err.message}`;
    statusEl.classList.add("err");
    goBtn.disabled = false;
    drawIdleCanvas();
  }
}

document.addEventListener("DOMContentLoaded", () => {
  checkHealth();
  drawIdleCanvas();
  drawReplayIdleCanvas();

  const form = document.getElementById("sim-form");
  if (form) {
    form.addEventListener("submit", onSimSubmit);
  }

  const replayForm = document.getElementById("replay-form");
  if (replayForm) {
    replayForm.addEventListener("submit", onReplaySubmit);
  }

  const scrub = document.getElementById("replay-scrub");
  if (scrub) {
    scrub.addEventListener("input", () => {
      stopReplayAnimation();
      const playBtn = document.getElementById("replay-play");
      if (playBtn) playBtn.textContent = "Play";
      showReplayFrame(Number(scrub.value));
    });
  }

  const playBtn = document.getElementById("replay-play");
  if (playBtn) {
    playBtn.addEventListener("click", toggleReplayPlay);
  }
});
