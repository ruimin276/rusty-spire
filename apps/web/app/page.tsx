"use client";

import { useEffect, useMemo, useState } from "react";
import {
  getContentManifest,
  solveCombat,
  type BrowserSolveResult,
  type CombatSetupV2,
  type ContentCard,
  type ContentCharacter,
  type ContentEnemy,
  type ContentManifest,
  type TraceStep,
} from "../src/simulator";

type CharacterId = string;
type EnemyId = string;

type DeckItem = {
  id: string;
  name: string;
  type: "Attack" | "Skill";
  quantity: number;
  asset: string;
};

const quickSeeds = [1, 2, 4, 7, 17, 42];

function shortNumber(value: number) {
  return new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

function assetUrl(asset: string | null) {
  return asset ? `./${asset}` : "./favicon.svg";
}

function traceRows(steps: TraceStep[], cards: Record<string, ContentCard>) {
  let turn = 1;
  return steps.map((step) => {
    const action = step.action;
    const card = action.card_id ? cards[action.card_id]?.name ?? action.card_id : null;
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

function buildSetup(
  manifest: ContentManifest,
  character: ContentCharacter,
  enemy: ContentEnemy,
  extraCards: string[],
  seed: number,
  ascension: number,
): CombatSetupV2 {
  const hp = character.max_hp;
  const range = ascension >= 8 ? enemy.ascension_hp : enemy.hp;
  const enemyHp = Math.floor((range[0] + range[1]) / 2);

  return {
    schema_version: 2,
    package: manifest.package,
    ascension_level: ascension,
    rng: { run_seed: String(seed), profile: "isolated_combat_xoshiro_v1" },
    character: {
      id: character.id,
      current_hp: hp,
      max_hp: hp,
    },
    deck: [
      ...character.starter_deck,
      ...extraCards.map((id) => ({ id, quantity: 1, upgrade_level: 0 })),
    ],
    relics: character.starter_relics.map((id) => ({ id })),
    potions: [],
    encounter: {
      type: "custom",
      enemies: [{ id: enemy.id, current_hp: enemyHp, max_hp: enemyHp }],
    },
  };
}

export default function Home() {
  const [manifest, setManifest] = useState<ContentManifest | null>(null);
  const [manifestError, setManifestError] = useState<string | null>(null);
  const [character, setCharacter] = useState<CharacterId>("CHARACTER.SILENT");
  const [enemy, setEnemy] = useState<EnemyId>("MONSTER.NIBBIT");
  const [seed, setSeed] = useState(1);
  const [extraCards, setExtraCards] = useState<string[]>([]);
  const [ascension, setAscension] = useState(0);
  const [isSolving, setIsSolving] = useState(false);
  const [runKey, setRunKey] = useState(0);
  const [copied, setCopied] = useState(false);
  const [wasmRun, setWasmRun] = useState<{
    setupSignature: string;
    value: BrowserSolveResult;
  } | null>(null);
  const [solveError, setSolveError] = useState<string | null>(null);

  useEffect(() => {
    getContentManifest().then(setManifest).catch((error) => {
      setManifestError(error instanceof Error ? error.message : String(error));
    });
  }, []);

  const characters = useMemo(
    () => Object.fromEntries((manifest?.characters ?? []).map((item) => [item.id, item])),
    [manifest],
  );
  const enemies = useMemo(
    () => Object.fromEntries((manifest?.enemies ?? []).map((item) => [item.id, item])),
    [manifest],
  );
  const cards = useMemo(
    () => Object.fromEntries((manifest?.cards ?? []).map((item) => [item.id, item])),
    [manifest],
  );
  const relics = useMemo(
    () => Object.fromEntries((manifest?.relics ?? []).map((item) => [item.id, item])),
    [manifest],
  );
  const decks = useMemo<Record<CharacterId, DeckItem[]>>(
    () => Object.fromEntries((manifest?.characters ?? []).map((item) => [
      item.id,
      item.starter_deck.map((entry) => {
        const card = cards[entry.id];
        return {
          id: entry.id,
          name: card?.name ?? entry.id,
          type: card?.card_type === "Attack" ? "Attack" : "Skill",
          quantity: entry.quantity,
          asset: assetUrl(card?.asset ?? null),
        };
      }),
    ])),
    [cards, manifest],
  );
  const selectedCharacter = characters[character];
  const selectedEnemy = enemies[enemy];
  const selectedRelic = selectedCharacter
    ? relics[selectedCharacter.starter_relics[0]]
    : undefined;
  const availableProofCards = (manifest?.cards ?? []).filter(
    (card) => card.character === character
      && !selectedCharacter?.starter_deck.some((entry) => entry.id === card.id),
  );
  const activeDeck = [
    ...(decks[character] ?? []),
    ...extraCards.map((id) => {
      const card = cards[id];
      return {
        id,
        name: card?.name ?? id,
        type: card?.card_type === "Attack" ? "Attack" as const : "Skill" as const,
        quantity: 1,
        asset: assetUrl(card?.asset ?? null),
      };
    }),
  ];
  const setup = useMemo(
    () => manifest && selectedCharacter && selectedEnemy
      ? buildSetup(manifest, selectedCharacter, selectedEnemy, extraCards, seed, ascension)
      : null,
    [ascension, extraCards, manifest, seed, selectedCharacter, selectedEnemy],
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
        opening: activeRun?.opening_hand ?? [],
      }
    : null;
  const trace = result ? traceRows(result.actions, cards) : [];
  const proofLabel = isSolving
    ? "Search running"
    : result?.optimality_proven
      ? result.won ? "Optimal result" : "No winning line"
      : result ? "Search incomplete" : solveError ? "Solver error" : "Ready";

  if (!manifest || !selectedCharacter || !selectedEnemy || !setup) {
    return (
      <main className="app-shell">
        <section className="empty-state">
          <h1>Loading combat package…</h1>
          {manifestError ? <p>{manifestError}</p> : <p>Initializing the local Rust/WASM engine.</p>}
        </section>
      </main>
    );
  }

  async function solve() {
    if (!setup) return;
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
    if (!setup) return;
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
        <div className="runtime-status" title={`${manifest.package.package_id} ${manifest.package.sha256}`}>
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
            <div><dt>Package</dt><dd>{manifest.game_version}</dd></div>
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
                      onClick={() => { setCharacter(id); setExtraCards([]); }}
                    >
                      <span className="character-art"><img src={assetUrl(item.asset)} alt="" /></span>
                      <span><strong>{item.name}</strong><small>{item.max_hp} HP · Starter deck · {item.starter_deck.reduce((total, entry) => total + entry.quantity, 0)} cards</small></span>
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
                      <span className="enemy-art"><img src={assetUrl(item.asset)} alt="" /></span>
                      <span className="enemy-name"><strong>{item.name}</strong><small>{item.hp[0]}–{item.hp[1]} HP</small></span>
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
                <div><h3 id="loadout-title">Combat loadout</h3><span>{activeDeck.reduce((total, card) => total + card.quantity, 0)} cards</span></div>
                <div className="relic-summary">
                  <img
                    src={assetUrl(selectedRelic?.asset ?? null)}
                    alt=""
                  />
                  <span>{selectedRelic?.name ?? "No starter relic"}</span>
                </div>
              </div>
              <div className="deck-list">
                {activeDeck.map((card) => (
                  <div className="deck-row" key={card.name}>
                    <img src={card.asset} alt="" />
                    <span><strong>{card.name}</strong><small>{card.type}</small></span>
                    <b>×{card.quantity}</b>
                  </div>
                ))}
              </div>
              <div className="seed-presets" aria-label="Add reviewed proof-slice cards">
                <span>Add card</span>
                {availableProofCards.map((card) => {
                  const active = extraCards.includes(card.id);
                  return (
                    <button
                      type="button"
                      key={card.id}
                      className={active ? "active" : ""}
                      onClick={() => setExtraCards((values) => active
                        ? values.filter((id) => id !== card.id)
                        : [...values, card.id])}
                    >
                      {card.name}
                    </button>
                  );
                })}
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
                    <img className="result-character" src={assetUrl(selectedCharacter.asset)} alt="" />
                    <span>vs</span>
                    <img className="result-enemy" src={assetUrl(selectedEnemy.asset)} alt="" />
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
                        <img src={assetUrl(cards[card]?.asset ?? null)} alt={`${cards[card]?.name ?? card} card`} />
                        <span>{cards[card]?.name ?? card}</span>
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
                  <img src={assetUrl(selectedCharacter.asset)} alt="" />
                  <span>→</span>
                  <img src={assetUrl(selectedEnemy.asset)} alt="" />
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
