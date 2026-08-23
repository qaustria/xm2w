/* XM2w - custom UI */

// display order of config slots (config index -> row)
// VERIFIED physical mapping (via hardware tests):
//   1=Right@0x4E  2=Middle@0x55  3=Back@0x5C  4=Forward@0x63
//   5=DPI@0x6A  6=ScrollUp@0x71  7=ScrollDown@0x78
//   0=Left@0x7F (outside the writable delta — locked to Left click)
// codes: 0=Left 2=Right 4=Middle 8=Back 0x10=Forward
const BUTTON_ORDER = [0, 1, 2, 4, 3, 5, 6, 7]; // Left, Right, Middle, Forward, Back, DPI, Scroll Up, Scroll Down
const BUTTON_NAMES = { 0: "Left", 1: "Right", 2: "Middle", 3: "Back", 4: "Forward", 5: "DPI", 6: "Scroll Up", 7: "Scroll Down" };
const VIEWS = { 0: "topdown", 1: "topdown", 2: "topdown", 3: "side", 4: "side", 5: "persp", 6: "topdown", 7: "topdown" };
const LEFT_LOCKED = true; // slot 0 (Left) is outside the writable config delta

const HID = {
  a:0x04,b:0x05,c:0x06,d:0x07,e:0x08,f:0x09,g:0x0a,h:0x0b,i:0x0c,j:0x0d,k:0x0e,l:0x0f,
  m:0x10,n:0x11,o:0x12,p:0x13,q:0x14,r:0x15,s:0x16,t:0x17,u:0x18,v:0x19,w:0x1a,x:0x1b,
  y:0x1c,z:0x1d,"1":0x1e,"2":0x1f,"3":0x20,"4":0x21,"5":0x22,"6":0x23,"7":0x24,"8":0x25,
  "9":0x26,"0":0x27,enter:0x28,esc:0x29,backspace:0x2a,tab:0x2b,space:0x2c,
  "-":0x2d,"=":0x2e,"[":0x2f,"]":0x30,"\\":0x31,";":0x33,"'":0x34,"`":0x35,",":0x36,
  ".":0x37,"/":0x38,capslock:0x39,
  f1:0x3a,f2:0x3b,f3:0x3c,f4:0x3d,f5:0x3e,f6:0x3f,f7:0x40,f8:0x41,f9:0x42,f10:0x43,
  f11:0x44,f12:0x45,home:0x4a,pageup:0x4b,delete:0x4c,end:0x4d,pagedown:0x4e,
  right:0x4f,left:0x50,down:0x51,up:0x52,
};

const MOD_NAMES = { 1: "Ctrl", 2: "Shift", 4: "Alt", 8: "Win" };

// keyboard rows: {k: key-id (HID map key), w: flex width}
const KB_ROWS = [
  [{ k: "esc" }, { k: "f1" }, { k: "f2" }, { k: "f3" }, { k: "f4" }, { k: "f5" }, { k: "f6" },
   { k: "f7" }, { k: "f8" }, { k: "f9" }, { k: "f10" }, { k: "f11" }, { k: "f12" }],
  [{ k: "`" }, { k: "1" }, { k: "2" }, { k: "3" }, { k: "4" }, { k: "5" }, { k: "6" },
   { k: "7" }, { k: "8" }, { k: "9" }, { k: "0" }, { k: "-" }, { k: "=" }, { k: "backspace", w: 1.8 }],
  [{ k: "tab", w: 1.4 }, { k: "q" }, { k: "w" }, { k: "e" }, { k: "r" }, { k: "t" }, { k: "y" },
   { k: "u" }, { k: "i" }, { k: "o" }, { k: "p" }, { k: "[" }, { k: "]" }, { k: "\\", w: 1.4 }],
  [{ k: "capslock", w: 1.7 }, { k: "a" }, { k: "s" }, { k: "d" }, { k: "f" }, { k: "g" }, { k: "h" },
   { k: "j" }, { k: "k" }, { k: "l" }, { k: ";" }, { k: "'" }, { k: "enter", w: 2 }],
  [{ k: "shift", w: 2.2, mod: 2 }, { k: "z" }, { k: "x" }, { k: "c" }, { k: "v" }, { k: "b" },
   { k: "n" }, { k: "m" }, { k: "," }, { k: "." }, { k: "/" }, { k: "shift", w: 2.6, mod: 2 }],
  [{ k: "ctrl", w: 1.3, mod: 1 }, { k: "win", w: 1.3, mod: 8 }, { k: "alt", w: 1.3, mod: 4 },
   { k: "space", w: 5.5 }, { k: "alt", w: 1.3, mod: 4 }, { k: "win", w: 1.3, mod: 8 },
   { k: "ctrl", w: 1.3, mod: 1 }],
  [{ k: "home" }, { k: "pageup" }, { k: "delete" }, { k: "end" }, { k: "pagedown" },
   { k: "arrowleft" }, { k: "arrowdown" }, { k: "arrowup" }, { k: "arrowright" }],
];

