const page = document.body.dataset.page;
let currentUser = null;

function qs(selector) {
  return document.querySelector(selector);
}

function setStatus(message, isError = false, target = "#status") {
  const el = qs(target);
  if (!el) return;
  el.textContent = message || "";
  el.classList.toggle("error", isError);
}

async function api(path, options = {}) {
  const response = await fetch(path, {
    credentials: "same-origin",
    headers: {
      "Content-Type": "application/json",
      ...(options.headers || {}),
    },
    ...options,
  });
  const text = await response.text();
  const data = text ? JSON.parse(text) : null;
  if (!response.ok) {
    throw new Error(data?.error?.message || response.statusText);
  }
  return data;
}

function html(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function formatDate(value) {
  return value ? new Date(value).toLocaleString() : "";
}

function table(headers, rows) {
  return `<div class="table-wrap"><table><thead><tr>${headers
    .map((header) => `<th>${html(header)}</th>`)
    .join("")}</tr></thead><tbody>${rows.join("")}</tbody></table></div>`;
}

async function loadCurrentUser() {
  try {
    return await api("/api/auth/me");
  } catch (_) {
    return null;
  }
}

function isAdmin() {
  return currentUser?.role === "admin";
}

function applyAuthState() {
  const admin = isAdmin();
  const authenticated = Boolean(currentUser);

  document.querySelectorAll("[data-admin-only]").forEach((element) => {
    element.classList.toggle("auth-visible", admin);
    element.classList.toggle("hidden", !admin);
  });
  document.querySelectorAll("[data-auth-only]").forEach((element) => {
    element.classList.toggle("auth-visible", authenticated);
    element.classList.toggle("hidden", !authenticated);
  });
  document.querySelectorAll("[data-anonymous-only]").forEach((element) => {
    element.classList.toggle("auth-visible", !authenticated);
    element.classList.toggle("hidden", authenticated);
  });
  document.querySelectorAll("[data-non-admin-only]").forEach((element) => {
    element.classList.toggle("auth-visible", !admin);
    element.classList.toggle("hidden", admin);
  });
}

function wireLogout() {
  const button = qs("#logout");
  if (!button) return;
  button.addEventListener("click", async () => {
    await api("/api/auth/logout", { method: "POST", body: "{}" }).catch(
      () => null,
    );
    window.location.href = "/login";
  });
}

async function initLogin() {
  qs("#login-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    try {
      await api("/api/auth/login", {
        method: "POST",
        body: JSON.stringify({
          username: form.get("username"),
          password: form.get("password"),
        }),
      });
      window.location.href = "/leaderboard";
    } catch (error) {
      setStatus(error.message, true);
    }
  });
}

