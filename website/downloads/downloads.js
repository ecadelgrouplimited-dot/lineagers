"use strict";

// Lists the files of the latest release from latest.txt and SHA256SUMS, so the page
// always matches what is actually published.
const DESCRIPTIONS = [
  [/-x86_64-linux\.tar\.gz$/, "lineage CLI and guard-server, Linux x86-64 (static)"],
  [/-src\.tar\.gz$/, "Source code"],
  [/^lineage_guard\.py$/, "Python client for guard-server (no dependencies)"],
];

function cell(text, className) {
  const td = document.createElement("td");
  td.textContent = text;
  if (className) td.className = className;
  return td;
}

async function load() {
  const body = document.getElementById("files");
  try {
    const version = (await (await fetch("/downloads/latest.txt", { cache: "no-cache" })).text()).trim();
    document.getElementById("version").textContent = version;
    document.querySelectorAll(".v").forEach((el) => { el.textContent = version; });
    const sums = await (await fetch(`/downloads/${version}/SHA256SUMS`, { cache: "no-cache" })).text();
    const rows = sums.trim().split("\n").map((line) => {
      const [hash, name] = line.trim().split(/\s+/);
      const row = document.createElement("tr");
      const link = document.createElement("a");
      link.href = `/downloads/${version}/${name}`;
      link.textContent = name;
      const fileCell = document.createElement("td");
      fileCell.append(link);
      const described = DESCRIPTIONS.find(([pattern]) => pattern.test(name));
      row.append(fileCell, cell(described ? described[1] : ""), cell(hash, "hash"));
      return row;
    });
    const sumsRow = document.createElement("tr");
    const sumsLink = document.createElement("a");
    sumsLink.href = `/downloads/${version}/SHA256SUMS`;
    sumsLink.textContent = "SHA256SUMS";
    const sumsCell = document.createElement("td");
    sumsCell.append(sumsLink);
    sumsRow.append(sumsCell, cell("Checksums for the files above"), cell(""));
    body.replaceChildren(...rows, sumsRow);
  } catch (e) {
    body.replaceChildren();
    const row = document.createElement("tr");
    const td = cell("Could not load the release list. Browse /downloads/ directly.", "muted");
    td.colSpan = 3;
    row.append(td);
    body.append(row);
  }
}

load();