const state = { settings: null, dirty: false, emu: false };
const $ = (id) => document.getElementById(id);

function invoke(cmd, args) {
  return fetch("/api/" + cmd, {
    method: cmd === "info" ? "GET" : "POST",
    headers: { "Content-Type": "application/json" },
    body: cmd === "info" ? undefined : JSON.stringify(args || {}),
  }).then((r) => r.json()).then((j) => {
    if (j.error) throw new Error(j.error);
    return j;
  });
}
const log = (m) => { console.log(m); fetch("/api/log", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ t: "app", m: String(m) }) }).catch(() => {}); };

function toast(msg, kind) {
  let t = document.getElementById("toast");
  if (!t) {
    t = document.createElement("div");
    t.id = "toast";
    t.style.cssText = "position:fixed;bottom:70px;left:50%;transform:translateX(-50%);z-index:300;padding:9px 18px;border-radius:8px;font:600 13px/1 var(--font);color:#fff;background:#2a2a30;border:1px solid #3a3a42;box-shadow:0 10px 30px rgba(0,0,0,.5);opacity:0;transition:opacity .15s;pointer-events:none;";
    document.body.appendChild(t);
  }
  t.textContent = msg;
  t.style.borderColor = kind === "err" ? "#ff5a5a" : "#3ddc84";
  t.style.opacity = "1";
  clearTimeout(t._timer);
  t._timer = setTimeout(() => { t.style.opacity = "0"; }, 2200);
}

function clampCpi(v) {
  if (!v || v < 50) return 50;
  if (v > 26000) return 26000;
  return Math.round(v / 50) * 50;
}

// ---------------- product view ----------------
function setView(name) {
  const img = $("product");
  if (img.dataset.view === name) return;
  img.style.opacity = 0;
  setTimeout(() => {
    img.src = "imgs/" + name + ".png";
    img.dataset.view = name;
    img.style.opacity = 1;
  }, 100);
}

// ---------------- custom dropdown ----------------
let openDD = null;
document.addEventListener("click", () => {
  if (openDD) { openDD.classList.remove("open"); openDD = null; }
});

function makeDropdown(container, options, current, onChange, locked) {
  const dd = document.createElement("div");
  dd.className = "dd" + (locked ? " locked" : "");
  const trigger = document.createElement("button");
  trigger.className = "dd-trigger";
  trigger.disabled = !!locked;
  const label = document.createElement("span");
  const chev = document.createElement("span");
  chev.className = "dd-chevron";
  trigger.append(label, chev);
  const menu = document.createElement("div");
  menu.className = "dd-menu";
  dd.append(trigger, menu);

  function render() {
    label.textContent = options.find((o) => o.value === current)?.label ?? current;
    menu.innerHTML = "";
    options.forEach((o) => {
      const item = document.createElement("button");
      item.className = "dd-item" + (o.value === current ? " selected" : "");
      item.textContent = o.label;
      item.addEventListener("click", (e) => {
        e.stopPropagation();
        current = o.value;
        dd.classList.remove("open");
        render();
        onChange(o.value);
      });
      menu.appendChild(item);
    });
  }
  trigger.addEventListener("click", (e) => {
    e.stopPropagation();
    const wasOpen = dd.classList.contains("open");
    if (openDD) openDD.classList.remove("open");
    openDD = wasOpen ? null : dd;
    dd.classList.toggle("open", !wasOpen);
  });
  render();
  container.appendChild(dd);
  return { get: () => current, set: (v) => { current = v; render(); } };
}