async function initLeaderboard() {
  let currentRows = [];

  const rankLabel = (rank) => (rank == null ? "UR" : rank);
  const percentage = (value) =>
    value == null ? "N/A" : `${Math.round(value * 100)}%`;
  const averagePlacement = (value) =>
    value == null ? "N/A" : value.toFixed(2);

  const renderRows = () => {
    const search = qs("#player-search").value.trim().toLowerCase();
    const rows = currentRows.filter((row) =>
      row.display_name.toLowerCase().includes(search),
    );
    qs("#leaderboard-count").textContent =
      `${rows.length} player${rows.length === 1 ? "" : "s"}`;

    if (rows.length === 0) {
      qs("#leaderboard").innerHTML =
        `<div class="empty-state">No players match the current filters.</div>`;
      return;
    }

    qs("#leaderboard").innerHTML =
      `<div class="rank-table" role="table" aria-label="Coup leaderboard">
      <div class="rank-head" role="row">
        <span>Rank</span>
        <span>Player</span>
        <span>Rating</span>
        <span>Rank score</span>
        <span>Record</span>
        <span>Last played</span>
      </div>
      ${rows
        .map(
          (
            row,
          ) => `<div class="rank-row${row.rank == null ? " unranked" : ""}${row.active ? "" : " inactive"}" role="row">
            <div class="rank-cell rank-number" role="cell">${rankLabel(row.rank)}</div>
            <div class="rank-cell player-cell" role="cell">
              <strong>${html(row.display_name)}</strong>
              <span>${row.active ? "Active" : "Inactive"} - ${row.games_played} game${row.games_played === 1 ? "" : "s"} - Avg place ${averagePlacement(row.average_placement)}</span>
            </div>
            <div class="rank-cell metric-cell" role="cell">
              <strong>${row.display_rating.toLocaleString()}</strong>
              <span>display</span>
            </div>
            <div class="rank-cell metric-cell" role="cell">
              <strong>${row.rank_score.toLocaleString()}</strong>
              <span>conservative</span>
            </div>
            <div class="rank-cell metric-cell" role="cell">
              <strong>${row.wins}-${row.losses}</strong>
              <span>${percentage(row.win_rate)} win rate</span>
            </div>
            <div class="rank-cell last-played" role="cell">${formatDate(row.last_played_at) || "N/A"}</div>
          </div>`,
        )
        .join("")}
    </div>`;
  };

  const render = async () => {
    const minGames = qs("#min-games").value || "3";
    try {
      currentRows = await api(
        `/api/leaderboard?min_games=${encodeURIComponent(minGames)}`,
      );
      renderRows();
      setStatus("");
    } catch (error) {
      qs("#leaderboard-count").textContent = "Unavailable";
      setStatus(error.message, true);
    }
  };
  qs("#player-search").addEventListener("input", renderRows);
  qs("#min-games").addEventListener("change", render);
  await render();
}

async function initPlayers() {
  const createDialog = qs("#create-player-dialog");
  const createForm = qs("#create-player-form");
  const playersContainer = qs("#players");

  const closeCreateDialog = () => {
    createDialog?.close();
    setStatus("", false, "#create-status");
  };

  const renderPlayersTable = (rows) => {
    playersContainer.innerHTML = table(
      ["Name", "Active", "Rating", "Score", "Games", "Wins", "Losses"],
      rows.map(
        (row) => `<tr class="player-row">
          <td>${html(row.display_name)}</td>
          <td>${row.active ? "Yes" : "No"}</td>
          <td>${row.rating.display_rating}</td>
          <td>${row.rating.rank_score}</td>
          <td>${row.rating.games_played}</td>
          <td>${row.rating.wins}</td>
          <td>${row.rating.losses}</td>
        </tr>`,
      ),
    );
  };

  const render = async () => {
    try {
      const rows = await api("/api/players");
      renderPlayersTable(rows);
      setStatus("");
    } catch (error) {
      setStatus(error.message, true);
    }
  };
  qs("#open-create-player").addEventListener("click", () => {
    if (!currentUser) {
      window.location.href = "/login";
      return;
    }
    if (!isAdmin()) {
      setStatus("Admin access is required to create players.", true);
      return;
    }
    createDialog?.showModal();
    createForm?.elements.display_name?.focus();
  });
  qs("#close-create-player")?.addEventListener("click", closeCreateDialog);
  createDialog?.addEventListener("click", (event) => {
    if (event.target === createDialog) closeCreateDialog();
  });
  createForm?.addEventListener("submit", async (event) => {
    event.preventDefault();
    const formEl = event.currentTarget;
    const form = new FormData(formEl);
    try {
      await api("/api/players", {
        method: "POST",
        body: JSON.stringify({ display_name: form.get("display_name") }),
      });
      formEl.reset();
      closeCreateDialog();
      setStatus("Created player.");
      await render();
    } catch (error) {
      setStatus(error.message, true, "#create-status");
    }
  });
  await render();
}

