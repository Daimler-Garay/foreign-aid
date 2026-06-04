const page = document.body.dataset.page;

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

function wireLogout() {
  const button = qs("#logout");
  if (!button) return;
  button.addEventListener("click", async () => {
    await api("/api/auth/logout", { method: "POST", body: "{}" }).catch(() => null);
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
  const render = async () => {
    const minGames = qs("#min-games").value || "3";
    const includeInactive = qs("#include-inactive").checked;
    try {
      const rows = await api(`/api/leaderboard?min_games=${encodeURIComponent(minGames)}&include_inactive=${includeInactive}`);
      qs("#leaderboard").innerHTML = table(
        ["Rank", "Player", "Rating", "Score", "Games", "Wins", "Win Rate", "Avg Place", "Last Played"],
        rows.map((row) => `<tr><td>${row.rank ?? "Unranked"}</td><td>${html(row.display_name)}</td><td>${row.display_rating}</td><td>${row.rank_score}</td><td>${row.games_played}</td><td>${row.wins}</td><td>${row.win_rate == null ? "" : `${Math.round(row.win_rate * 100)}%`}</td><td>${row.average_placement == null ? "" : row.average_placement.toFixed(2)}</td><td>${formatDate(row.last_played_at)}</td></tr>`),
      );
      setStatus("");
    } catch (error) {
      setStatus(error.message, true);
    }
  };
  qs("#refresh").addEventListener("click", render);
  await render();
}

async function initPlayers() {
  const render = async () => {
    try {
      const includeInactive = qs("#include-inactive").checked;
      const rows = await api(`/api/players?include_inactive=${includeInactive}`);
      qs("#players").innerHTML = table(
        ["Name", "Active", "Rating", "Score", "Games", "Wins", "Losses"],
        rows.map((row) => `<tr><td>${html(row.display_name)}</td><td>${row.active ? "Yes" : "No"}</td><td>${row.rating.display_rating}</td><td>${row.rating.rank_score}</td><td>${row.rating.games_played}</td><td>${row.rating.wins}</td><td>${row.rating.losses}</td></tr>`),
      );
      setStatus("");
    } catch (error) {
      setStatus(error.message, true);
    }
  };
  qs("#refresh").addEventListener("click", render);
  qs("#create-player-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    try {
      await api("/api/players", {
        method: "POST",
        body: JSON.stringify({ display_name: form.get("display_name") }),
      });
      event.currentTarget.reset();
      setStatus("Created.", false, "#create-status");
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
      ["Played", "Status", "Notes", "Rating", "Detail"],
      rows.map((row) => `<tr><td>${formatDate(row.played_at)}</td><td>${html(row.status)}</td><td>${html(row.notes)}</td><td>${html(row.rating_algorithm)} v${row.rating_algorithm_version}</td><td><a href="/matches/${row.id}">Open</a></td></tr>`),
    );
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function initMatchDetail() {
  const id = window.location.pathname.split("/").pop();
  const render = async () => {
    const detail = await api(`/api/matches/${id}`);
    qs("#match-detail").innerHTML = `<div class="stack"><p><strong>Status:</strong> ${html(detail.status)}<br><strong>Played:</strong> ${formatDate(detail.played_at)}<br><strong>Notes:</strong> ${html(detail.notes)}</p>${table(
      ["Place", "Player", "Old", "New", "Delta"],
      detail.participants.map((row) => `<tr><td>${row.placement}</td><td>${html(row.display_name)}</td><td>${row.old_display_rating}</td><td>${row.new_display_rating}</td><td>${row.display_delta}</td></tr>`),
    )}</div>`;
    qs("#void-match").disabled = detail.status !== "confirmed";
  };
  qs("#void-match").addEventListener("click", async () => {
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
  const players = await api("/api/players");
  qs("#participant-fields").innerHTML = players
    .map((player) => `<div class="participant-row"><label>${html(player.display_name)}<select name="player_id"><option value="">Not playing</option><option value="${player.id}">${html(player.display_name)}</option></select></label><label>Place <input name="placement:${player.id}" type="number" min="1"></label></div>`)
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
  const render = async () => {
    try {
      const limit = qs("#limit").value || "100";
      const rows = await api(`/api/admin/audit-log?limit=${encodeURIComponent(limit)}`);
      qs("#audit-log").innerHTML = table(
        ["Created", "Action", "Entity", "Value"],
        rows.map((row) => `<tr><td>${formatDate(row.created_at)}</td><td>${html(row.action)}</td><td>${html(row.entity_type)}</td><td><code>${html(JSON.stringify(row.new_value || row.old_value || {}))}</code></td></tr>`),
      );
      setStatus("");
    } catch (error) {
      setStatus(error.message, true);
    }
  };
  qs("#refresh").addEventListener("click", render);
  await render();
}

wireLogout();

const initializers = {
  login: initLogin,
  leaderboard: initLeaderboard,
  players: initPlayers,
  matches: initMatches,
  "match-detail": initMatchDetail,
  "submit-match": initSubmitMatch,
  "audit-log": initAuditLog,
};

initializers[page]?.();
