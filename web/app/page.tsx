"use client";

import { useMemo, useState } from "react";
import { solveCombat, type BrowserSolveResult, type TraceStep } from "../src/simulator";

type CharacterId = "silent" | "ironclad";
type EnemyId = "nibbit" | "fuzzy_wurm_crawler" | "shrinker_beetle";

type DeckItem = {
  name: string;
  type: "Attack" | "Skill";
  quantity: number;
  asset: string;
};

const CATALOG_SHA =
  "7a27dc78a49f6523b64dcc140117f8c21690d1fde6240208de488ee0e88e088c";

const characters = {
  silent: {
    name: "The Silent",
    hp: 70,
    description: "Starter deck · 12 cards",
    asset: "./spire-codex/characters/silent.webp",
  },
  ironclad: {
    name: "Ironclad",
    hp: 80,
    description: "Starter deck · 10 cards",
    asset: "./spire-codex/characters/ironclad.webp",
  },
} as const;

const enemies = {
  nibbit: {
    name: "Nibbit",
    hp: [42, 46],
    ascensionHp: [44, 48],
    intent: "12 attack",
    asset: "./spire-codex/monsters/nibbit.webp",
  },
  fuzzy_wurm_crawler: {
    name: "Fuzzy Wurm Crawler",
    hp: [55, 57],
    ascensionHp: [58, 59],
    intent: "4 attack",
    asset: "./spire-codex/monsters/fuzzy_wurm_crawler.webp",
  },
  shrinker_beetle: {
    name: "Shrinker Beetle",
    hp: [38, 40],
    ascensionHp: [40, 42],
    intent: "1 Shrink",
    asset: "./spire-codex/monsters/shrinker_beetle.webp",
  },
} as const;

const decks: Record<CharacterId, DeckItem[]> = {
  silent: [
    { name: "Strike", type: "Attack", quantity: 5, asset: "./spire-codex/cards/strike_silent.webp" },
    { name: "Defend", type: "Skill", quantity: 5, asset: "./spire-codex/cards/defend_silent.webp" },
    { name: "Neutralize", type: "Attack", quantity: 1, asset: "./spire-codex/cards/neutralize.webp" },
    { name: "Survivor", type: "Skill", quantity: 1, asset: "./spire-codex/cards/survivor.webp" },
  ],
  ironclad: [
    { name: "Strike", type: "Attack", quantity: 5, asset: "./spire-codex/cards/strike_ironclad.webp" },
    { name: "Defend", type: "Skill", quantity: 4, asset: "./spire-codex/cards/defend_ironclad.webp" },
    { name: "Bash", type: "Attack", quantity: 1, asset: "./spire-codex/cards/bash.webp" },
  ],
};

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

