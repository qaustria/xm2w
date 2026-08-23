/* Tooltip helper */
const tipEl = (() => {
  const el = document.createElement("div");
  el.className = "tip";
  document.body.appendChild(el);
  return el;
})();

function showTip(x, y, title, text) {
  tipEl.innerHTML = `<span class="t">${title}</span>${text}`;
  tipEl.classList.add("show");
  const r = tipEl.getBoundingClientRect();
  let tx = x + 14;
  let ty = y + 16;
  if (tx + r.width > window.innerWidth - 8) tx = x - r.width - 14;
  if (ty + r.height > window.innerHeight - 8) ty = y - r.height - 16;
  tipEl.style.left = tx + "px";
  tipEl.style.top = ty + "px";
}

function hideTip() {
  tipEl.classList.remove("show");
}

function bindTip(el, title, text) {
  el.addEventListener("mouseenter", (e) => showTip(e.clientX, e.clientY, title, text));
  el.addEventListener("mousemove", (e) => showTip(e.clientX, e.clientY, title, text));
  el.addEventListener("mouseleave", hideTip);
}
