(async () => {
  const log = (m) => fetch("/api/log", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ t: "applytest", m }) });
  const errs = [];
  window.addEventListener("error", (e) => errs.push(String(e.message)));
  await new Promise(r => setTimeout(r, 2500));
  // change level 1 to 300 then click Apply
  const sliders = document.querySelectorAll(".level-slider");
  if (!sliders.length) return log("FAIL: no sliders");
  const ev = new Event("input", { bubbles: true });
  sliders[0].value = 300;
  sliders[0].dispatchEvent(ev);
  await new Promise(r => setTimeout(r, 300));
  document.getElementById("btn-apply").click();
  await new Promise(r => setTimeout(r, 2500));
  const t = document.getElementById("toast");
  log("toast: " + (t ? t.textContent : "none"));
  log("errors: " + (errs.length ? errs.join(" | ") : "none"));
})();
