"use strict";

document.documentElement.classList.add("js");
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

// Tabs: any [role=tablist] with buttons[data-tab] and panels[data-panel].
document.querySelectorAll("[role=tablist]").forEach((list) => {
  const buttons = list.querySelectorAll(".tabs button");
  buttons.forEach((button) => {
    button.addEventListener("click", () => {
      buttons.forEach((b) => b.setAttribute("aria-selected", String(b === button)));
      list.querySelectorAll("[data-panel]").forEach((panel) => {
        panel.hidden = panel.dataset.panel !== button.dataset.tab;
      });
    });
  });
});

// Copy buttons copy the code next to them.
document.querySelectorAll(".copy").forEach((button) => {
  button.addEventListener("click", async () => {
    const text = button.parentElement.querySelector("code").textContent;
    try {
      await navigator.clipboard.writeText(text);
      button.textContent = "Copied";
    } catch {
      button.textContent = "Select and copy";
    }
    setTimeout(() => { button.textContent = "Copy"; }, 1500);
  });
});

// Hero feed: replays the lines one by one, then loops.
const feed = document.getElementById("feed");
if (feed) {
  const lines = [...feed.children];
  if (reducedMotion) {
    lines.forEach((li) => li.classList.add("shown"));
  } else {
    let i = 0;
    const tick = () => {
      if (i === lines.length) {
        setTimeout(() => { lines.forEach((li) => li.classList.remove("shown")); i = 0; setTimeout(tick, 600); }, 3200);
        return;
      }
      lines[i++].classList.add("shown");
      setTimeout(tick, 850);
    };
    setTimeout(tick, 400);
  }
}

// Reveal sections as they scroll into view.
const revealed = document.querySelectorAll(".reveal");
if (reducedMotion || !("IntersectionObserver" in window)) {
  revealed.forEach((el) => el.classList.add("in"));
} else {
  const observer = new IntersectionObserver((entries) => {
    entries.forEach((entry) => {
      if (entry.isIntersecting) {
        entry.target.classList.add("in");
        observer.unobserve(entry.target);
      }
    });
  }, { rootMargin: "0px 0px -8% 0px" });
  revealed.forEach((el) => observer.observe(el));
}