async function initMatches() {
  try {
    const rows = await api("/api/matches");
    qs("#matches").innerHTML = table(
      ["Played", "Status", "Notes", "Detail"],
      rows.map(
        (row) =>
          `<tr><td>${formatDate(row.played_at)}</td><td>${html(row.status)}</td><td>${html(row.notes)}</td><td><a href="/matches/${row.id}">Open</a></td></tr>`,
      ),
    );
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function initMatchDetail() {
  const id = window.location.pathname.split("/").pop();
  const render = async () => {
    const detail = await api(`/api/matches/${id}`);
    qs("#match-detail").innerHTML =
      `<div class="stack"><p><strong>Status:</strong> ${html(detail.status)}<br><strong>Played:</strong> ${formatDate(detail.played_at)}<br><strong>Notes:</strong> ${html(detail.notes)}</p>${table(
        ["Place", "Player", "Old", "New", "Delta"],
        detail.participants.map(
          (row) =>
            `<tr><td>${row.placement}</td><td>${html(row.display_name)}</td><td>${row.old_display_rating}</td><td>${row.new_display_rating}</td><td>${row.display_delta}</td></tr>`,
        ),
      )}</div>`;
    const voidButton = qs("#void-match");
    if (voidButton) voidButton.disabled = detail.status !== "confirmed";
  };
  qs("#void-match")?.addEventListener("click", async () => {
    try {
      await api(`/api/matches/${id}/void`, { method: "POST", body: "{}" });
      await render();
    } catch (error) {
      setStatus(error.message, true);
    }
  });
  try {
    await render();
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function initSubmitMatch() {
  if (!isAdmin()) {
    setStatus("Admin login required to submit matches.", true);
    return;
  }
  const players = await api("/api/players");
  qs("#participant-fields").innerHTML = players
    .map(
      (player) =>
        `<div class="participant-row"><label>${html(player.display_name)}<select name="player_id"><option value="">Not playing</option><option value="${player.id}">${html(player.display_name)}</option></select></label><label>Place <input name="placement:${player.id}" type="number" min="1"></label></div>`,
    )
    .join("");
  qs("#submit-match-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    const placements = [];
    for (const playerId of form.getAll("player_id").filter(Boolean)) {
      const placement = Number(form.get(`placement:${playerId}`));
      if (placement > 0) placements.push({ player_id: playerId, placement });
    }
    try {
      const playedAt = new Date(form.get("played_at")).toISOString();
      const notes = form.get("notes") || null;
      const response = await api("/api/matches", {
        method: "POST",
        body: JSON.stringify({ played_at: playedAt, notes, placements }),
      });
      window.location.href = `/matches/${response.match_id}`;
    } catch (error) {
      setStatus(error.message, true);
    }
  });
}

async function initAuditLog() {
  if (!isAdmin()) {
    setStatus("Admin login required to view the audit log.", true);
    return;
  }
  const render = async () => {
    try {
      const limit = qs("#limit").value || "100";
      const rows = await api(
        `/api/admin/audit-log?limit=${encodeURIComponent(limit)}`,
      );
      qs("#audit-log").innerHTML = table(
        ["Created", "Action", "Entity", "Value"],
        rows.map(
          (row) =>
            `<tr><td>${formatDate(row.created_at)}</td><td>${html(row.action)}</td><td>${html(row.entity_type)}</td><td><code>${html(JSON.stringify(row.new_value || row.old_value || {}))}</code></td></tr>`,
        ),
      );
      setStatus("");
    } catch (error) {
      setStatus(error.message, true);
    }
  };
  qs("#limit").addEventListener("change", render);
  await render();
}

const initializers = {
  login: initLogin,
  leaderboard: initLeaderboard,
  players: initPlayers,
  matches: initMatches,
  "match-detail": initMatchDetail,
  "submit-match": initSubmitMatch,
  "audit-log": initAuditLog,
};

async function main() {
  currentUser = await loadCurrentUser();
  applyAuthState();
  wireLogout();
  await initializers[page]?.();
}

main();