// ---------------- editable DPI value ----------------
function makeEditableVal(el, c, refresh) {
  // el = the .v span; split = edit X and Y side by side
  const commit = (v) => {
    c.x = clampCpi(v);
    if (!c.split) c.y = c.x;
    setDirty();
    refresh();
  };
  const commitXY = (x, y) => {
    c.x = clampCpi(x);
    c.y = clampCpi(y);
    setDirty();
    refresh();
  };
  el.addEventListener("dblclick", (e) => {
    e.stopPropagation();
    log("dblclick on DPI value, c.x=" + c.x + " split=" + c.split);
    const wrap = document.createElement("div");
    wrap.className = "level-editwrap";
    const mk = (initial, onDone) => {
      const inp = document.createElement("input");
      inp.type = "text";
      inp.inputMode = "numeric";
      inp.value = initial;
      inp.className = "level-edit";
      log("edit input created value=" + inp.value);
      let finished = false;
      const fin = () => {
        if (finished) return;
        finished = true;
        onDone(clampCpi(+inp.value));
      };
      const cancel = () => {
        if (finished) return;
        finished = true;
        wrap.replaceWith(el);
      };
      inp.addEventListener("keydown", (ev) => {
        ev.stopPropagation();
        if (ev.key === "Enter") fin();
        if (ev.key === "Escape") cancel();
      });
      inp.addEventListener("blur", fin);
      inp.addEventListener("click", (ev) => ev.stopPropagation());
      return inp;
    };
    if (c.split) {
      const ix = mk(c.x, (x) => { commitXY(x, c.y); });
      const sep = document.createElement("span");
      sep.className = "level-editsep";
      sep.textContent = "/";
      const iy = mk(c.y, (y) => { commitXY(c.x, y); });
      wrap.append(ix, sep, iy);
    } else {
      wrap.appendChild(mk(c.x, commit));
    }
    el.replaceWith(wrap);
    const first = wrap.querySelector("input");
    // re-apply value explicitly (some WebKit builds need it after insertion)
    if (c.split) {
      const ins = wrap.querySelectorAll("input");
      ins[0].value = c.x;
      ins[1].value = c.y;
    } else {
      first.value = c.x;
    }
    log("edit input shown, first value=" + first.value);
    first.focus();
    first.select();
  });
}

