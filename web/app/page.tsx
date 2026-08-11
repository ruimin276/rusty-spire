"use client";

import { useMemo, useState } from "react";
import { solveCombat, type BrowserSolveResult, type TraceStep } from "../src/simulator";

type CharacterId = "silent" | "ironclad";
type EnemyId = "nibbit" | "fuzzy_wurm_crawler" | "shrinker_beetle";

const CATALOG_SHA =
  "7a27dc78a49f6523b64dcc140117f8c21690d1fde6240208de488ee0e88e088c";

const enemies = {
  nibbit: {
    name: "Nibbit",
    code: "NIB",
    hp: [42, 46],
    ascensionHp: [44, 48],
    intent: "12 attack",
    accent: "coral",
  },
  fuzzy_wurm_crawler: {
    name: "Fuzzy Wurm Crawler",
    code: "FWC",
    hp: [55, 57],
    ascensionHp: [58, 59],
    intent: "4 attack",
    accent: "acid",
  },
  shrinker_beetle: {
    name: "Shrinker Beetle",
    code: "SHR",
    hp: [38, 40],
    ascensionHp: [40, 42],
    intent: "1 shrink",
    accent: "violet",
  },
} as const;

const quickSeeds = [1, 2, 4, 7, 17, 42];

function shortNumber(value: number) {
  return new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

function cardName(modelId: string) {
  return modelId
    .replace("CARD.", "")
    .replace("_SILENT", "")
    .replace("_IRONCLAD", "")
    .split("_")
    .map((part) => part[0] + part.slice(1).toLowerCase())
    .join(" ");
}

function cardCost(name: string) {
  if (name === "Neutralize") return 0;
  if (name === "Bash") return 2;
  return 1;
}

function cardEffect(name: string) {
  if (name === "Strike") return "6 DMG";
  if (name === "Defend") return "5 BLK";
  if (name === "Survivor") return "8 BLK";
  if (name === "Bash") return "8 DMG · 2 VULN";
  return "3 DMG · 1 WK";
}

function traceRows(steps: TraceStep[]) {
  let turn = 1;
  return steps.map((step) => {
    const action = step.action;
    const card = action.card_id ? cardName(action.card_id) : null;
    const row = {
      turn: String(turn).padStart(2, "0"),
      action: card ?? (action.type === "end_turn" ? "End turn" : "Choose card"),
      detail: card
        ? `${action.cost ?? 0} energy${action.target_combat_id ? " · target enemy" : ""}`
        : action.type === "end_turn"
          ? "Enemy resolves · next draw"
          : `Select ${action.selection.join(", ")}`,
      hash: step.state_hash.slice(0, 8),
    };
    if (action.type === "end_turn") turn += 1;
    return row;
  });
}

function buildSetup(character: CharacterId, enemy: EnemyId, seed: number, ascension: number) {
  const isSilent = character === "silent";
  const hp = isSilent ? 70 : 80;
  const range = ascension >= 8 ? enemies[enemy].ascensionHp : enemies[enemy].hp;
  const enemyHp = Math.floor((range[0] + range[1]) / 2);

  return {
    schema_version: 1,
    catalog_sha256: CATALOG_SHA,
    ascension_level: ascension,
    rng: { run_seed: String(seed), profile: "isolated_combat_xoshiro_v1" },
    character: {
      id: isSilent ? "CHARACTER.SILENT" : "CHARACTER.IRONCLAD",
      current_hp: hp,
      max_hp: hp,
    },
    deck: isSilent
      ? [
          { id: "CARD.STRIKE_SILENT", quantity: 5, upgrade_level: 0 },
          { id: "CARD.DEFEND_SILENT", quantity: 5, upgrade_level: 0 },
          { id: "CARD.NEUTRALIZE", quantity: 1, upgrade_level: 0 },
          { id: "CARD.SURVIVOR", quantity: 1, upgrade_level: 0 },
        ]
      : [
          { id: "CARD.STRIKE_IRONCLAD", quantity: 5, upgrade_level: 0 },
          { id: "CARD.DEFEND_IRONCLAD", quantity: 4, upgrade_level: 0 },
          { id: "CARD.BASH", quantity: 1, upgrade_level: 0 },
        ],
    relics: [{ id: isSilent ? "RELIC.RING_OF_THE_SNAKE" : "RELIC.BURNING_BLOOD" }],
    potions: [],
    encounter: {
      type: "custom",
      enemies: [
        {
          id: `MONSTER.${enemy.toUpperCase()}`,
          current_hp: enemyHp,
          max_hp: enemyHp,
        },
      ],
    },
    policy: "minimize_hp_loss",
  };
}

export default function Home() {
  const [character, setCharacter] = useState<CharacterId>("silent");
  const [enemy, setEnemy] = useState<EnemyId>("nibbit");
  const [seed, setSeed] = useState(1);
  const [ascension, setAscension] = useState(0);
  const [isSolving, setIsSolving] = useState(false);
  const [runKey, setRunKey] = useState(0);
  const [copied, setCopied] = useState(false);
  const [wasmRun, setWasmRun] = useState<{
    setupSignature: string;
    value: BrowserSolveResult;
  } | null>(null);
  const [solveError, setSolveError] = useState<string | null>(null);

  const setup = useMemo(
    () => buildSetup(character, enemy, seed, ascension),
    [ascension, character, enemy, seed],
  );
  const setupSignature = JSON.stringify(setup);
  const activeRun = wasmRun?.setupSignature === setupSignature ? wasmRun.value : null;
  const result = activeRun?.result ?? null;
  const winningResult = result?.won && result.hp_loss !== null && result.final_hp !== null
    ? {
        hpLoss: result.hp_loss,
        finalHp: result.final_hp,
        turns: result.actions.filter((step) => step.action.type === "end_turn").length,
        actions: result.actions.length,
        explored: result.explored_states,
        runtime: result.runtime_seconds,
        opening: activeRun?.opening_hand.map(cardName) ?? [],
      }
    : null;
  const trace = result ? traceRows(result.actions) : [];
  const proofLabel = isSolving
    ? "WASM SEARCH RUNNING"
    : result?.optimality_proven
      ? result.won ? "WASM OPTIMUM PROVEN" : "NO WINNING LINE"
      : result ? "WASM SEARCH INCOMPLETE" : solveError ? "WASM ERROR" : "RUST / WASM READY";

  const deck = character === "silent"
    ? [
        ["Strike", "Attack", 5],
        ["Defend", "Skill", 5],
        ["Neutralize", "Attack", 1],
        ["Survivor", "Skill", 1],
      ]
    : [
        ["Strike", "Attack", 5],
        ["Defend", "Skill", 4],
        ["Bash", "Attack", 1],
      ];

  async function solve() {
    setIsSolving(true);
    setSolveError(null);
    try {
      const value = await solveCombat(setup);
      setWasmRun({ setupSignature, value });
      setRunKey((value) => value + 1);
    } catch (error) {
      setSolveError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSolving(false);
    }
  }

  async function copySetup() {
    await navigator.clipboard.writeText(JSON.stringify(setup, null, 2));
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1400);
  }

  return (
    <main className="site-shell">
      <header className="topbar">
        <a className="brand" href="#top" aria-label="SLS2 Combat Lab home">
          <span className="brand-mark">S2</span>
          <span>SLS2 / COMBAT LAB</span>
        </a>
        <div className="catalog-status">
          <span className="status-dot" />
          <span>RUST / WASM LOCAL</span>
          <span className="catalog-hash">7a27dc78</span>
        </div>
        <a className="text-link" href="#method">How it works <span>↗</span></a>
      </header>

      <section className="hero" id="top">
        <div>
          <p className="eyebrow">DETERMINISTIC COMBAT SEARCH</p>
          <h1>Find the line.<br /><em>Keep the HP.</em></h1>
        </div>
        <p className="hero-copy">
          Reconstruct a single combat from its seed, then search every legal line for the
          smallest possible health loss. Same setup in, same trace out.
        </p>
      </section>

      <section className="workbench" aria-label="Combat simulator workbench">
        <aside className="setup-panel">
          <div className="panel-heading">
            <span className="step-number">01</span>
            <div><p className="kicker">COMBAT INPUT</p><h2>Build the encounter</h2></div>
          </div>

          <fieldset className="control-group">
            <legend>Character</legend>
            <div className="segment-control">
              {(["silent", "ironclad"] as CharacterId[]).map((id) => (
                <button
                  type="button"
                  key={id}
                  className={character === id ? "active" : ""}
                  onClick={() => setCharacter(id)}
                >
                  <span className={`character-glyph ${id}`}>{id === "silent" ? "S" : "I"}</span>
                  <span>{id === "silent" ? "The Silent" : "Ironclad"}</span>
                  <small>{id === "silent" ? "70 HP" : "80 HP"}</small>
                </button>
              ))}
            </div>
          </fieldset>

          <fieldset className="control-group">
            <legend>Single enemy</legend>
            <div className="enemy-list">
              {(Object.keys(enemies) as EnemyId[]).map((id) => {
                const item = enemies[id];
                return (
                  <button
                    type="button"
                    key={id}
                    className={`enemy-option ${enemy === id ? "active" : ""}`}
                    onClick={() => setEnemy(id)}
                  >
                    <span className={`enemy-sigil ${item.accent}`}>{item.code}</span>
                    <span className="enemy-name">{item.name}<small>{item.hp[0]}–{item.hp[1]} HP</small></span>
                    <span className="enemy-intent">{item.intent}</span>
                    <span className="radio-mark" />
                  </button>
                );
              })}
            </div>
          </fieldset>

          <div className="seed-row">
            <label htmlFor="seed">Run seed</label>
            <input
              id="seed"
              type="number"
              min="0"
              max="4294967295"
              value={seed}
              onChange={(event) => setSeed(Math.min(4_294_967_295, Math.max(0, Number(event.target.value))))}
            />
          </div>
          <div className="seed-presets" aria-label="Seed presets">
            {quickSeeds.map((value) => (
              <button type="button" key={value} className={seed === value ? "active" : ""} onClick={() => setSeed(value)}>{value}</button>
            ))}
          </div>

          <div className="ascension-control">
            <div><label htmlFor="ascension">Ascension</label><output>A{ascension}</output></div>
            <input id="ascension" type="range" min="0" max="10" value={ascension} onChange={(event) => setAscension(Number(event.target.value))} />
            <div className="range-labels"><span>BASE</span><span>TOUGH · A8</span><span>DEADLY · A9</span></div>
          </div>

          <button className="solve-button" type="button" onClick={solve} disabled={isSolving}>
            <span>{isSolving ? "RUST IS SEARCHING" : "RUN OPTIMAL SEARCH"}</span>
            <span className="button-arrow">{isSolving ? "···" : "→"}</span>
          </button>
          <button className="copy-button" type="button" onClick={copySetup}>{copied ? "SETUP COPIED" : "COPY COMBATSETUPV1 JSON"}</button>
        </aside>

        <section className="result-panel" key={runKey} aria-live="polite">
          <div className="result-topline">
            <div><span className="step-number">02</span><p className="kicker">SEARCH OUTPUT</p></div>
            <span className={`proof-badge ${result?.optimality_proven ? "verified" : "pending"}`}>
              <span />{proofLabel}
            </span>
          </div>

          {winningResult ? (
            <>
              <div className="victory-block">
                <div className="hp-orbit">
                  <span className="orbit-label">HP LOSS</span>
                  <strong>{winningResult.hpLoss}</strong>
                  <span>/ {character === "silent" ? 70 : 80}</span>
                </div>
                <div className="victory-copy">
                  <p>VICTORY</p>
                  <h2>{winningResult.hpLoss === 0 ? "No damage taken." : `${winningResult.finalHp} HP remains.`}</h2>
                  <span>Computed locally by the Rust core running as WebAssembly.</span>
                </div>
              </div>

              <div className="metric-strip">
                <div><span>FINAL HP</span><strong>{winningResult.finalHp}</strong></div>
                <div><span>ENEMY TURNS</span><strong>{winningResult.turns}</strong></div>
                <div><span>ACTIONS</span><strong>{winningResult.actions}</strong></div>
                <div><span>STATES</span><strong>{shortNumber(winningResult.explored)}</strong></div>
                <div><span>RUNTIME</span><strong>{winningResult.runtime < 0.01 ? `${(winningResult.runtime * 1000).toFixed(1)}ms` : `${winningResult.runtime.toFixed(3)}s`}</strong></div>
              </div>

              <div className="draw-section">
                <div className="section-title"><h3>Opening draw</h3><span>shuffle stream · counter 11</span></div>
                <div className="card-hand">
                  {winningResult.opening.map((card, index) => (
                    <div className={`mini-card ${card.toLowerCase()}`} key={`${card}-${index}`}>
                      <span className="energy">{cardCost(card)}</span>
                      <strong>{card}</strong>
                      <small>{cardEffect(card)}</small>
                    </div>
                  ))}
                </div>
              </div>

              <div className="trace-section">
                <div className="section-title"><h3>Optimal line</h3><span>full Rust trace · state hashes shortened</span></div>
                <div className="trace-list wasm-trace">
                  {trace.map((item, index) => (
                    <div className="trace-row" key={`${item.turn}-${index}`}>
                      <span className="turn-label">T{item.turn}</span>
                      <span className="trace-node" />
                      <strong>{item.action}</strong>
                      <span>{item.detail}</span>
                      <code>{item.hash}</code>
                    </div>
                  ))}
                </div>
              </div>
            </>
          ) : (
            <div className="custom-state">
              <span className="custom-glyph">{solveError ? "!" : isSolving ? "⟳" : "↳"}</span>
              <p className="kicker">{solveError ? "SIMULATOR ERROR" : result ? "SEARCH RESULT" : "CLIENT RUNTIME"}</p>
              <h2>
                {solveError
                  ? <>The browser solver<br />could not start.</>
                  : isSolving
                    ? <>Exploring the<br />combat graph…</>
                    : result?.complete
                      ? <>No winning line<br />exists.</>
                      : result
                        ? <>The search limit<br />was reached.</>
                        : <>Rust is ready<br />in your browser.</>}
              </h2>
              <p>
                {solveError
                  ? solveError
                  : isSolving
                    ? "The WebAssembly worker is evaluating legal actions without blocking the interface."
                    : result
                      ? `${shortNumber(result.explored_states)} states explored · ${result.termination_reason.replaceAll("_", " ")}. Incomplete searches never claim optimality.`
                      : "Press Run Optimal Search to execute the real Rust combat engine locally. No combat data is sent to a server."}
              </p>
              <button type="button" onClick={solve} disabled={isSolving}>{isSolving ? "SEARCHING" : solveError ? "TRY AGAIN" : "RUN RUST / WASM SEARCH"}</button>
            </div>
          )}
        </section>

        <aside className="deck-panel">
          <div className="panel-heading compact">
            <span className="step-number">03</span>
            <div><p className="kicker">LOADOUT</p><h2>Starter deck</h2></div>
          </div>
          <div className="deck-list">
            {deck.map(([name, type, quantity]) => (
              <div className="deck-row" key={String(name)}>
                <span className={`deck-type ${String(type).toLowerCase()}`}>{String(type).slice(0, 1)}</span>
                <div><strong>{name}</strong><small>{type} · base</small></div>
                <span>×{quantity}</span>
              </div>
            ))}
          </div>
          <div className="relic-row">
            <span className="relic-glyph">◇</span>
            <div><small>RELIC</small><strong>{character === "silent" ? "Ring of the Snake" : "Burning Blood"}</strong></div>
          </div>
          <div className="policy-card">
            <span>POLICY · RUST/WASM</span>
            <strong>Minimize HP loss</strong>
            <p>Graph search orders states by monotonic damage taken. Tie-breakers never redefine the optimum.</p>
          </div>
          <div className="rng-readout">
            <div><span>RUN SEED</span><strong>{seed}</strong></div>
            <div><span>PROFILE</span><code>xoshiro_v1</code></div>
            <div><span>CATALOG</span><code>7a27dc78…</code></div>
          </div>
        </aside>
      </section>

      <section className="method" id="method">
        <div><p className="eyebrow">WHY IT REPLAYS</p><h2>One seed.<br />Named streams.<br />Zero ambiguity.</h2></div>
        <div className="method-grid">
          <article><span>01</span><h3>Freeze the inputs</h3><p>Catalog identity, deck instances, enemy HP, relics, and ascension are captured in one strict setup.</p></article>
          <article><span>02</span><h3>Branch safely</h3><p>Every candidate state owns its RNG counters, so search order cannot alter the next draw or move.</p></article>
          <article><span>03</span><h3>Prove the optimum</h3><p>The first victory removed from the loss-ordered frontier is the minimum-HP-loss solution.</p></article>
        </div>
      </section>

      <footer>
        <div><span className="brand-mark">S2</span><span>Isolated combat. Reproducible by construction.</span></div>
        <span>CLIENT-SIDE RUST/WASM · OFFLINE CATALOG · v0.2</span>
      </footer>
    </main>
  );
}
