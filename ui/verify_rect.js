// injected verification: report layout rects via /api/log
setTimeout(() => {
  const report = {};
  for (const id of ["btn-github", "btn-apply", "product", "kp-keys"]) {
    const el = document.getElementById(id);
    if (el) {
      const r = el.getBoundingClientRect();
      report[id] = { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) };
    } else report[id] = null;
  }
  const fb = document.querySelector(".footbar");
  if (fb) {
    const r = fb.getBoundingClientRect();
    const logo = document.querySelector(".egg-logo");
    const lr = logo ? logo.getBoundingClientRect() : null;
    const vis = getComputedStyle(fb);
    report.footbar = { y: Math.round(r.y), h: Math.round(r.height), display: vis.display, visibility: vis.visibility, logo: lr ? { w: Math.round(lr.width), h: Math.round(lr.height) } : null };
  }
  report.viewport = { w: window.innerWidth, h: window.innerHeight };
  fetch("/api/log", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(report) });
}, 1500);
