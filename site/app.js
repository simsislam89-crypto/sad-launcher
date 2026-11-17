// Sad Launcher site: GSAP animations, eroot.shop-style FAQ search & sweep, OS auto-installer selection, voxel terrain
(() => {
  "use strict";
  const root = document.documentElement;
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const fine = matchMedia("(hover: hover) and (pointer: fine)").matches;
  const cores = navigator.hardwareConcurrency || 4;
  const memory = navigator.deviceMemory || 8;
  const saveData = navigator.connection && navigator.connection.saveData;
  const lite = cores <= 4 || memory <= 4 || !!saveData;
  if (lite) root.classList.add("lite");

  const hasGsap = !reduce && typeof window.gsap !== "undefined";
  if (hasGsap) {
    root.classList.add("has-gsap");
    if (typeof window.ScrollTrigger !== "undefined") {
      window.gsap.registerPlugin(window.ScrollTrigger);
    }
  }

  // ---------- count up ----------
  function countUp(el) {
    if (el.dataset.counted) return;
    el.dataset.counted = "1";
    const target = parseFloat(el.dataset.count);
    const dec = parseInt(el.dataset.dec || "0", 10);
    const fmt = (v) => v.toFixed(dec).replace(".", ",");
    if (reduce || target === 0) { el.textContent = fmt(target); return; }
    const obj = { v: 0 };
    if (hasGsap) {
      window.gsap.to(obj, {
        v: target,
        duration: 1.15,
        ease: "power3.out",
        onUpdate: () => { el.textContent = fmt(obj.v); },
      });
      return;
    }
    const t0 = performance.now();
    const step = (now) => {
      const t = Math.min(1, (now - t0) / 1100);
      const k = 1 - Math.pow(2, -10 * t);
      el.textContent = fmt(target * (t >= 1 ? 1 : k));
      if (t < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  }

  // ---------- GSAP hero, container scroll tilt, and reveals ----------
  const reveals = Array.from(document.querySelectorAll(".reveal"));
  const revealEl = (el) => {
    if (el.classList.contains("in")) return;
    el.classList.add("in");
    el.querySelectorAll("[data-count]").forEach(countUp);
  };

  if (hasGsap && !reduce) {
    window.gsap.fromTo(
      ".hero-in",
      { y: 20, opacity: 0 },
      { y: 0, opacity: 1, duration: 0.72, stagger: 0.08, ease: "power3.out", clearProps: "transform,opacity" },
    );

    const tiltFrame = document.querySelector("#tilt .tilt__frame");
    if (tiltFrame && !lite && typeof window.ScrollTrigger !== "undefined") {
      window.gsap.fromTo(
        tiltFrame,
        { rotateX: 22, scale: 0.92, opacity: 0.65 },
        {
          rotateX: 0,
          scale: 1,
          opacity: 1,
          ease: "power2.out",
          scrollTrigger: {
            trigger: "#tilt",
            start: "top 92%",
            end: "top 40%",
            scrub: 0.6,
          },
        },
      );
    }

    if (typeof window.ScrollTrigger !== "undefined") {
      reveals.forEach((el) => {
        window.ScrollTrigger.create({
          trigger: el,
          start: "top 94%",
          once: true,
          onEnter: () => revealEl(el),
          onEnterBack: () => revealEl(el),
        });
      });
    } else {
      reveals.forEach(revealEl);
    }
  } else if (!reduce && "IntersectionObserver" in window) {
    root.classList.add("js-anim");
    const io = new IntersectionObserver((entries) => {
      for (const e of entries) {
        if (!e.isIntersecting) continue;
        revealEl(e.target);
        io.unobserve(e.target);
      }
    }, { rootMargin: "0px 0px -4% 0px", threshold: 0.05 });
    reveals.forEach((el) => io.observe(el));
  } else {
    reveals.forEach(revealEl);
  }

  // Never leave sections invisible when landing on a hash (#download / #faq)
  const revealVisibleAndAbove = () => {
    const vh = window.innerHeight || 900;
    for (const el of reveals) {
      const r = el.getBoundingClientRect();
      if (r.top <= vh * 0.98) revealEl(el);
    }
  };
  addEventListener("hashchange", () => setTimeout(revealVisibleAndAbove, 60));
  addEventListener("scroll", revealVisibleAndAbove, { passive: true });
  setTimeout(revealVisibleAndAbove, 120);

  // ---------- spotlight border + magnetic CTA ----------
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

  // ---------- search demo ----------
  const typer = document.getElementById("typer");
  const typerOut = document.getElementById("typer-out");
  const pairs = [
    ["мини карта", "Minimap"],
    ["зкщьштутсу", "Prominence"],
    ["проминсе", "Prominence"],
    ["гамма", "Gamma Utils"],
  ];
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
      }, 65);
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

  // ---------- OS detection + copy buttons ----------
  const ua = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.userAgent || "";
  const os = /win/i.test(ua) ? "windows" : /mac|iphone|ipad/i.test(ua) ? "mac" : /linux|android|x11/i.test(ua) ? "linux" : "";
  if (os) {
    const card = document.querySelector(`.os__card[data-os="${os}"]`);
    if (card) card.classList.add("is-mine");
    const ctaMap = {
      windows: { label: "Скачать для Windows", href: "download/SadLauncherSetup.exe" },
      mac: { label: "Скачать для macOS", href: "download/install-macos.command" },
      linux: { label: "Скачать для Linux", href: "download/install-linux.sh" },
    };
    const target = ctaMap[os];
    if (target) {
      document.querySelectorAll("[data-download]").forEach((a) => {
        a.textContent = target.label;
        a.setAttribute("href", target.href);
        a.setAttribute("download", "");
      });
    }
  }

  function copyText(text, onDone) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(onDone, () => {
        fallbackCopy(text);
        onDone();
      });
      return;
    }
    fallbackCopy(text);
    onDone();
  }
  function fallbackCopy(text) {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.left = "-9999px";
    document.body.appendChild(ta);
    ta.select();
    try { document.execCommand("copy"); } catch (_) {}
    ta.remove();
  }

  document.querySelectorAll("[data-copy-target]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const sel = btn.getAttribute("data-copy-target");
      const src = sel && document.querySelector(sel);
      if (!src) return;
      const span = btn.querySelector("span");
      const prev = span ? span.textContent : "";
      copyText(src.textContent.trim(), () => {
        if (span) {
          span.textContent = "Скопировано";
          setTimeout(() => { span.textContent = prev; }, 1600);
        }
      });
    });
  });

  // ---------- FAQ (adapted from eroot.shop v85: sweep fill, search, deep-link, GSAP spring) ----------
  function initFaq() {
    const box = document.getElementById("faqList");
    if (!box) return;
    const items = Array.from(box.querySelectorAll("details.faq__item"));
    const input = document.getElementById("stfQ");
    const clearBtn = document.getElementById("stfClear");
    const countEl = document.getElementById("stfCount");
    const emptyEl = document.getElementById("stfEmpty");
    const emptyTxt = document.getElementById("stfEmptyTxt");
    const resetBtn = document.getElementById("stfReset");

    const escHtml = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    const PLUS =
      '<span class="stf-cell" aria-hidden="true"><svg class="stf-plus" viewBox="0 0 24 24" fill="none">' +
      '<path d="M12 3v18M3 12h18" stroke="currentColor" stroke-width="1.9" stroke-linecap="round"/></svg></span>';

    items.forEach((d) => {
      const sum = d.querySelector("summary");
      if (!sum) return;
      const qTxt = sum.textContent.trim();
      const row = (inv) =>
        `<span class="stf-row${inv ? " stf-row--inv" : ""}"${inv ? ' aria-hidden="true"' : ""}>` +
        `<span class="stf-label"><span class="stf-q">${escHtml(qTxt)}</span><span class="stf-snip"></span></span>` +
        `${PLUS}</span>`;
      sum.innerHTML = row(false) + row(true);
      d.insertAdjacentHTML("afterbegin", '<span class="stf-fill" aria-hidden="true"></span>');
      d.insertAdjacentHTML(
        "beforeend",
        '<div class="stf-tools"><button type="button" class="stf-copy">Ссылка на ответ</button></div>',
      );

      const bodyEl = document.createElement("div");
      bodyEl.className = "stf-body";
      for (const n of Array.from(d.childNodes)) {
        if (n === sum || (n.classList && n.classList.contains("stf-fill"))) continue;
        bodyEl.appendChild(n);
      }
      d.appendChild(bodyEl);
      d.__stfBody = bodyEl;
      d.__qText = qTxt;
      d.__aText = (bodyEl.querySelector("p") || { textContent: "" }).textContent.trim();
      d.setAttribute("data-state", d.open ? "open" : "closed");
    });

    const openItem = (d) => {
      items.forEach((other) => { if (other !== d && other.open) closeItem(other); });
      clearTimeout(d.__stfT);
      d.open = true;
      d.setAttribute("data-state", "open");
      const b = d.__stfBody;
      if (!b || !hasGsap || reduce) return;
      window.gsap.killTweensOf(b);
      window.gsap.fromTo(b, { height: 0 }, { height: "auto", duration: 0.45, ease: "power3.out", clearProps: "height" });
      window.gsap.fromTo(b, { opacity: 0, y: -8 }, { opacity: 1, y: 0, duration: 0.5, ease: "power3.out", clearProps: "transform,opacity" });
    };

    const closeItem = (d) => {
      if (!d.open) return;
      if (reduce || !hasGsap || !d.__stfBody) {
        d.setAttribute("data-state", "closed");
        d.open = false;
        return;
      }
      d.setAttribute("data-state", "closing");
      clearTimeout(d.__stfT);
      const b = d.__stfBody;
      window.gsap.killTweensOf(b);
      window.gsap.to(b, {
        height: 0,
        opacity: 0,
        y: -4,
        duration: 0.34,
        ease: "power2.inOut",
        onComplete: () => {
          d.open = false;
          d.setAttribute("data-state", "closed");
          window.gsap.set(b, { clearProps: "height,transform,opacity" });
        },
      });
    };

    items.forEach((d) => {
      const sum = d.querySelector("summary");
      sum.addEventListener("click", (e) => {
        e.preventDefault();
        if (d.getAttribute("data-state") === "open") closeItem(d);
        else openItem(d);
      });
      sum.addEventListener("keydown", (e) => {
        const vis = items.filter((o) => !o.hidden);
        const idx = vis.indexOf(d);
        let to = -1;
        if (e.key === "ArrowDown") to = (idx + 1) % vis.length;
        else if (e.key === "ArrowUp") to = (idx - 1 + vis.length) % vis.length;
        else if (e.key === "Home") to = 0;
        else if (e.key === "End") to = vis.length - 1;
        if (to >= 0) {
          e.preventDefault();
          vis[to].querySelector("summary").focus();
        }
      });
      const copyBtn = d.querySelector(".stf-copy");
      if (copyBtn) {
        copyBtn.addEventListener("click", () => {
          const url = `${location.origin}${location.pathname}#${d.id}`;
          copyText(url, () => {
            copyBtn.textContent = "Ссылка скопирована";
            setTimeout(() => { copyBtn.textContent = "Ссылка на ответ"; }, 1800);
          });
        });
      }
    });

    const highlight = (text, words) => {
      const low = text.toLowerCase();
      const ranges = [];
      for (const w of words) {
        let from = 0;
        while (true) {
          const idx = low.indexOf(w, from);
          if (idx < 0) break;
          ranges.push([idx, idx + w.length]);
          from = idx + w.length;
        }
      }
      if (!ranges.length) return escHtml(text);
      ranges.sort((a, b) => a[0] - b[0]);
      const merged = [ranges[0]];
      for (let i = 1; i < ranges.length; i++) {
        const last = merged[merged.length - 1];
        if (ranges[i][0] <= last[1]) last[1] = Math.max(last[1], ranges[i][1]);
        else merged.push(ranges[i]);
      }
      let out = "";
      let pos = 0;
      for (const [s, e] of merged) {
        out += escHtml(text.slice(pos, s)) + `<mark class="stf-mark">${escHtml(text.slice(s, e))}</mark>`;
        pos = e;
      }
      return out + escHtml(text.slice(pos));
    };

    const runFilter = () => {
      if (!input) return;
      const raw = input.value.trim();
      const words = raw ? raw.toLowerCase().split(/\s+/).filter(Boolean) : [];
      if (clearBtn) clearBtn.hidden = !raw;
      let shown = 0;
      for (const d of items) {
        const hay = `${d.__qText} ${d.__aText}`.toLowerCase();
        const ok = !words.length || words.every((w) => hay.includes(w));
        d.hidden = !ok;
        if (ok) shown += 1;
        d.querySelectorAll(".stf-q").forEach((el) => {
          el.innerHTML = words.length ? highlight(d.__qText, words) : escHtml(d.__qText);
        });
        const snip = words.length && ok && words.some((w) => d.__aText.toLowerCase().includes(w))
          ? d.__aText.slice(0, 96) + (d.__aText.length > 96 ? "…" : "")
          : "";
        d.querySelectorAll(".stf-snip").forEach((el) => {
          el.innerHTML = snip ? highlight(snip, words) : "";
        });
      }
      if (countEl) {
        countEl.textContent = raw ? `${shown} из ${items.length}` : "";
      }
      if (emptyEl) {
        emptyEl.hidden = shown > 0;
        if (emptyTxt && raw) emptyTxt.textContent = `По запросу «${raw}» вопросов нет.`;
      }
    };

    if (input) input.addEventListener("input", runFilter);
    if (clearBtn) {
      clearBtn.addEventListener("click", () => {
        input.value = "";
        runFilter();
        input.focus();
      });
    }
    if (resetBtn) {
      resetBtn.addEventListener("click", () => {
        if (input) input.value = "";
        runFilter();
        if (input) input.focus();
      });
    }

    const openHashFaq = () => {
      const id = location.hash.slice(1);
      if (!id) return;
      const match = items.find((d) => d.id === id);
      if (match) {
        openItem(match);
        setTimeout(() => match.scrollIntoView({ behavior: "smooth", block: "center" }), 60);
      }
    };
    openHashFaq();
    addEventListener("hashchange", openHashFaq);
  }
  initFaq();

  // ---------- hero: procedural voxel terrain ----------
  const canvas = document.getElementById("terrain");
  if (!canvas || !canvas.getContext) return;
  const ctx = canvas.getContext("2d", { alpha: false });

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
  const TILE = 8;
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
          ctx.fillStyle = `rgb(${g(light * 0.55)},${g(light * 0.55)},${g(light * 0.58)})`;
          ctx.beginPath(); ctx.moveTo(px - TILE, py); ctx.lineTo(px, py + TILE * 0.5); ctx.lineTo(px, py + TILE * 1.4); ctx.lineTo(px - TILE, py + TILE * 0.9); ctx.fill();
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
    if (now - last < 50) return;
    last = now;
    frame(now);
  };
  const start = () => { if (!running && !reduce && !lite) { running = true; raf = requestAnimationFrame(loop); } };
  const stop = () => { running = false; cancelAnimationFrame(raf); };
  new IntersectionObserver((es) => { es[0].isIntersecting ? start() : stop(); }).observe(canvas);
  document.addEventListener("visibilitychange", () => (document.hidden ? stop() : start()));
  let rz = 0;
  addEventListener("resize", () => {
    clearTimeout(rz);
    rz = setTimeout(() => { resize(); frame(performance.now()); }, 150);
  });
})();
