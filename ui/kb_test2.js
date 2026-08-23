(async () => {
  const log = (m) => fetch("/api/log", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ t: "kb2", m }) });
  const errs = [];
  window.addEventListener("error", (e) => errs.push(String(e.message)));
  await new Promise(r => setTimeout(r, 2500));
  document.querySelector('[data-page="assign"]').click();
  await new Promise(r => setTimeout(r, 300));
  const rows = document.querySelectorAll(".btn-row");
  // row 3 = Back (index 3)
  const dd = rows[3].querySelector(".dd-trigger");
  dd.click();
  await new Promise(r => setTimeout(r, 200));
  [...document.querySelectorAll(".dd-item")].find(i => i.textContent.includes("Keyboard")).click();
  await new Promise(r => setTimeout(r, 400));
  document.querySelector(".btn-row .key-btn").click();
  await new Promise(r => setTimeout(r, 300));
  [...document.querySelectorAll(".kp-key")].find(k => k.textContent === "w").click();
  [...document.querySelectorAll(".kp-mod")].find(m => m.textContent === "Ctrl").click();
  log("combo: " + document.getElementById("kp-combo").textContent);
  document.getElementById("kp-ok").click();
  await new Promise(r => setTimeout(r, 400));
  log("key-btn: " + document.querySelectorAll(".btn-row")[3].querySelector(".key-btn").textContent);
  // apply and read back device state
  document.getElementById("btn-apply").click();
  await new Promise(r => setTimeout(r, 2500));
  log("toast: " + (document.getElementById("toast") ? document.getElementById("toast").textContent : "none"));
  log("errors: " + (errs.length ? errs.join(" | ") : "none"));
})();