function cardAsset(name: string, character: CharacterId) {
  const match = decks[character].find((card) => card.name === name);
  return match?.asset ?? decks[character][0].asset;
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
  const hp = characters[character].hp;
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
      enemies: [{ id: `MONSTER.${enemy.toUpperCase()}`, current_hp: enemyHp, max_hp: enemyHp }],
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
    ? "Search running"
    : result?.optimality_proven
      ? result.won ? "Optimal result" : "No winning line"
      : result ? "Search incomplete" : solveError ? "Solver error" : "Ready";
  const selectedCharacter = characters[character];
  const selectedEnemy = enemies[enemy];

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
    <div className="app-shell" id="top">
      <header className="app-header">
        <a className="brand" href="#top" aria-label="SLS2 Combat Lab home">
          <span className="brand-mark">S2</span>
          <span><strong>Combat Lab</strong><small>Slay the Spire 2</small></span>
        </a>
        <nav aria-label="Primary navigation">
          <a href="#workspace">Solver</a>
          <a href="#method">Method</a>
          <a href="https://github.com/ruimin276/rusty-spire" target="_blank" rel="noreferrer">Source ↗</a>
        </nav>
        <div className="runtime-status" title={`Catalog ${CATALOG_SHA}`}>
          <span className="status-dot" /> Local Rust/WASM
        </div>
      </header>

      <main>
        <section className="page-intro">
          <div>
            <p className="eyebrow">Deterministic combat search</p>
            <h1>Combat solver</h1>
            <p>Configure one isolated encounter and find the winning line with the least HP loss.</p>
          </div>
          <dl className="catalog-summary">
            <div><dt>Catalog</dt><dd>v0.107.1</dd></div>
            <div><dt>Policy</dt><dd>Minimize HP loss</dd></div>
            <div><dt>Runtime</dt><dd>Local browser</dd></div>
          </dl>
        </section>

        <section className="workspace" id="workspace" aria-label="Combat solver workspace">
          <aside className="setup-panel">
            <div className="panel-heading">
              <div><span className="step-number">1</span><h2>Combat setup</h2></div>
              <button className="copy-button" type="button" onClick={copySetup}>
                {copied ? "Copied" : "Copy JSON"}
              </button>
            </div>

            <fieldset className="control-group">
              <legend>Character</legend>
              <div className="character-options">
                {(Object.keys(characters) as CharacterId[]).map((id) => {
                  const item = characters[id];
                  return (
                    <button
                      type="button"
                      key={id}
                      className={character === id ? "active" : ""}
                      aria-pressed={character === id}
                      onClick={() => setCharacter(id)}
                    >
                      <span className="character-art"><img src={item.asset} alt="" /></span>
                      <span><strong>{item.name}</strong><small>{item.hp} HP · {item.description}</small></span>
                      <span className="selection-mark" />
                    </button>
                  );
                })}
              </div>
            </fieldset>

            <fieldset className="control-group">
              <legend>Enemy</legend>
              <div className="enemy-list">
                {(Object.keys(enemies) as EnemyId[]).map((id) => {
                  const item = enemies[id];
                  return (
                    <button
                      type="button"
                      key={id}
                      className={enemy === id ? "active" : ""}
                      aria-pressed={enemy === id}
                      onClick={() => setEnemy(id)}
                    >
                      <span className="enemy-art"><img src={item.asset} alt="" /></span>
                      <span className="enemy-name"><strong>{item.name}</strong><small>{item.hp[0]}–{item.hp[1]} HP · {item.intent}</small></span>
                      <span className="selection-mark" />
                    </button>
                  );
                })}
              </div>
            </fieldset>

            <div className="parameter-grid">
              <div className="field-control">
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
              <div className="field-control">
                <label htmlFor="ascension">Ascension</label>
                <select id="ascension" value={ascension} onChange={(event) => setAscension(Number(event.target.value))}>
                  {Array.from({ length: 11 }, (_, value) => <option key={value} value={value}>A{value}</option>)}
                </select>
              </div>
            </div>

            <div className="seed-presets" aria-label="Seed presets">
              <span>Presets</span>
              {quickSeeds.map((value) => (
                <button type="button" key={value} className={seed === value ? "active" : ""} onClick={() => setSeed(value)}>{value}</button>
              ))}
            </div>

            <section className="loadout" aria-labelledby="loadout-title">
              <div className="section-heading">
                <div><h3 id="loadout-title">Starter loadout</h3><span>{decks[character].reduce((total, card) => total + card.quantity, 0)} cards</span></div>
                <div className="relic-summary">
                  <img
                    src={character === "silent" ? "./spire-codex/relics/ring_of_the_snake.webp" : "./spire-codex/relics/burning_blood.webp"}
                    alt=""
                  />
                  <span>{character === "silent" ? "Ring of the Snake" : "Burning Blood"}</span>
                </div>
              </div>
              <div className="deck-list">
                {decks[character].map((card) => (
                  <div className="deck-row" key={card.name}>
                    <img src={card.asset} alt="" />
                    <span><strong>{card.name}</strong><small>{card.type}</small></span>
                    <b>×{card.quantity}</b>
                  </div>
                ))}
              </div>
            </section>

            <button className="solve-button" type="button" onClick={solve} disabled={isSolving}>
              <span>{isSolving ? "Searching…" : "Run optimal search"}</span>
              <span aria-hidden="true">→</span>
            </button>
            <p className="privacy-note">Runs locally in your browser. No combat data is uploaded.</p>
          </aside>

          <section className="result-panel" key={runKey} aria-live="polite">
            <div className="result-header">
              <div><span className="step-number">2</span><h2>Results</h2></div>
              <span className={`proof-badge ${result?.optimality_proven ? "verified" : "pending"}`}>
                <span />{proofLabel}
              </span>
            </div>

            {winningResult ? (
              <>
                <section className="result-summary">
                  <div className="matchup-art" aria-hidden="true">
                    <img className="result-character" src={selectedCharacter.asset} alt="" />
                    <span>vs</span>
                    <img className="result-enemy" src={selectedEnemy.asset} alt="" />
                  </div>
                  <div className="result-copy">
                    <span className="success-label">Victory · optimum proven</span>
                    <h3>{winningResult.hpLoss === 0 ? "No HP lost" : `${winningResult.hpLoss} HP lost`}</h3>
                    <p>{selectedCharacter.name} finishes with <strong>{winningResult.finalHp} HP</strong>.</p>
                  </div>
                </section>

                <div className="metric-strip">
                  <div><span>Final HP</span><strong>{winningResult.finalHp}</strong></div>
                  <div><span>Enemy turns</span><strong>{winningResult.turns}</strong></div>
                  <div><span>Actions</span><strong>{winningResult.actions}</strong></div>
                  <div><span>States</span><strong>{shortNumber(winningResult.explored)}</strong></div>
                  <div><span>Runtime</span><strong>{winningResult.runtime < 0.01 ? `${(winningResult.runtime * 1000).toFixed(1)}ms` : `${winningResult.runtime.toFixed(3)}s`}</strong></div>
                </div>

                <section className="draw-section">
                  <div className="section-title"><h3>Opening hand</h3><span>Seed {seed} · shuffle stream</span></div>
                  <div className="card-hand">
                    {winningResult.opening.map((card, index) => (
                      <div className="mini-card" key={`${card}-${index}`}>
                        <img src={cardAsset(card, character)} alt={`${card} card`} />
                        <span>{card}</span>
                      </div>
                    ))}
                  </div>
                </section>

                <section className="trace-section">
                  <div className="section-title"><h3>Optimal action sequence</h3><span>{trace.length} actions · hashes shortened</span></div>
                  <div className="trace-list">
                    {trace.map((item, index) => (
                      <div className="trace-row" key={`${item.turn}-${index}`}>
                        <span className="turn-label">T{item.turn}</span>
                        <strong>{item.action}</strong>
                        <span>{item.detail}</span>
                        <code>{item.hash}</code>
                      </div>
                    ))}
                  </div>
                </section>
              </>
            ) : (
              <div className="empty-state">
                <div className="empty-matchup" aria-hidden="true">
                  <img src={selectedCharacter.asset} alt="" />
                  <span>→</span>
                  <img src={selectedEnemy.asset} alt="" />
                </div>
                <span className="empty-kicker">{solveError ? "Solver error" : isSolving ? "Search in progress" : result ? "Search complete" : "Current matchup"}</span>
                <h3>
                  {solveError
                    ? "The solver could not start"
                    : isSolving
                      ? "Exploring the combat graph…"
                      : result?.complete
                        ? "No winning line exists"
                        : result
                          ? "The search limit was reached"
                          : `${selectedCharacter.name} vs. ${selectedEnemy.name}`}
                </h3>
                <p>
                  {solveError
                    ? solveError
                    : isSolving
                      ? "Evaluating legal actions in a background Web Worker."
                      : result
                        ? `${shortNumber(result.explored_states)} states explored · ${result.termination_reason.replaceAll("_", " ")}.`
                        : `Seed ${seed} · Ascension ${ascension} · ${selectedEnemy.hp[0]}–${selectedEnemy.hp[1]} base HP`}
                </p>
                <button type="button" onClick={solve} disabled={isSolving}>{isSolving ? "Searching…" : solveError ? "Try again" : "Run search"}</button>
              </div>
            )}
          </section>
        </section>

        <section className="method" id="method">
          <div className="method-heading">
            <p className="eyebrow">Method</p>
            <h2>How the solver works</h2>
            <p>The simulator is deterministic, catalog-pinned, and runs entirely in the browser.</p>
          </div>
          <div className="method-grid">
            <article><span>01</span><h3>Freeze the inputs</h3><p>Catalog identity, deck, enemy HP, relics, seed, and ascension are captured in a strict setup.</p></article>
            <article><span>02</span><h3>Explore safely</h3><p>Every branch owns its state and RNG counters, so search order cannot alter future draws.</p></article>
            <article><span>03</span><h3>Prove the result</h3><p>The first victory removed from the loss-ordered frontier has the minimum possible HP loss.</p></article>
          </div>
        </section>
      </main>

      <footer>
        <span>Rusty Spire · deterministic isolated combat simulator</span>
        <span>Game artwork sourced from <a href="https://spire-codex.com/developers" target="_blank" rel="noreferrer">Spire Codex ↗</a></span>
      </footer>
    </div>
  );
}
