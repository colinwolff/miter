"use strict";

/**
 * Shows one demo panel and marks its tab selected.
 * @param {string} name - the panel to reveal, e.g. "skeleton".
 */
function selectDemoTab(name) {
  document.querySelectorAll(".demo-tab").forEach((tab) => {
    const isActive = tab.dataset.demo === name;
    tab.classList.toggle("is-active", isActive);
    tab.setAttribute("aria-selected", String(isActive));
  });

  document.querySelectorAll(".panel").forEach((panel) => {
    panel.classList.toggle("is-active", panel.id === "panel-" + name);
  });
}

/**
 * Swaps the command shown in the install copy box.
 * @param {HTMLElement} tab - the install tab that was clicked.
 */
function selectInstallTab(tab) {
  const command = tab.dataset.command;
  if (!command) {
    return;
  }

  document.querySelectorAll(".install .tab").forEach((other) => {
    other.classList.toggle("is-active", other === tab);
  });

  const box = document.getElementById("install-command");
  if (box) {
    box.textContent = command;
  }

  const copy = document.querySelector('.copy[data-copy-target="install-command"]');
  if (copy) {
    copy.dataset.copyText = command;
  }
}

/**
 * Flashes "Copied!" on a button for a moment, then restores its label.
 * @param {HTMLElement} button - the button that was pressed.
 */
function flashCopied(button) {
  const original = button.dataset.label || button.textContent;
  button.dataset.label = original;
  button.textContent = "Copied!";
  button.classList.add("is-copied");
  window.setTimeout(() => {
    button.textContent = original;
    button.classList.remove("is-copied");
  }, 1400);
}

/**
 * Copies the text of the element named by the button's data-copy-target,
 * falling back to the clipboard API's own rejection when it is unavailable.
 * @param {HTMLElement} button - the button carrying data-copy-target.
 */
async function copyCommand(button) {
  const target = document.getElementById(button.dataset.copyTarget);
  const text = button.dataset.copyText || (target ? target.textContent.trim() : "");
  if (!text) {
    return;
  }

  try {
    if (navigator.clipboard) {
      await navigator.clipboard.writeText(text);
    } else {
      const scratch = document.createElement("textarea");
      scratch.value = text;
      document.body.appendChild(scratch);
      scratch.select();
      document.execCommand("copy");
      scratch.remove();
    }
    flashCopied(button);
  } catch (error) {
    button.textContent = "Press Ctrl+C";
    window.setTimeout(() => {
      button.textContent = button.dataset.label || "Copy";
    }, 1400);
    console.warn("miter docs: clipboard write failed", error);
  }
}

/**
 * Wires the install tabs, demo tabs and copy buttons of the landing page.
 */
function initMiterDocs() {
  document.querySelectorAll(".install .tab[data-command]").forEach((tab) => {
    tab.addEventListener("click", () => selectInstallTab(tab));
  });

  document.querySelectorAll(".demo-tab").forEach((tab) => {
    tab.addEventListener("click", () => selectDemoTab(tab.dataset.demo));
  });

  document.querySelectorAll(".copy").forEach((button) => {
    button.addEventListener("click", () => copyCommand(button));
  });
}

document.addEventListener("DOMContentLoaded", initMiterDocs);
