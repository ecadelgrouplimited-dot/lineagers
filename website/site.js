"use strict";

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
