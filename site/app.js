/* Sad Launcher — site interactions:
   - 21st.dev GridPatternSpotlight + Radial Horizon Arc
   - ReactBits WebGL2 Aurora shader
   - ReactBits WebGL2 Particles shader (parallax + repel + click-burst)
   - ReactBits BlurText (word-by-word blur reveal + focusOnHover)
   - ReactBits MagicBento (proximity border glow, spotlight, 3D tilt, click wave, star specks, scrubbable chart)
   - ReactBits SpotlightCard (proximity light sharing + press flare)
   - ReactBits Floating Magnifying Dock
   - eroot.shop style FAQ search + GSAP ScrollTrigger */
(function () {
  'use strict';

  var reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  var WINDOWS_INSTALLER_URL = 'https://github.com/simsislam89-crypto/sad-launcher/raw/main/site/download/SadLauncherSetup.exe';

  function detectOS() {
    var ua = (navigator.userAgent || '').toLowerCase();
    var platform = ((navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || '').toLowerCase();
    var s = ua + ' ' + platform;
    if (s.indexOf('mac') !== -1 || s.indexOf('darwin') !== -1) return 'mac';
    if (s.indexOf('linux') !== -1 && s.indexOf('android') === -1) return 'linux';
    return 'windows';
  }

  function initOS() {
    var os = detectOS();
    var ctas = document.querySelectorAll('[data-download]');
    var labels = {
      windows: { text: 'Скачать для Windows', href: WINDOWS_INSTALLER_URL },
      mac:     { text: 'Скачать для macOS',   href: 'download/install-macos.command' },
      linux:   { text: 'Скачать для Linux',   href: 'download/install-linux.sh' }
    };
    var cfg = labels[os] || labels.windows;
    ctas.forEach(function (el) {
      el.textContent = cfg.text;
      el.setAttribute('href', cfg.href);
    });

    var card = document.querySelector('.os__card[data-os="' + os + '"]');
    if (card) card.classList.add('is-detected');

    var hint = document.getElementById('os-hint');
    if (hint) {
      var names = { windows: 'Windows', mac: 'macOS', linux: 'Linux' };
      hint.textContent = 'Определена система: ' + (names[os] || 'Windows') + '. Выделен подходящий автоустановщик.';
      hint.hidden = false;
    }
  }

  function initCopyButtons() {
    var btns = document.querySelectorAll('[data-copy-target]');
    btns.forEach(function (btn) {
      btn.addEventListener('click', function () {
        var sel = btn.getAttribute('data-copy-target');
        var target = sel ? document.querySelector(sel) : null;
        if (!target) return;
        var text = (target.textContent || '').trim();
        if (!text) return;

        var labelSpan = btn.querySelector('span');
        var prev = labelSpan ? labelSpan.textContent : '';
        function markCopied() {
          btn.classList.add('is-copied');
          if (labelSpan) labelSpan.textContent = 'Скопировано';
          setTimeout(function () {
            btn.classList.remove('is-copied');
            if (labelSpan) labelSpan.textContent = prev || 'Копировать';
          }, 1800);
        }

        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(text).then(markCopied).catch(function () {
            fallbackCopy(text, markCopied);
          });
        } else {
          fallbackCopy(text, markCopied);
        }
      });
    });
  }

  function fallbackCopy(text, done) {
    var ta = document.createElement('textarea');
    ta.value = text;
    ta.style.position = 'fixed';
    ta.style.opacity = '0';
    document.body.appendChild(ta);
    ta.select();
    try { document.execCommand('copy'); } catch (_) {}
    document.body.removeChild(ta);
    if (done) done();
  }

  /* 1. 21st.dev GridPatternSpotlight */
  function initHeroGridSpot() {
    var hero = document.getElementById('hero');
    var spot = document.getElementById('hero-grid-spot');
    if (!hero || !spot) return;

    var size = 340;
    var tx = -9999, ty = -9999, cx = -9999, cy = -9999;
    var raf = 0;

    function tick() {
      raf = 0;
      cx += (tx - cx) * 0.18;
      cy += (ty - cy) * 0.18;
      spot.style.transform = 'translate3d(' + (cx - size / 2).toFixed(1) + 'px,' + (cy - size / 2).toFixed(1) + 'px,0)';
      if (Math.hypot(tx - cx, ty - cy) > 0.5) {
        raf = requestAnimationFrame(tick);
      }
    }

    hero.addEventListener('pointermove', function (e) {
      var r = hero.getBoundingClientRect();
      tx = e.clientX - r.left;
      ty = e.clientY - r.top;
      if (cx < -1000) { cx = tx; cy = ty; }
      if (!raf) raf = requestAnimationFrame(tick);
    }, { passive: true });
  }

  /* 2. ReactBits WebGL2 Aurora Shader */
  function initAurora() {
    var canvas = document.getElementById('aurora-canvas');
    if (!canvas) return;
    var gl = canvas.getContext('webgl2', { alpha: true, antialias: true, premultipliedAlpha: true });
    if (!gl) return;

    var VERT = '#version 300 es\nlayout(location = 0) in vec2 position;\nvoid main() {\n  gl_Position = vec4(position, 0.0, 1.0);\n}\n';
    var FRAG = '#version 300 es\nprecision highp float;\nuniform float uTime;\nuniform float uAmplitude;\nuniform vec3 uColorStops[3];\nuniform vec2 uResolution;\nuniform float uBlend;\nout vec4 fragColor;\nvec3 permute(vec3 x) { return mod(((x * 34.0) + 1.0) * x, 289.0); }\nfloat snoise(vec2 v){\n  const vec4 C = vec4(0.211324865405187, 0.366025403784439, -0.577350269189626, 0.024390243902439);\n  vec2 i  = floor(v + dot(v, C.yy));\n  vec2 x0 = v - i + dot(i, C.xx);\n  vec2 i1 = (x0.x > x0.y) ? vec2(1.0, 0.0) : vec2(0.0, 1.0);\n  vec4 x12 = x0.xyxy + C.xxzz;\n  x12.xy -= i1;\n  i = mod(i, 289.0);\n  vec3 p = permute(permute(i.y + vec3(0.0, i1.y, 1.0)) + i.x + vec3(0.0, i1.x, 1.0));\n  vec3 m = max(0.5 - vec3(dot(x0, x0), dot(x12.xy, x12.xy), dot(x12.zw, x12.zw)), 0.0);\n  m = m * m;\n  m = m * m;\n  vec3 x = 2.0 * fract(p * C.www) - 1.0;\n  vec3 h = abs(x) - 0.5;\n  vec3 ox = floor(x + 0.5);\n  vec3 a0 = x - ox;\n  m *= 1.79284291400159 - 0.85373472095314 * (a0*a0 + h*h);\n  vec3 g;\n  g.x  = a0.x  * x0.x  + h.x  * x0.y;\n  g.yz = a0.yz * x12.xz + h.yz * x12.yw;\n  return 130.0 * dot(m, g);\n}\nvoid main() {\n  vec2 uv = gl_FragCoord.xy / uResolution;\n  float t = clamp(uv.x, 0.0, 1.0);\n  vec3 rampColor = t < 0.5 ? mix(uColorStops[0], uColorStops[1], t * 2.0) : mix(uColorStops[1], uColorStops[2], (t - 0.5) * 2.0);\n  float height = snoise(vec2(uv.x * 2.0 + uTime * 0.1, uTime * 0.22)) * 0.5 * uAmplitude;\n  height = exp(height);\n  height = (uv.y * 2.0 - height + 0.25);\n  float intensity = 0.55 * height;\n  float midPoint = 0.22;\n  float auroraAlpha = smoothstep(midPoint - uBlend * 0.5, midPoint + uBlend * 0.5, intensity);\n  auroraAlpha = clamp(auroraAlpha, 0.0, 1.0);\n  vec3 auroraColor = intensity * rampColor;\n  fragColor = vec4(auroraColor * auroraAlpha, auroraAlpha);\n}\n';

    function compile(type, src) {
      var s = gl.createShader(type);
      gl.shaderSource(s, src);
      gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) return null;
      return s;
    }
    var vs = compile(gl.VERTEX_SHADER, VERT);
    var fs = compile(gl.FRAGMENT_SHADER, FRAG);
    if (!vs || !fs) return;

    var prog = gl.createProgram();
    gl.attachShader(prog, vs);
    gl.attachShader(prog, fs);
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return;

    var vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    var buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);

    gl.useProgram(prog);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);

    var uTime = gl.getUniformLocation(prog, 'uTime');
    var uAmplitude = gl.getUniformLocation(prog, 'uAmplitude');
    var uColorStops = gl.getUniformLocation(prog, 'uColorStops');
    var uResolution = gl.getUniformLocation(prog, 'uResolution');
    var uBlend = gl.getUniformLocation(prog, 'uBlend');

    // Monochrome silver + subtle emerald stop (#ffffff, #9b9ba4, #82e6a5)
    gl.uniform3fv(uColorStops, new Float32Array([
      0.95, 0.95, 0.97,
      0.58, 0.60, 0.65,
      0.51, 0.90, 0.65
    ]));
    gl.uniform1f(uAmplitude, 0.95);
    gl.uniform1f(uBlend, 0.55);

    function resize() {
      var w = canvas.clientWidth || 1;
      var h = canvas.clientHeight || 1;
      var dpr = Math.min(window.devicePixelRatio || 1, 1.75);
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.uniform2f(uResolution, canvas.width, canvas.height);
    }
    resize();
    window.addEventListener('resize', resize, { passive: true });

    var start = performance.now();
    var visible = true;
    if ('IntersectionObserver' in window) {
      new IntersectionObserver(function (entries) {
        visible = entries[0].isIntersecting;
      }).observe(canvas);
    }

    function frame(now) {
      if (visible) {
        var t = reduceMotion ? 1.5 : (now - start) * 0.00055;
        gl.clear(gl.COLOR_BUFFER_BIT);
        gl.useProgram(prog);
        gl.bindVertexArray(vao);
        gl.uniform1f(uTime, t);
        gl.drawArrays(gl.TRIANGLES, 0, 3);
      }
      if (!reduceMotion) requestAnimationFrame(frame);
    }
    requestAnimationFrame(frame);
  }

  /* 3. ReactBits Interactive Particles (cursor parallax, repel & click-burst) */
  function initParticles() {
    var canvas = document.getElementById('particles-canvas');
    if (!canvas) return;
    var ctx = canvas.getContext('2d');
    if (!ctx) return;

    var w = 0, h = 0, dpr = Math.min(window.devicePixelRatio || 1, 2);
    var count = 90;
    var specks = [];
    var mouse = { x: 0.5, y: 0.5, tx: 0.5, ty: 0.5, active: false };
    var burst = { x: 0.5, y: 0.5, age: 99 };

    function resize() {
      var rect = canvas.getBoundingClientRect();
      w = rect.width || 1;
      h = rect.height || 1;
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }

    function seed() {
      specks.length = 0;
      for (var i = 0; i < count; i++) {
        var isEmerald = (i % 6 === 0);
        specks.push({
          x: Math.random(),
          y: Math.random(),
          z: 0.25 + Math.random() * 0.75,
          vx: (Math.random() - 0.5) * 0.00022,
          vy: (Math.random() - 0.5) * 0.00022,
          r: 0.9 + Math.random() * 1.4,
          phase: Math.random() * Math.PI * 2,
          speed: 0.8 + Math.random() * 1.8,
          rgb: isEmerald ? '130,230,165' : '255,255,255'
        });
      }
    }

    resize();
    seed();
    window.addEventListener('resize', resize, { passive: true });

    var hero = document.getElementById('hero');
    if (hero) {
      hero.addEventListener('pointermove', function (e) {
        var r = canvas.getBoundingClientRect();
        mouse.tx = (e.clientX - r.left) / Math.max(1, r.width);
        mouse.ty = (e.clientY - r.top) / Math.max(1, r.height);
        mouse.active = true;
      }, { passive: true });
      hero.addEventListener('pointerleave', function () {
        mouse.active = false;
      });
      hero.addEventListener('pointerdown', function (e) {
        var r = canvas.getBoundingClientRect();
        burst.x = (e.clientX - r.left) / Math.max(1, r.width);
        burst.y = (e.clientY - r.top) / Math.max(1, r.height);
        burst.age = 0;
      });
    }

    var last = performance.now();
    var visible = true;
    if ('IntersectionObserver' in window) {
      new IntersectionObserver(function (entries) {
        visible = entries[0].isIntersecting;
      }).observe(canvas);
    }

    function draw(now) {
      var dt = Math.min(0.05, (now - last) * 0.001);
      last = now;

      if (visible) {
        ctx.clearRect(0, 0, w, h);
        mouse.x += (mouse.tx - mouse.x) * 0.08;
        mouse.y += (mouse.ty - mouse.y) * 0.08;
        burst.age += dt * 1.6;

        var parX = (mouse.x - 0.5) * 28;
        var parY = (mouse.y - 0.5) * 20;

        for (var i = 0; i < specks.length; i++) {
          var s = specks[i];
          if (!reduceMotion) {
            s.x = (s.x + s.vx + 1) % 1;
            s.y = (s.y + s.vy + 1) % 1;
            s.phase += dt * s.speed;
          }
          var px = s.x * w + parX * s.z;
          var py = s.y * h + parY * s.z;

          if (mouse.active) {
            var dx = px - mouse.x * w;
            var dy = py - mouse.y * h;
            var dist = Math.hypot(dx, dy);
            if (dist > 0.1 && dist < 110) {
              var push = (1 - dist / 110) * 14 * s.z;
              px += (dx / dist) * push;
              py += (dy / dist) * push;
            }
          }

          if (burst.age < 1.2) {
            var bdx = px - burst.x * w;
            var bdy = py - burst.y * h;
            var bdist = Math.hypot(bdx, bdy);
            var waveR = burst.age * 260;
            var ring = Math.max(0, 1 - Math.abs(bdist - waveR) / 60);
            if (bdist > 0.1 && ring > 0) {
              var kick = ring * (1 - burst.age / 1.2) * 18;
              px += (bdx / bdist) * kick;
              py += (bdy / bdist) * kick;
            }
          }

          var twinkle = 0.28 + 0.45 * (0.5 + 0.5 * Math.sin(s.phase));
          ctx.fillStyle = 'rgba(' + s.rgb + ',' + (twinkle * s.z).toFixed(3) + ')';
          ctx.beginPath();
          ctx.arc(px, py, s.r * s.z, 0, Math.PI * 2);
          ctx.fill();
        }
      }
      if (!reduceMotion) requestAnimationFrame(draw);
    }
    requestAnimationFrame(draw);
  }

  /* 4. ReactBits BlurText (word-by-word blur reveal + focusOnHover) */
  function initBlurText() {
    var nodes = document.querySelectorAll('[data-blur-text]');
    nodes.forEach(function (el) {
      var raw = (el.textContent || '').trim();
      if (!raw) return;
      var words = raw.split(/\s+/);
      var focusHover = el.getAttribute('data-focus-hover') === 'true';
      el.textContent = '';

      var spans = [];
      words.forEach(function (word, idx) {
        var span = document.createElement('span');
        span.className = 'blur-word';
        span.textContent = word;
        if (!reduceMotion) {
          span.style.opacity = '0';
          span.style.filter = 'blur(10px)';
          span.style.transform = 'translateY(-14px)';
        }
        if (focusHover) {
          span.addEventListener('pointerenter', function () {
            el.classList.add('is-hovering-word');
            span.classList.add('is-hovered');
          });
          span.addEventListener('pointerleave', function () {
            span.classList.remove('is-hovered');
            if (!el.querySelector('.blur-word.is-hovered')) {
              el.classList.remove('is-hovering-word');
            }
          });
        }
        el.appendChild(span);
        spans.push(span);
        if (idx < words.length - 1) {
          el.appendChild(document.createTextNode(' '));
        }
      });

      if (reduceMotion) return;

      function revealWords() {
        spans.forEach(function (span, idx) {
          setTimeout(function () {
            span.style.opacity = '1';
            span.style.filter = 'blur(0px)';
            span.style.transform = 'translateY(0)';
          }, idx * 75);
        });
      }

      if ('IntersectionObserver' in window) {
        var io = new IntersectionObserver(function (entries) {
          if (entries[0].isIntersecting) {
            revealWords();
            io.disconnect();
          }
        }, { threshold: 0.15 });
        io.observe(el);
      } else {
        revealWords();
      }
    });
  }

  /* 5. ReactBits SpotlightCard (proximity 80px light sharing + border glow + press flare) */
  function initSpotlightCards() {
    var cards = Array.prototype.slice.call(document.querySelectorAll('[data-spotlight-card]'));
    if (!cards.length) return;

    var proximity = 90;
    var states = cards.map(function (el) {
      return {
        el: el,
        tx: 0, ty: 0, cx: 0, cy: 0,
        ta: 0, ca: 0,
        tba: 0, cba: 0,
        tp: 0, cp: 0,
        seeded: false
      };
    });

    var raf = 0;
    function tick() {
      raf = 0;
      var keep = false;
      for (var i = 0; i < states.length; i++) {
        var s = states[i];
        s.cx += (s.tx - s.cx) * 0.22;
        s.cy += (s.ty - s.cy) * 0.22;
        s.ca += (s.ta - s.ca) * 0.18;
        s.cba += (s.tba - s.cba) * 0.18;
        s.cp += (s.tp - s.cp) * 0.22;

        s.el.style.setProperty('--spot-x', s.cx.toFixed(1) + 'px');
        s.el.style.setProperty('--spot-y', s.cy.toFixed(1) + 'px');
        s.el.style.setProperty('--spot-a', s.ca.toFixed(3));
        s.el.style.setProperty('--spot-border-a', s.cba.toFixed(3));
        s.el.style.setProperty('--spot-press', s.cp.toFixed(3));

        if (
          Math.abs(s.tx - s.cx) > 0.3 ||
          Math.abs(s.ty - s.cy) > 0.3 ||
          Math.abs(s.ta - s.ca) > 0.005 ||
          Math.abs(s.tba - s.cba) > 0.005 ||
          Math.abs(s.tp - s.cp) > 0.005
        ) {
          keep = true;
        }
      }
      if (keep) raf = requestAnimationFrame(tick);
    }

    function schedule() {
      if (!raf) raf = requestAnimationFrame(tick);
    }

    document.addEventListener('pointermove', function (e) {
      for (var i = 0; i < states.length; i++) {
        var s = states[i];
        var r = s.el.getBoundingClientRect();
        var x = e.clientX - r.left;
        var y = e.clientY - r.top;
        var dx = x < 0 ? -x : x > r.width ? x - r.width : 0;
        var dy = y < 0 ? -y : y > r.height ? y - r.height : 0;
        var dist = Math.hypot(dx, dy);

        if (dist <= proximity) {
          s.tx = x;
          s.ty = y;
          if (!s.seeded) { s.cx = x; s.cy = y; s.seeded = true; }
          var falloff = dist === 0 ? 1 : 1 - dist / proximity;
          s.ta = falloff * falloff;
          s.tba = falloff;
        } else {
          s.ta = 0;
          s.tba = 0;
          s.tp = 0;
        }
      }
      schedule();
    }, { passive: true });

    states.forEach(function (s) {
      s.el.addEventListener('pointerdown', function () {
        s.tp = 1;
        schedule();
      });
      s.el.addEventListener('pointerup', function () {
        s.tp = 0;
        schedule();
      });
      s.el.addEventListener('pointercancel', function () {
        s.tp = 0;
        schedule();
      });
    });
  }

  /* 6. ReactBits MagicBento (proximity glow, spotlight, 3D tilt, click wave, stars, scrubbable chart) */
  function initMagicBento() {
    var grid = document.getElementById('magic-bento');
    if (!grid) return;

    var cards = Array.prototype.slice.call(grid.querySelectorAll('.magic-bento__card'));
    var spotRadius = 360;
    var nearRadius = spotRadius * 0.55;
    var fadeRadius = spotRadius * 0.85;

    var cardStates = cards.map(function (card) {
      var canvas = card.querySelector('.magic-bento__stars');
      var specks = [];
      for (var i = 0; i < 11; i++) {
        specks.push({
          x: Math.random(),
          y: Math.random(),
          vx: (Math.random() - 0.5) * 0.04,
          vy: (Math.random() - 0.5) * 0.04,
          r: 0.8 + Math.random() * 1.1,
          phase: Math.random() * Math.PI * 2,
          speed: 1.2 + Math.random() * 1.6
        });
      }
      return {
        card: card,
        canvas: canvas,
        ctx: canvas ? canvas.getContext('2d') : null,
        specks: specks,
        hot: 0,
        currentHot: 0,
        hovered: false,
        rx: 0, ry: 0, trx: 0, try: 0,
        wave: null
      };
    });

    function resizeStarCanvases() {
      var dpr = Math.min(window.devicePixelRatio || 1, 2);
      cardStates.forEach(function (st) {
        if (!st.canvas || !st.ctx) return;
        var w = st.card.clientWidth || 1;
        var h = st.card.clientHeight || 1;
        st.canvas.width = Math.round(w * dpr);
        st.canvas.height = Math.round(h * dpr);
        st.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      });
    }
    resizeStarCanvases();
    window.addEventListener('resize', resizeStarCanvases, { passive: true });

    grid.addEventListener('pointermove', function (e) {
      cardStates.forEach(function (st) {
        var r = st.card.getBoundingClientRect();
        var lx = e.clientX - r.left;
        var ly = e.clientY - r.top;
        st.card.style.setProperty('--mb-x', lx.toFixed(1) + 'px');
        st.card.style.setProperty('--mb-y', ly.toFixed(1) + 'px');

        var dx = Math.max(r.left - e.clientX, 0, e.clientX - r.right);
        var dy = Math.max(r.top - e.clientY, 0, e.clientY - r.bottom);
        var dist = Math.hypot(dx, dy);

        var near = 0;
        if (dist <= nearRadius) near = 1;
        else if (dist < fadeRadius) near = (fadeRadius - dist) / (fadeRadius - nearRadius);
        st.card.style.setProperty('--mb-near', near.toFixed(3));

        var inside = dx === 0 && dy === 0;
        st.hot = inside ? 1 : near * 0.55;
        st.hovered = inside;

        if (inside && !reduceMotion) {
          var nx = (lx / Math.max(1, r.width) - 0.5) * 2;
          var ny = (ly / Math.max(1, r.height) - 0.5) * 2;
          st.trx = -ny * 3.8;
          st.try = nx * 3.8;
        } else {
          st.trx = 0;
          st.try = 0;
        }
      });
    }, { passive: true });

    grid.addEventListener('pointerleave', function () {
      cardStates.forEach(function (st) {
        st.card.style.setProperty('--mb-near', '0');
        st.hot = 0;
        st.hovered = false;
        st.trx = 0;
        st.try = 0;
      });
    });

    cardStates.forEach(function (st) {
      st.card.addEventListener('pointerdown', function (e) {
        var r = st.card.getBoundingClientRect();
        var x = e.clientX - r.left;
        var y = e.clientY - r.top;
        var maxR = Math.hypot(Math.max(x, r.width - x), Math.max(y, r.height - y)) + 36;
        st.card.style.setProperty('--mb-wx', x.toFixed(1) + 'px');
        st.card.style.setProperty('--mb-wy', y.toFixed(1) + 'px');
        st.wave = { start: performance.now(), maxR: maxR };
      });
    });

    // Interactive Scrubbable Speed Chart inside Card 1
    initMagicBentoChart();

    var last = performance.now();
    var visible = true;
    if ('IntersectionObserver' in window) {
      new IntersectionObserver(function (entries) {
        visible = entries[0].isIntersecting;
      }).observe(grid);
    }

    function step(now) {
      var dt = Math.min(0.05, (now - last) * 0.001);
      last = now;

      if (visible) {
        var k = 1 - Math.exp(-12 * dt);
        cardStates.forEach(function (st) {
          st.currentHot += (st.hot - st.currentHot) * k;
          st.rx += (st.trx - st.rx) * k;
          st.ry += (st.try - st.ry) * k;

          st.card.style.setProperty('--mb-hot', st.currentHot.toFixed(3));
          st.card.style.setProperty('--mb-rx', st.rx.toFixed(2) + 'deg');
          st.card.style.setProperty('--mb-ry', st.ry.toFixed(2) + 'deg');

          if (st.wave) {
            var wt = Math.min(1, (now - st.wave.start) / 680);
            var we = 1 - Math.pow(1 - wt, 3);
            st.card.style.setProperty('--mb-wr', (we * st.wave.maxR).toFixed(1) + 'px');
            st.card.style.setProperty('--mb-wa', (1 - wt).toFixed(3));
            if (wt >= 1) {
              st.card.style.setProperty('--mb-wa', '0');
              st.wave = null;
            }
          }

          if (st.ctx && st.canvas) {
            var w = st.card.clientWidth || 1;
            var h = st.card.clientHeight || 1;
            st.ctx.clearRect(0, 0, w, h);
            var baseAlpha = st.hovered ? 0.85 : 0.25;
            for (var i = 0; i < st.specks.length; i++) {
              var s = st.specks[i];
              if (!reduceMotion) {
                var boost = st.hovered ? 1.8 : 0.55;
                s.x = (s.x + s.vx * dt * boost + 1) % 1;
                s.y = (s.y + s.vy * dt * boost + 1) % 1;
                s.phase += dt * s.speed;
              }
              var tw = 0.35 + 0.65 * (0.5 + 0.5 * Math.sin(s.phase));
              st.ctx.fillStyle = 'rgba(130, 230, 165, ' + (tw * baseAlpha * 0.65).toFixed(3) + ')';
              st.ctx.beginPath();
              st.ctx.arc(s.x * w, s.y * h, s.r, 0, Math.PI * 2);
              st.ctx.fill();
            }
          }
        });
      }
      if (!reduceMotion) requestAnimationFrame(step);
    }
    requestAnimationFrame(step);
  }

  function initMagicBentoChart() {
    var chart = document.getElementById('mb-chart');
    var plot = document.getElementById('mb-plot');
    var svg = document.getElementById('mb-plot-svg');
    var valEl = document.getElementById('mb-chart-val');
    var noteEl = document.getElementById('mb-chart-note');
    var scrub = document.getElementById('mb-scrub');
    var scrubDot = document.getElementById('mb-scrub-dot');
    if (!chart || !plot || !svg) return;

    var W = 240, H = 68;
    var ys = [56, 51, 53, 42, 44, 33, 36, 25, 28, 17, 19, 9];
    var labels = [
      { v: '6 модов · 0,4 с',  n: 'Старт' },
      { v: '11 модов · 0,8 с', n: '8 потоков' },
      { v: '16 модов · 1,1 с', n: '8 потоков' },
      { v: '22 мода · 1,5 с',  n: '8 потоков' },
      { v: '27 модов · 1,9 с', n: '8 потоков' },
      { v: '32 мода · 2,3 с',  n: 'Распаковка' },
      { v: '37 модов · 2,7 с', n: 'Распаковка' },
      { v: '42 мода · 3,1 с',  n: '8 потоков' },
      { v: '46 модов · 3,5 с', n: '8 потоков' },
      { v: '50 модов · 3,9 с', n: 'Финал' },
      { v: '52 мода · 4,1 с',  n: 'Финал' },
      { v: '54 мода · 4,4 с',  n: 'Готово' }
    ];

    var pts = ys.map(function (y, i) {
      return [(i / (ys.length - 1)) * W, y];
    });
    var d = 'M ' + pts[0][0].toFixed(1) + ' ' + pts[0][1].toFixed(1);
    for (var i = 0; i < pts.length - 1; i++) {
      var p0 = pts[i === 0 ? 0 : i - 1];
      var p1 = pts[i];
      var p2 = pts[i + 1];
      var p3 = pts[i + 2 < pts.length ? i + 2 : i + 1];
      var c1x = p1[0] + (p2[0] - p0[0]) / 6;
      var c1y = p1[1] + (p2[1] - p0[1]) / 6;
      var c2x = p2[0] - (p3[0] - p1[0]) / 6;
      var c2y = p2[1] - (p3[1] - p1[1]) / 6;
      d += ' C ' + c1x.toFixed(1) + ' ' + c1y.toFixed(1) + ', ' + c2x.toFixed(1) + ' ' + c2y.toFixed(1) + ', ' + p2[0].toFixed(1) + ' ' + p2[1].toFixed(1);
    }
    var area = d + ' L ' + W + ' ' + H + ' L 0 ' + H + ' Z';
    var lastPt = pts[pts.length - 1];

    svg.setAttribute('viewBox', '0 0 ' + W + ' ' + H);
    svg.innerHTML =
      '<defs>' +
        '<linearGradient id="mb-chart-grad" x1="0" y1="0" x2="0" y2="1">' +
          '<stop offset="0%" stop-color="rgb(130, 230, 165)" stop-opacity="0.38" />' +
          '<stop offset="100%" stop-color="rgb(130, 230, 165)" stop-opacity="0" />' +
        '</linearGradient>' +
      '</defs>' +
      '<path class="magic-bento__area" d="' + area + '" />' +
      '<path class="magic-bento__curve" d="' + d + '" pathLength="1" />';

    var peak = document.createElement('span');
    peak.className = 'magic-bento__peak';
    peak.style.left = ((lastPt[0] / W) * 100).toFixed(2) + '%';
    peak.style.top = ((lastPt[1] / H) * 100).toFixed(2) + '%';
    plot.appendChild(peak);

    function sampleY(t) {
      var pos = Math.max(0, Math.min(1, t)) * (ys.length - 1);
      var idx = Math.min(ys.length - 2, Math.max(0, Math.floor(pos)));
      var f = pos - idx;
      return ys[idx] + (ys[idx + 1] - ys[idx]) * f;
    }

    var parentCard = chart.closest('.magic-bento__card');
    if (!parentCard) return;

    parentCard.addEventListener('pointermove', function (e) {
      var r = plot.getBoundingClientRect();
      if (!r.width) return;
      var t = Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
      var y = sampleY(t);
      var idx = Math.round(t * (labels.length - 1));
      var item = labels[idx] || labels[labels.length - 1];
      chart.setAttribute('data-scrub', '');
      if (scrub) scrub.style.left = (t * 100).toFixed(2) + '%';
      if (scrubDot) scrubDot.style.top = ((y / H) * 100).toFixed(2) + '%';
      if (valEl) valEl.textContent = item.v;
      if (noteEl) noteEl.textContent = item.n;
    }, { passive: true });

    parentCard.addEventListener('pointerleave', function () {
      chart.removeAttribute('data-scrub');
      if (valEl) valEl.textContent = '54 мода · 4,4 с';
      if (noteEl) noteEl.textContent = '8 потоков';
    });
  }

  /* 7. ReactBits Floating Magnifying Dock */
  function initDock() {
    var dock = document.getElementById('dock');
    if (!dock) return;
    var items = Array.prototype.slice.call(dock.querySelectorAll('.dock__item'));
    var baseSize = 44;
    var maxSize = 62;
    var distLimit = 150;

    dock.addEventListener('pointermove', function (e) {
      if (reduceMotion) return;
      items.forEach(function (item) {
        var r = item.getBoundingClientRect();
        var center = r.left + r.width / 2;
        var dist = Math.abs(e.clientX - center);
        var size = baseSize;
        if (dist < distLimit) {
          var ratio = 1 - dist / distLimit;
          var eased = 0.5 - 0.5 * Math.cos(ratio * Math.PI);
          size = baseSize + (maxSize - baseSize) * eased;
        }
        item.style.setProperty('--dock-size', size.toFixed(1) + 'px');
      });
    }, { passive: true });

    dock.addEventListener('pointerleave', function () {
      items.forEach(function (item) {
        item.style.setProperty('--dock-size', baseSize + 'px');
      });
    });

    // Highlight active section in Dock on scroll
    var sections = items.map(function (item) {
      var href = item.getAttribute('href') || '';
      var target = href.charAt(0) === '#' ? document.querySelector(href) : null;
      return { item: item, target: target };
    }).filter(function (s) { return !!s.target; });

    function updateActiveDock() {
      var y = window.scrollY + window.innerHeight * 0.36;
      var active = sections[0];
      for (var i = 0; i < sections.length; i++) {
        var top = sections[i].target.offsetTop;
        if (y >= top) active = sections[i];
      }
      items.forEach(function (it) {
        it.classList.toggle('is-active', active && it === active.item);
      });
    }
    window.addEventListener('scroll', updateActiveDock, { passive: true });
    updateActiveDock();
  }

  /* Search typer demo */
  function initTyper() {
    var q = document.getElementById('typer');
    var a = document.getElementById('typer-out');
    if (!q || !a || reduceMotion) return;
    var pairs = [
      ['мини карта', 'Minimap'],
      ['шейдеры', 'Iris Shaders'],
      ['зкщьштутсу', 'Prominence'],
      ['оптимизация', 'Sodium · FerriteCore'],
      ['рюкзак', 'Traveler’s Backpack']
    ];
    var i = 0;
    setInterval(function () {
      i = (i + 1) % pairs.length;
      q.textContent = pairs[i][0];
      a.textContent = pairs[i][1];
    }, 2600);
  }

  /* FAQ — Slit Type (21st.dev) in plain JS: slit headline, ink flood,
     single open, ↑↓ Home End, #hash deep links, copy link, search with
     highlighted matches and snippets, topic chips. */
  function initFaq() {
    var root = document.getElementById('faq');
    var list = document.getElementById('faqList');
    if (!root || !list) return;
    var input = document.getElementById('stfQ');
    var clearBtn = document.getElementById('stfClear');
    var countEl = document.getElementById('stfCount');
    var emptyEl = document.getElementById('stfEmpty');
    var emptyQ = document.getElementById('stfEmptyQ');
    var resetBtn = document.getElementById('stfReset');
    var live = document.getElementById('stfLive');
    var chips = Array.prototype.slice.call(document.querySelectorAll('#stfChips .stf-chip'));
    var items = Array.prototype.slice.call(list.querySelectorAll('.stf-item'));
    var reduced = function () {
      return window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    };

    // Headline: each letter its own span, rising through the slit once.
    var title = root.querySelector('.stf-title');
    var n = 0;
    if (title) {
      Array.prototype.forEach.call(title.querySelectorAll('.stf-line'), function (line) {
        var words = line.textContent.trim().split(/\s+/);
        line.textContent = '';
        line.setAttribute('aria-hidden', 'true');
        words.forEach(function (word, wi) {
          var w = document.createElement('span');
          w.className = 'stf-word';
          Array.from(word).forEach(function (ch) {
            var c = document.createElement('span');
            c.className = 'stf-ch';
            c.style.setProperty('--i', n++);
            c.textContent = ch;
            w.appendChild(c);
          });
          line.appendChild(w);
          if (wi < words.length - 1) line.appendChild(document.createTextNode(' '));
        });
      });
      if (!reduced() && 'IntersectionObserver' in window) {
        root.setAttribute('data-intro', 'pending');
        var io = new IntersectionObserver(function (entries) {
          if (entries.some(function (e) { return e.isIntersecting; })) {
            requestAnimationFrame(function () { root.setAttribute('data-intro', 'done'); });
            io.disconnect();
          }
        }, { threshold: 0.15 });
        io.observe(root);
        setTimeout(function () { root.setAttribute('data-intro', 'done'); }, 2500);
      }
    }

    // Per item: the plain question/answer (for search) and the inverted row copy.
    var data = items.map(function (item) {
      var btn = item.querySelector('.stf-btn');
      var row = btn.querySelector('.stf-row');
      var inv = row.cloneNode(true);
      inv.classList.add('stf-row--inv');
      inv.setAttribute('aria-hidden', 'true');
      btn.appendChild(inv);
      var text = item.querySelector('.stf-text');
      return {
        item: item,
        id: item.id,
        btn: btn,
        labels: [row.querySelector('.stf-label'), inv.querySelector('.stf-label')],
        question: row.querySelector('.stf-q').textContent,
        answerHTML: text.innerHTML,
        answer: text.textContent.replace(/\s+/g, ' ').trim(),
        text: text,
        cat: item.getAttribute('data-cat') || ''
      };
    });

    var openId = null;
    var timers = {};
    var query = '';
    var category = '';

    function setState(d, state) {
      d.item.setAttribute('data-state', state);
      d.btn.setAttribute('aria-expanded', state === 'open' ? 'true' : 'false');
    }

    function close(d) {
      clearTimeout(timers[d.id]);
      if (reduced()) { setState(d, 'closed'); return; }
      setState(d, 'closing');
      timers[d.id] = setTimeout(function () {
        if (d.item.getAttribute('data-state') === 'closing') setState(d, 'closed');
      }, 300);
    }

    function open(id, scroll) {
      data.forEach(function (d) {
        if (d.id === id) {
          clearTimeout(timers[d.id]);
          setState(d, 'open');
          if (scroll) {
            setTimeout(function () {
              d.btn.scrollIntoView({ block: 'center', behavior: reduced() ? 'auto' : 'smooth' });
            }, 60);
          }
        } else if (d.item.getAttribute('data-state') === 'open') {
          close(d);
        }
      });
      openId = id;
      render();
    }

    function toggle(d) {
      if (openId === d.id) {
        close(d);
        openId = null;
        render();
      } else {
        open(d.id, false);
      }
    }

    // ---- search
    function words(q) { return q.toLowerCase().split(/\s+/).filter(Boolean); }
    function matches(text, q) {
      var ws = words(q);
      var t = text.toLowerCase();
      return ws.every(function (w) { return t.indexOf(w) > -1; });
    }
    function esc(s) {
      return s.replace(/[&<>"]/g, function (c) { return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]; });
    }
    function marked(text, q) {
      var ws = words(q);
      if (!ws.length) return esc(text);
      var t = text.toLowerCase();
      var hit = new Array(text.length).fill(false);
      ws.forEach(function (w) {
        var at = t.indexOf(w);
        while (at > -1) {
          for (var k = at; k < at + w.length; k++) hit[k] = true;
          at = t.indexOf(w, at + 1);
        }
      });
      var out = '';
      var open = false;
      for (var k = 0; k < text.length; k++) {
        if (hit[k] && !open) { out += '<mark class="stf-mark">'; open = true; }
        if (!hit[k] && open) { out += '</mark>'; open = false; }
        out += esc(text[k]);
      }
      return out + (open ? '</mark>' : '');
    }
    function snippet(text, q) {
      var ws = words(q);
      var t = text.toLowerCase();
      var at = -1;
      ws.forEach(function (w) {
        var i = t.indexOf(w);
        if (i > -1 && (at < 0 || i < at)) at = i;
      });
      if (at < 0) return '';
      var from = Math.max(0, at - 44);
      var to = Math.min(text.length, at + 88);
      if (from > 0) from = text.indexOf(' ', from) + 1 || from;
      if (to < text.length) to = text.lastIndexOf(' ', to) > at ? text.lastIndexOf(' ', to) : to;
      return (from > 0 ? '…' : '') + text.slice(from, to).trim() + (to < text.length ? '…' : '');
    }
    // Marks inside the answer without breaking its <code>/<p>: text nodes only.
    function markAnswer(d, q) {
      d.text.innerHTML = d.answerHTML;
      if (!words(q).length) return;
      var walker = document.createTreeWalker(d.text, NodeFilter.SHOW_TEXT);
      var nodes = [];
      while (walker.nextNode()) nodes.push(walker.currentNode);
      nodes.forEach(function (node) {
        var html = marked(node.nodeValue, q);
        if (html.indexOf('<mark') < 0) return;
        var span = document.createElement('span');
        span.innerHTML = html;
        node.parentNode.replaceChild(span, node);
      });
    }

    function render() {
      var shown = 0;
      var visible = [];
      var counts = {};
      var all = 0;
      data.forEach(function (d) {
        var hay = d.question + ' ' + d.answer;
        var qOk = matches(hay, query);
        if (qOk) {
          all++;
          counts[d.cat] = (counts[d.cat] || 0) + 1;
        }
        var ok = qOk && (!category || d.cat === category);
        d.item.hidden = !ok;
        if (!ok) return;
        shown++;
        visible.push(d);
        var snip = query.trim() && d.item.getAttribute('data-state') !== 'open' ? snippet(d.answer, query) : '';
        var label = '<span class="stf-q">' + marked(d.question, query) + '</span>' +
          (snip ? '<span class="stf-snippet">' + marked(snip, query) + '</span>' : '');
        d.labels.forEach(function (l) { l.innerHTML = label; });
        markAnswer(d, query);
      });
      chips.forEach(function (c) {
        var cat = c.getAttribute('data-cat');
        var small = c.querySelector('small');
        if (small) small.textContent = cat ? (counts[cat] || 0) : all;
        c.setAttribute('aria-pressed', cat === category ? 'true' : 'false');
      });
      var filtering = query.trim() !== '' || category !== '';
      if (countEl) countEl.textContent = filtering ? shown + ' из ' + data.length + ' вопросов' : '';
      if (clearBtn) clearBtn.hidden = !query;
      if (emptyEl) emptyEl.hidden = shown > 0;
      if (emptyQ) emptyQ.textContent = query.trim() || category;
      list.hidden = shown === 0;
      return visible;
    }

    data.forEach(function (d) {
      d.btn.addEventListener('click', function () { toggle(d); });
      d.btn.addEventListener('keydown', function (e) {
        var vis = data.filter(function (x) { return !x.item.hidden; });
        var i = vis.indexOf(d);
        var to = -1;
        if (e.key === 'ArrowDown') to = (i + 1) % vis.length;
        else if (e.key === 'ArrowUp') to = (i - 1 + vis.length) % vis.length;
        else if (e.key === 'Home') to = 0;
        else if (e.key === 'End') to = vis.length - 1;
        if (to < 0) return;
        e.preventDefault();
        vis[to].btn.focus();
      });
      var copy = d.item.querySelector('.stf-copy');
      if (copy) {
        copy.addEventListener('click', function () {
          var url = location.origin + location.pathname + location.search + '#' + d.id;
          var done = function () {
            copy.textContent = 'Ссылка скопирована';
            if (live) live.textContent = 'Ссылка скопирована';
            clearTimeout(timers.__copy);
            timers.__copy = setTimeout(function () {
              copy.textContent = 'Скопировать ссылку';
              if (live) live.textContent = '';
            }, 1800);
          };
          if (navigator.clipboard && window.isSecureContext) {
            navigator.clipboard.writeText(url).then(done, function () { fallbackCopy(url); done(); });
          } else {
            fallbackCopy(url);
            done();
          }
        });
      }
    });

    function fallbackCopy(text) {
      var ta = document.createElement('textarea');
      ta.value = text;
      ta.setAttribute('readonly', '');
      ta.style.position = 'fixed';
      ta.style.opacity = '0';
      document.body.appendChild(ta);
      ta.select();
      try { document.execCommand('copy'); } catch (e) { /* nothing else to try */ }
      ta.remove();
    }

    if (input) {
      input.addEventListener('input', function () { query = input.value; render(); });
      input.addEventListener('keydown', function (e) {
        if (e.key === 'Escape' && input.value) {
          e.preventDefault();
          input.value = '';
          query = '';
          render();
        }
        if (e.key === 'ArrowDown') {
          var first = data.filter(function (x) { return !x.item.hidden; })[0];
          if (first) { e.preventDefault(); first.btn.focus(); }
        }
      });
    }
    if (clearBtn) {
      clearBtn.addEventListener('click', function () {
        input.value = '';
        query = '';
        render();
        input.focus();
      });
    }
    if (resetBtn) {
      resetBtn.addEventListener('click', function () {
        if (input) input.value = '';
        query = '';
        category = '';
        render();
      });
    }
    chips.forEach(function (c) {
      c.addEventListener('click', function () {
        var cat = c.getAttribute('data-cat');
        category = category === cat ? '' : cat;
        render();
      });
    });

    // A #hash naming a question (bento cards link here) opens it.
    function fromHash() {
      var h = decodeURIComponent(location.hash.replace(/^#/, ''));
      if (!h) return;
      var d = data.filter(function (x) { return x.id === h; })[0];
      if (!d) return;
      if (d.item.hidden) { query = ''; category = ''; if (input) input.value = ''; }
      open(h, true);
    }
    window.addEventListener('hashchange', fromHash);
    render();
    fromHash();
  }

  /* Voxel terrain canvas */
  function initTerrain() {
    var canvas = document.getElementById('terrain');
    if (!canvas) return;
    var ctx = canvas.getContext('2d');
    if (!ctx) return;

    var dpr = Math.min(window.devicePixelRatio || 1, 2);
    var w = 0, h = 0, t = 0;
    var cols = 16, rows = 16;
    var visible = true;

    function resize() {
      var rect = canvas.getBoundingClientRect();
      w = rect.width;
      h = rect.height;
      canvas.width = Math.max(1, Math.floor(w * dpr));
      canvas.height = Math.max(1, Math.floor(h * dpr));
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }

    function heightAt(x, y, time) {
      var nx = x * 0.28 + time * 0.55;
      var ny = y * 0.28 + time * 0.4;
      var v = Math.sin(nx) * Math.cos(ny) + Math.sin(nx * 0.6 - ny * 0.7) * 0.6;
      return Math.round((v + 1.6) * 1.7);
    }

    function drawCube(cx, cy, tileW, tileH, cubeH, shade) {
      var topY = cy - cubeH;
      ctx.beginPath();
      ctx.moveTo(cx, topY - tileH);
      ctx.lineTo(cx + tileW, topY);
      ctx.lineTo(cx, topY + tileH);
      ctx.lineTo(cx - tileW, topY);
      ctx.closePath();
      ctx.fillStyle = 'rgba(255,255,255,' + (0.055 * shade).toFixed(3) + ')';
      ctx.fill();
      ctx.strokeStyle = 'rgba(255,255,255,' + (0.12 * shade).toFixed(3) + ')';
      ctx.lineWidth = 0.7;
      ctx.stroke();
    }

    function frame() {
      if (!visible) {
        if (!reduceMotion) requestAnimationFrame(frame);
        return;
      }
      ctx.clearRect(0, 0, w, h);
      var tileW = Math.max(18, Math.min(w, h) * 0.036);
      var tileH = tileW * 0.5;
      var blockStep = tileH * 0.85;
      var originX = w * 0.5;
      var originY = h * 0.50;

      for (var x = 0; x < cols; x++) {
        for (var y = 0; y < rows; y++) {
          var sx = originX + (x - y) * tileW;
          var sy = originY + (x + y) * tileH;
          var lv = heightAt(x, y, t);
          var dist = Math.hypot(x - cols / 2, y - rows / 2) / (cols * 0.72);
          var fade = Math.max(0, 1 - dist * dist);
          if (fade > 0.04) {
            drawCube(sx, sy, tileW, tileH, lv * blockStep, fade);
          }
        }
      }

      if (!reduceMotion) {
        t += 0.011;
        requestAnimationFrame(frame);
      }
    }

    if ('IntersectionObserver' in window) {
      new IntersectionObserver(function (entries) {
        visible = entries[0].isIntersecting;
      }).observe(canvas);
    }

    resize();
    window.addEventListener('resize', resize, { passive: true });
    frame();
  }

  /* GSAP + ScrollTrigger reveals, counters, and magnetic button */
  function initMotion() {
    var hasGSAP = typeof window.gsap !== 'undefined' && typeof window.ScrollTrigger !== 'undefined';

    if (!hasGSAP || reduceMotion) {
      initFallbackIO();
      return;
    }

    var gsap = window.gsap;
    var ScrollTrigger = window.ScrollTrigger;
    gsap.registerPlugin(ScrollTrigger);

    gsap.from('.hero-in', {
      y: 22,
      opacity: 0,
      duration: 0.85,
      stagger: 0.09,
      ease: 'power3.out'
    });

    var frame = document.querySelector('.tilt__frame');
    if (frame) {
      gsap.to(frame, {
        rotateX: 0,
        scale: 1,
        ease: 'none',
        scrollTrigger: {
          trigger: '#inside',
          start: 'top 75%',
          end: 'center center',
          scrub: 0.6
        }
      });
    }

    gsap.utils.toArray('.reveal').forEach(function (el) {
      gsap.from(el, {
        y: 24,
        opacity: 0,
        duration: 0.72,
        ease: 'power3.out',
        scrollTrigger: {
          trigger: el,
          start: 'top 88%',
          once: true
        }
      });
    });

    gsap.utils.toArray('[data-count]').forEach(function (el) {
      var target = parseFloat(el.getAttribute('data-count'));
      var dec = parseInt(el.getAttribute('data-dec') || '0', 10);
      var obj = { v: 0 };
      ScrollTrigger.create({
        trigger: el,
        start: 'top 88%',
        once: true,
        onEnter: function () {
          gsap.to(obj, {
            v: target,
            duration: 1.3,
            ease: 'power2.out',
            onUpdate: function () {
              el.textContent = obj.v.toFixed(dec).replace('.', ',');
            }
          });
        }
      });
    });

    var magnet = document.querySelector('.magnet');
    if (magnet) {
      magnet.addEventListener('mousemove', function (e) {
        var r = magnet.getBoundingClientRect();
        var dx = (e.clientX - (r.left + r.width / 2)) * 0.16;
        var dy = (e.clientY - (r.top + r.height / 2)) * 0.22;
        gsap.to(magnet, { x: dx, y: dy, duration: 0.25, ease: 'power2.out' });
      });
      magnet.addEventListener('mouseleave', function () {
        gsap.to(magnet, { x: 0, y: 0, duration: 0.45, ease: 'elastic.out(1, 0.5)' });
      });
    }
  }

  function initFallbackIO() {
    var f = document.querySelector('.tilt__frame');
    if (f) f.style.transform = 'none';
  }

  window.addEventListener('DOMContentLoaded', function () {
    initOS();
    initCopyButtons();
    initHeroGridSpot();
    initAurora();
    initParticles();
    initBlurText();
    initSpotlightCards();
    initMagicBento();
    initDock();
    initFaq();
    initTyper();
    initTerrain();
    initMotion();
  });
})();