// ---------------- render: DPI levels ----------------
function renderLevels() {
  const list = $("level-list");
  list.innerHTML = "";
  state.settings.cpis.forEach((c, i) => {
    const card = document.createElement("div");
    card.className = "level-card";
    const active = i < state.settings.cpi_levels;
    const colors = ["#38bdf8", "#ff5a5a", "#5a7dff", "#3ddc84"];

    const head = document.createElement("div");
    head.className = "level-head";
    head.innerHTML = `
      <span class="level-dot" style="--dot:${colors[i]}"></span>
      <span class="level-name">Level ${i + 1}</span>
      <span class="level-val"><span class="v">${c.x}</span><span class="u">DPI</span></span>
    `;
    const valEl = head.querySelector(".level-val .v");
    const refresh = () => { renderAxes(); valEl.textContent = c.split ? `${c.x} / ${c.y}` : c.x; };
    makeEditableVal(valEl, c, refresh);
    bindTip(valEl, `Level ${i + 1}`,
      "Drag the slider, or double-click the number to type a value. 50–26000 DPI in steps of 50.");

    function makeSlider(axisLabel, value, onInput) {
      const wrap = document.createElement("div");
      wrap.className = "level-axis";
      const lab = document.createElement("span");
      lab.className = "axis-label";
      lab.textContent = axisLabel;
      const slider = document.createElement("input");
      slider.type = "range";
      slider.className = "level-slider";
      slider.min = 50; slider.max = 26000; slider.step = 50;
      slider.value = value;
      function paint() {
        const pct = ((slider.value - 50) / (26000 - 50)) * 100;
        slider.style.background =
          `linear-gradient(to right, var(--accent) ${pct}%, var(--border-2) ${pct}%)`;
      }
      slider.addEventListener("input", () => { onInput(+slider.value); paint(); });
      wrap.append(lab, slider);
      paint();
      return wrap;
    }

    const axisBox = document.createElement("div");
    axisBox.className = "level-axes";
    let xAxis = null;
    let yAxis = null;

    function renderAxes() {
      axisBox.innerHTML = "";
      xAxis = makeSlider("X", c.x, (v) => {
        c.x = v;
        if (!c.split) c.y = v;
        valEl.textContent = c.split ? `${c.x} / ${c.y}` : c.x;
        if (!c.split && yAxis) yAxis.value = v;
        setDirty();
      });
      axisBox.appendChild(xAxis);
      if (c.split) {
        yAxis = makeSlider("Y", c.y, (v) => {
          c.y = v;
          valEl.textContent = `${c.x} / ${c.y}`;
          setDirty();
        });
        axisBox.appendChild(yAxis);
      } else {
        yAxis = null;
      }
      valEl.textContent = c.split ? `${c.x} / ${c.y}` : c.x;
    }

    const foot = document.createElement("div");
    foot.className = "level-foot";
    const split = document.createElement("button");
    split.className = "split-btn" + (c.split ? " on" : "");
    split.textContent = "X/Y";
    foot.appendChild(split);
    if (!active) {
      const off = document.createElement("span");
      off.className = "off-label";
      off.textContent = "inactive";
      foot.appendChild(off);
    }

    bindTip(axisBox, `Level ${i + 1}`,
      `Sensitivity of this level. Press the DPI button to switch to it. The dot color matches the indicator LED.`);
    bindTip(split, "Split X/Y",
      "Use separate horizontal (X) and vertical (Y) sensitivity instead of one shared value.");
    split.addEventListener("click", () => {
      c.split = !c.split;
      split.classList.toggle("on", c.split);
      renderAxes();
      setDirty();
    });

    renderAxes();
    card.append(head, axisBox, foot);
    list.appendChild(card);
  });
}

// ---------------- render: toggles ----------------
function renderToggles() {
  const items = [
    ["slamclick", "Slamclick filter"],
    ["jitter", "Jitter filter"],
    ["angle_snapping", "Angle snapping"],
    ["ripple", "Ripple control"],
    ["motion_sync", "Motion sync"],
  ];
  const list = $("toggle-list");
  list.innerHTML = "";
  const TIP = {
    slamclick: ["Slamclick filter", "Blocks accidental clicks caused by slamming the mouse down hard on the desk."],
    jitter: ["Jitter filter", "Reduces sensor jitter, most noticeable at very high DPI."],
    angle_snapping: ["Angle snapping", "Straightens small deviations so horizontal and vertical mouse movement stays on a line."],
    ripple: ["Ripple control", "Smooths out sensor ripple noise during slow movement."],
    motion_sync: ["Motion sync", "Syncs sensor data to the USB polling interval for more consistent, even motion."],
  };
  items.forEach(([key, label]) => {
    const t = document.createElement("div");
    t.className = "toggle" + (state.settings[key] ? " on" : "");
    bindTip(t, TIP[key][0], TIP[key][1]);
    t.innerHTML = `<span>${label}</span><span class="switch"></span>`;
    t.addEventListener("click", () => {
      state.settings[key] = !state.settings[key];
      t.classList.toggle("on", state.settings[key]);
      setDirty();
    });
    list.appendChild(t);
  });
}

// ---------------- render: buttons ----------------
// device stores usage-1 (firmware table is shifted +1)
const usageToDev = (u) => (u - 1) & 0xff;
const devToUsage = (d) => (d + 1) & 0xff;

