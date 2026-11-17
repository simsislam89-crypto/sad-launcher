// Sad Launcher site. No libraries; everything degrades on weak devices.
(() => {
  "use strict";
  const root = document.documentElement;
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const fine = matchMedia("(hover: hover) and (pointer: fine)").matches;
  const cores = navigator.hardwareConcurrency || 4;
  const memory = navigator.deviceMemory || 8;
  const saveData = navigator.connection && navigator.connection.saveData;
  // Weak device: solid surfaces instead of blur, one still terrain frame.
  const lite = cores <= 4 || memory <= 4 || !!saveData;
  if (lite) root.classList.add("lite");

  // ---------- reveal on scroll ----------
  const reveals = document.querySelectorAll(".reveal");
  if (reduce || !("IntersectionObserver" in window)) {
    reveals.forEach((el) => el.classList.add("in"));
  } else {
    const io = new IntersectionObserver((entries) => {
      for (const e of entries) {
        if (!e.isIntersecting) continue;
        e.target.classList.add("in");
        e.target.querySelectorAll("[data-count]").forEach(countUp);
        io.unobserve(e.target);
      }
    }, { rootMargin: "0px 0px -8% 0px", threshold: 0.12 });
    reveals.forEach((el) => io.observe(el));
  }

  // ---------- count up (real numbers only) ----------
  function countUp(el) {
    const target = parseFloat(el.dataset.count);
    const dec = parseInt(el.dataset.dec || "0", 10);
    const fmt = (v) => v.toFixed(dec).replace(".", ",");
    if (reduce || target === 0) { el.textContent = fmt(target); return; }
    const t0 = performance.now();
    const dur = 1100;
    const step = (now) => {
      const t = Math.min(1, (now - t0) / dur);
      const k = 1 - Math.pow(2, -10 * t);
      el.textContent = fmt(target * (t >= 1 ? 1 : k));
      if (t < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  }

  // ---------- spotlight border + magnetic CTA (mouse only) ----------
  if (fine && !lite && !reduce) {
    document.querySelectorAll(".spot").forEach((card) => {
      card.addEventListener("pointermove", (e) => {
        const r = card.getBoundingClientRect();
        card.style.setProperty("--mx", `${e.clientX - r.left}px`);
        card.style.setProperty("--my", `${e.clientY - r.top}px`);
      }, { passive: true });
    });
    document.querySelectorAll(".magnet").forEach((btn) => {
      btn.addEventListener("pointermove", (e) => {
        const r = btn.getBoundingClientRect();
        const x = (e.clientX - r.left - r.width / 2) * 0.18;
        const y = (e.clientY - r.top - r.height / 2) * 0.3;
        btn.style.transform = `translate(${x}px, ${y}px)`;
      }, { passive: true });
      btn.addEventListener("pointerleave", () => { btn.style.transform = ""; });
    });
  }

  // ---------- search demo: real smart-search examples ----------
  const typer = document.getElementById("typer");
  const typerOut = document.getElementById("typer-out");
  const pairs = [["мини карта", "Minimap"], ["зкщьштутсу", "Prominence"], ["проминсе", "Prominence"], ["шейдеры", "Shaders"]];
  if (typer && !reduce) {
    let i = 0;
    let timer = 0;
    const show = () => {
      const [q, a] = pairs[i % pairs.length];
      let n = 0;
      typerOut.textContent = "";
      clearInterval(timer);
      timer = setInterval(() => {
        n += 1;
        typer.textContent = q.slice(0, n);
        if (n >= q.length) { clearInterval(timer); typerOut.textContent = a; }
      }, 70);
      i += 1;
    };
    const vis = new IntersectionObserver((es) => {
      for (const e of es) {
        if (e.isIntersecting && !typer.dataset.on) {
          typer.dataset.on = "1";
          show();
          typer.dataset.loop = String(setInterval(show, 2600));
        } else if (!e.isIntersecting && typer.dataset.on) {
          delete typer.dataset.on;
          clearInterval(Number(typer.dataset.loop));
          clearInterval(timer);
        }
      }
    });
    vis.observe(typer);
  }

  // ---------- OS detection for the download section ----------
  const ua = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.userAgent || "";
  const os = /win/i.test(ua) ? "windows" : /mac|iphone|ipad/i.test(ua) ? "mac" : /linux|android|x11/i.test(ua) ? "linux" : "";
  if (os) {
    const card = document.querySelector(`.os__card[data-os="${os}"]`);
    if (card) card.classList.add("is-mine");
    const hint = document.getElementById("os-hint");
    if (os !== "windows" && hint) {
      hint.hidden = false;
      hint.textContent = os === "mac"
        ? "Для macOS сборка пока в разработке. Установщик для Windows можно скачать выше."
        : "Для Linux сборка пока в разработке. Установщик для Windows можно скачать выше.";
      document.querySelectorAll("[data-download]").forEach((a) => { a.textContent = "Скачать"; });
    }
  }

  // ---------- hero: procedural voxel terrain ----------
  const canvas = document.getElementById("terrain");
  if (!canvas || !canvas.getContext) return;
  const ctx = canvas.getContext("2d", { alpha: false });

  // Value noise, seeded, smooth.
  const perm = new Uint8Array(512);
  let seed = 1337;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  for (let i = 0; i < 256; i++) perm[i] = i;
  for (let i = 255; i > 0; i--) { const j = Math.floor(rnd() * (i + 1)); [perm[i], perm[j]] = [perm[j], perm[i]]; }
  for (let i = 0; i < 256; i++) perm[i + 256] = perm[i];
  const vals = new Float32Array(256).map(() => rnd());
  const lerp = (a, b, t) => a + (b - a) * t;
  const fade = (t) => t * t * (3 - 2 * t);
  function noise(x, y) {
    const xi = Math.floor(x), yi = Math.floor(y);
    const xf = x - xi, yf = y - yi;
    const h = (a, b) => vals[perm[(perm[a & 255] + b) & 255]];
    const u = fade(xf), v = fade(yf);
    return lerp(lerp(h(xi, yi), h(xi + 1, yi), u), lerp(h(xi, yi + 1), h(xi + 1, yi + 1), u), v);
  }
  const height = (x, y) => {
    const n = noise(x * 0.09, y * 0.09) * 0.65 + noise(x * 0.22, y * 0.22) * 0.25 + noise(x * 0.6, y * 0.6) * 0.1;
    return Math.floor(n * 11);
  };

  // Rendered small and scaled up with pixelated sampling: cheap and
  // gives the blocky, dithered look.
  let W = 0, H = 0, scale = 3;
  function resize() {
    const cssW = canvas.clientWidth, cssH = canvas.clientHeight;
    scale = lite ? 4 : 3;
    W = Math.max(160, Math.round(cssW / scale));
    H = Math.max(120, Math.round(cssH / scale));
    canvas.width = W;
    canvas.height = H;
  }

  const GRID = lite ? 26 : 34;
  const TILE = 8; // half-width of a cube top in canvas px
  function frame(t) {
    ctx.fillStyle = "#0b0b0d";
    ctx.fillRect(0, 0, W, H);
    const drift = t * 0.00035;
    const ox = W * 0.62, oy = H * 0.18;
    const dx = Math.floor(drift), fx = drift - dx;
    for (let gy = 0; gy < GRID; gy++) {
      for (let gx = 0; gx < GRID; gx++) {
        const wx = gx + dx, wy = gy;
        const h = height(wx, wy);
        const px = ox + (gx - fx - gy) * TILE;
        const baseY = oy + (gx - fx + gy) * TILE * 0.5;
        const fog = Math.min(1, (gx + gy) / (GRID * 1.4));
        for (let z = Math.max(0, h - 2); z <= h; z++) {
          const py = baseY - z * TILE * 0.9 + 40;
          if (px < -TILE * 2 || px > W + TILE * 2 || py > H + TILE * 2) continue;
          const top = z === h;
          const light = 26 + h * 9;
          const g = (k) => Math.round(Math.min(235, k * (0.35 + fog * 0.65)));
          // left face
          ctx.fillStyle = `rgb(${g(light * 0.55)},${g(light * 0.55)},${g(light * 0.58)})`;
          ctx.beginPath(); ctx.moveTo(px - TILE, py); ctx.lineTo(px, py + TILE * 0.5); ctx.lineTo(px, py + TILE * 1.4); ctx.lineTo(px - TILE, py + TILE * 0.9); ctx.fill();
          // right face
          ctx.fillStyle = `rgb(${g(light * 0.38)},${g(light * 0.38)},${g(light * 0.4)})`;
          ctx.beginPath(); ctx.moveTo(px + TILE, py); ctx.lineTo(px, py + TILE * 0.5); ctx.lineTo(px, py + TILE * 1.4); ctx.lineTo(px + TILE, py + TILE * 0.9); ctx.fill();
          if (top) {
            ctx.fillStyle = `rgb(${g(light)},${g(light)},${g(light * 1.02)})`;
            ctx.beginPath(); ctx.moveTo(px, py - TILE * 0.5); ctx.lineTo(px + TILE, py); ctx.lineTo(px, py + TILE * 0.5); ctx.lineTo(px - TILE, py); ctx.fill();
          }
        }
      }
    }
  }

  resize();
  frame(0);
  let raf = 0, last = 0, running = false;
  const loop = (now) => {
    raf = requestAnimationFrame(loop);
    if (now - last < 50) return; // ~20 fps is plenty for a slow drift
    last = now;
    frame(now);
  };
  const start = () => { if (!running && !reduce && !lite) { running = true; raf = requestAnimationFrame(loop); } };
  const stop = () => { running = false; cancelAnimationFrame(raf); };
  // Only animate while the hero is on screen and the tab is visible.
  new IntersectionObserver((es) => { es[0].isIntersecting ? start() : stop(); }).observe(canvas);
  document.addEventListener("visibilitychange", () => (document.hidden ? stop() : start()));
  let rz = 0;
  addEventListener("resize", () => {
    clearTimeout(rz);
    rz = setTimeout(() => { resize(); frame(performance.now()); }, 150);
  });
})();
