(async () => {
  const log = (m) => fetch("/api/log", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ t: "kbtest", m }) });
  const errs = [];
  window.addEventListener("error", (e) => errs.push(String(e.message)));
  await new Promise(r => setTimeout(r, 2500));

  // 1. switch to Assignments tab
  const tab = document.querySelector('[data-page="assign"]');
  if (!tab) return log("FAIL: no assignments tab");
  tab.click();
  await new Promise(r => setTimeout(r, 300));

  // 2. find first button row dropdown trigger
  const rows = document.querySelectorAll(".btn-row");
  if (!rows.length) return log("FAIL: no button rows");
  const ddTrigger = rows[0].querySelector(".dd-wrap .dd-trigger");
  if (!ddTrigger) return log("FAIL: no dropdown trigger in row");
  ddTrigger.click();
  await new Promise(r => setTimeout(r, 200));

  // 3. click "Keyboard key…" item
  const items = [...document.querySelectorAll(".dd-item")];
  const keyItem = items.find(i => i.textContent.includes("Keyboard"));
  if (!keyItem) return log("FAIL: no Keyboard key item; items=" + items.map(i => i.textContent).join(","));
  keyItem.click();
  await new Promise(r => setTimeout(r, 400));

  // 4. key-btn should appear; click it
  const keyBtn = document.querySelector(".btn-row .key-btn");
  if (!keyBtn) return log("FAIL: key-btn did not appear; row html=" + rows[0].innerHTML.slice(0, 200));
  keyBtn.click();
  await new Promise(r => setTimeout(r, 300));

  // 5. overlay visible?
  const ov = document.getElementById("kp-overlay");
  if (!ov || ov.classList.contains("hidden")) return log("FAIL: picker overlay not shown");
  log("picker shown: " + document.querySelectorAll(".kp-key").length + " keys");

  // 6. click the 'w' key
  const wKey = [...document.querySelectorAll(".kp-key")].find(k => k.textContent === "w");
  if (!wKey) return log("FAIL: no w key");
  wKey.click();
  await new Promise(r => setTimeout(r, 200));
  log("combo after w: " + document.getElementById("kp-combo").textContent);

  // 7. toggle Ctrl modifier chip
  const ctrl = [...document.querySelectorAll(".kp-mod")].find(m => m.textContent === "Ctrl");
  ctrl.click();
  await new Promise(r => setTimeout(r, 200));
  log("combo after ctrl: " + document.getElementById("kp-combo").textContent);

  // 8. Done
  document.getElementById("kp-ok").click();
  await new Promise(r => setTimeout(r, 400));

  // 9. verify the button's stored value via a fresh apply round-trip read? -> read state from the key-btn label
  const lbl = document.querySelector(".btn-row .key-btn");
  log("final key-btn label: " + (lbl ? lbl.textContent : "MISSING"));
  log("errors: " + (errs.length ? errs.join(" | ") : "none"));
})();