function hidName(devCode) {
  const usage = devToUsage(devCode);
  const hit = Object.entries(HID).find(([, c]) => c === usage);
  return hit ? hit[0] : `0x${usage.toString(16)}`;
}
function parseKey(s) {
  s = s.trim().toLowerCase();
  if (s.startsWith("0x")) {
    const v = parseInt(s, 16);
    return isNaN(v) ? null : v & 0xff;
  }
  return HID[s] ?? null;
}

const BINDS = [
  { value: "mouse-left", label: "Left click" },
  { value: "mouse-right", label: "Right click" },
  { value: "mouse-middle", label: "Middle click" },
  { value: "mouse-back", label: "Back" },
  { value: "mouse-forward", label: "Forward click" },
  { value: "cpi", label: "CPI cycle" },
  { value: "scroll-up", label: "Scroll up" },
  { value: "scroll-down", label: "Scroll down" },
  { value: "key", label: "Keyboard key…" },
  { value: "disabled", label: "Disabled" },
];

function kindOf(b) {
  switch (b.kind) {
    case 0x00: return { 0x00: "mouse-left", 0x02: "mouse-right", 0x04: "mouse-middle", 0x08: "mouse-back", 0x10: "mouse-forward" }[b.value[0]] ?? "mouse-left";
    case 0x02: return "key";
    case 0x01: return b.value[0] === 1 ? "scroll-up" : "scroll-down";
    case 0x09: return "cpi";
    default: return "disabled";
  }
}

function applyBind(b, kind) {
  switch (kind) {
    case "mouse-left": b.kind = 0x00; b.value = [0x00, 0, 0, 0, 0]; break;
    case "mouse-right": b.kind = 0x00; b.value = [0x02, 0, 0, 0, 0]; break;
    case "mouse-middle": b.kind = 0x00; b.value = [0x04, 0, 0, 0, 0]; break;
    case "mouse-back": b.kind = 0x00; b.value = [0x08, 0, 0, 0, 0]; break;
    case "mouse-forward": b.kind = 0x00; b.value = [0x10, 0, 0, 0, 0]; break;
    case "cpi": b.kind = 0x09; b.value = [0xf1, 0, 0, 0, 0]; break;
    case "scroll-up": b.kind = 0x01; b.value = [0x01, 0, 0, 0, 0]; break;
    case "scroll-down": b.kind = 0x01; b.value = [0xff, 0, 0, 0, 0]; break;
    case "key":
      b.kind = 0x02;
      if (!b.value[1]) b.value = [0, usageToDev(0x1a), 0, 0, 0];
      break;
    case "disabled": b.kind = 0xff; b.value = [0, 0, 0, 0, 0]; break;
  }
}

function simPress(slot) {
  invoke("emu/press", { slot })
    .then((r) => {
      if (r && r.action) toast(BUTTON_NAMES[slot] + " \u2192 " + r.action);
    })
    .catch((e) => toast("simulate failed: " + e, "err"));
}

function renderButtons() {
  const list = $("button-list");
  list.innerHTML = "";
  BUTTON_ORDER.forEach((i) => {
    const b = state.settings.buttons[i];
    const row = document.createElement("div");
    row.className = "btn-row";
    const name = document.createElement("span");
    name.className = "btn-name";
    name.textContent = BUTTON_NAMES[i];

    const ddWrap = document.createElement("div");
    ddWrap.className = "dd-wrap";
    const cur = kindOf(b);
    const locked = LEFT_LOCKED && i === 0;
    const dd = makeDropdown(ddWrap, BINDS, cur, (v) => {
      applyBind(b, v);
      setDirty();
      if (v === "key") renderButtons();
    }, locked);

    row.append(name, ddWrap);
    if (cur === "key") {
      const btn = document.createElement("button");
      btn.className = "key-btn";
      btn.textContent = kpLabel(b.value[0], devToUsage(b.value[1]));
      btn.addEventListener("click", () => kpOpen(i, b));
      row.appendChild(btn);
    }

    const tipText = (LEFT_LOCKED && i === 0)
      ? "The left button is fixed by the mouse hardware and can't be remapped."
      : "What this physical button does right now. Click the dropdown to change it. Hovering highlights the button on the mouse.";
    bindTip(row, BUTTON_NAMES[i], tipText);
    row.addEventListener("mouseenter", () => { row.classList.add("hovered"); setView(VIEWS[i]); });
    row.addEventListener("mouseleave", () => row.classList.remove("hovered"));
    if (state.emu) {
      const sim = document.createElement("button");
      sim.className = "sim-btn";
      sim.textContent = "\u25b6";
      sim.title = "Simulate pressing this button (emulator)";
      sim.addEventListener("click", () => simPress(i));
      row.appendChild(sim);
    }
    list.appendChild(row);
  });
}

// ---------------- key picker ----------------
const kp = {
  open: false,
  index: -1,
  mods: 0,
  code: 0,
  capturing: false,
};

function usageName(u) {
  const hit = Object.entries(HID).find(([, c]) => c === u);
  return hit ? hit[0] : `0x${u.toString(16)}`;
}

function kpLabel(mods, usage) {
  const parts = [];
  for (const [bit, name] of Object.entries(MOD_NAMES)) {
    if (mods & +bit) parts.push(name);
  }
  const keyName = usageName(usage);
  if (keyName && !["ctrl", "shift", "alt", "win"].includes(keyName)) parts.push(keyName);
  return parts.join(" + ") || "—";
}

function kpRender() {
  $("kp-combo").textContent = kpLabel(kp.mods, kp.code);

  // modifier chips
  const modsEl = $("kp-mods");
  modsEl.innerHTML = "";
  for (const [bit, name] of Object.entries(MOD_NAMES)) {
    const b = document.createElement("button");
    b.className = "kp-mod" + (kp.mods & +bit ? " on" : "");
    b.textContent = name;
    b.addEventListener("click", () => {
      kp.mods ^= +bit;
      kpRender();
    });
    modsEl.appendChild(b);
  }

  // keyboard
  const keysEl = $("kp-keys");
  keysEl.innerHTML = "";
  KB_ROWS.forEach((row) => {
    const r = document.createElement("div");
    r.className = "kp-row";
    row.forEach(({ k, w, mod }) => {
      const b = document.createElement("button");
      b.className = "kp-key" + (w ? " wide" : "") + (mod ? " mod" : "");
      b.style.flexGrow = w || 1;
      b.style.flexBasis = "0";
      b.textContent = k === "space" ? "Space" : k.length === 1 ? k : k;
      const isMod = !!mod;
      const sel = isMod ? !!(kp.mods & mod) : kp.code === HID[k];
      if (sel) b.classList.add("sel");
      b.addEventListener("click", () => {
        if (isMod) {
          kp.mods ^= mod;
        } else {
          kp.code = HID[k];
        }
        kpRender();
      });
      r.appendChild(b);
    });
    keysEl.appendChild(r);
  });
}

function kpOpen(index, b) {
  kp.open = true;
  kp.index = index;
  kp.mods = b.value[0] || 0;
  kp.code = b.value[1] || 0;
  $("kp-overlay").classList.remove("hidden");
  kpRender();
}

function kpClose() {
  kp.open = false;
  kp.capturing = false;
  $("kp-capture-btn").textContent = "…or press a key";
  $("kp-overlay").classList.add("hidden");
}

// ---------------- load / apply ----------------
function load() {
  invoke("info")
    .then((r) => {
      state.settings = r.settings;
      state.emu = !!r.emu;
      const badge = document.getElementById("emu-badge");
      if (badge) badge.classList.toggle("hidden", !state.emu);
      state.dirty = false;
      const btn = document.getElementById("btn-apply");
      if (btn) btn.classList.remove("dirty");
      renderLevels();
      renderToggles();
      renderButtons();
      setView("topdown");
    })
    .catch((e) => { console.error(e); log("load FAILED: " + e); toast("Can't reach the mouse", "err"); });
}

function setDirty() {
  setDirty();
  const btn = document.getElementById("btn-apply");
  if (btn) btn.classList.add("dirty");
}

function apply() {
  log("apply buttons[3]=" + JSON.stringify(state.settings.buttons[3]));
  invoke("apply", state.settings)
    .then((fresh) => {
      state.settings = fresh.settings || fresh;
      state.dirty = false;
      const btn = document.getElementById("btn-apply");
      if (btn) btn.classList.remove("dirty");
      renderLevels();
      renderToggles();
      renderButtons();
      toast("Applied");
    })
    .catch((e) => { console.error(e); toast("Apply failed: " + e, "err"); });
}

document.addEventListener("DOMContentLoaded", () => {
  log("UI LOADED v" + Date.now() + " keypicker=" + !!document.getElementById("kp-keys") + " footer=" + !!document.getElementById("btn-github"));
  // static tooltips
  document.querySelectorAll("[data-tip]").forEach((el) => {
    bindTip(el, el.dataset.title, el.dataset.tip);
  });
  // tabs
  document.querySelectorAll(".tab").forEach((t) => {
    t.addEventListener("click", () => {
      document.querySelectorAll(".tab").forEach((x) => x.classList.remove("active"));
      t.classList.add("active");
      document.querySelectorAll(".page").forEach((p) => p.classList.add("hidden"));
      $("page-" + t.dataset.page).classList.remove("hidden");
    });
  });
  if (location.hash === "#assign") {
    const t = document.querySelector('.tab[data-page="assign"]');
    if (t) t.click();
  }

  // view switching on hover
  $("group-levels").addEventListener("mouseenter", () => setView("persp"));
  $("group-levels").addEventListener("mouseleave", () => setView("topdown"));
  $("group-sensor").addEventListener("mouseenter", () => setView("topdown"));

  // polling + lod dropdowns
  makeDropdown($("rate-dd"), [
    { value: 1000, label: "1000 Hz" },
    { value: 2000, label: "2000 Hz" },
    { value: 4000, label: "4000 Hz" },
  ], 4000, (v) => { state.settings.polling_hz = v; setDirty(); });
  bindTip($("rate-dd").querySelector(".dd-trigger"), "Polling rate",
    "How often the mouse sends position updates to the computer. 4000 Hz is smoothest and uses the most battery.");
  makeDropdown($("lod-dd"), [
    { value: 0, label: "0 mm" },
    { value: 1, label: "1 mm" },
    { value: 2, label: "2 mm" },
  ], 1, (v) => { state.settings.lod = v; setDirty(); });
  bindTip($("lod-dd").querySelector(".dd-trigger"), "Lift-off distance",
    "How high the mouse must be lifted before the sensor stops tracking. Lower is better for lifting between swipes.");

  $("btn-apply").addEventListener("click", apply);
  $("btn-github").addEventListener("click", () => {
    window.open("https://github.com/qaustria/xm2w", "_blank");
  });
  $("kp-close").addEventListener("click", kpClose);
  $("kp-cancel").addEventListener("click", kpClose);
  $("kp-overlay").addEventListener("click", (e) => { if (e.target.id === "kp-overlay") kpClose(); });
  $("kp-ok").addEventListener("click", () => {
    const b = state.settings.buttons[kp.index];
    log("kp-ok: index=" + kp.index + " mods=" + kp.mods + " usage=" + kp.code);
    b.value = [kp.mods & 0xff, usageToDev(kp.code), 0, 0, 0];
    setDirty();
    kpClose();
    renderButtons();
  });
  // press-a-key capture
  $("kp-capture-btn").addEventListener("click", () => {
    kp.capturing = true;
    $("kp-capture-btn").textContent = "Press any key…";
  });
  window.addEventListener("keydown", (e) => {
    if (!kp.open || !kp.capturing) return;
    e.preventDefault();
    const key = e.key.toLowerCase();
    const code = HID[key] ?? HID[e.code.toLowerCase()] ?? HID[e.code.replace("Key", "").toLowerCase()];
    if (code != null) {
      kp.code = code;
      kp.mods = (e.ctrlKey ? 1 : 0) | (e.shiftKey ? 2 : 0) | (e.altKey ? 4 : 0) | (e.metaKey ? 8 : 0);
      kp.capturing = false;
      $("kp-capture-btn").textContent = "…or press a key";
      kpRender();
    }
  });
  load();
});
