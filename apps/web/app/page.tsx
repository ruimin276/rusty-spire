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
} from "../src/simulator";
import CombatStateEditor from "./combat-state-editor";
import CombatReplayView from "./combat-replay";

type CharacterId = string;
type EnemyId = string;
type CardScope = "character" | "neutral";

type DeckItem = {
  id: string;
  name: string;
  type: string;
  quantity: number;
  upgradeLevel: number;
  asset: string;
};

const quickSeeds = [1, 2, 4, 7, 17, 42];

function shortNumber(value: number) {
  return new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

function assetUrl(asset: string | null) {
  return asset ? `./${asset}` : "./favicon.svg";
}

function buildSetup(
  manifest: ContentManifest,
  character: ContentCharacter,
  enemy: ContentEnemy,
  deck: CombatSetupV2["deck"],
  characterHp: { current_hp: number; max_hp: number },
  relicIds: string[],
  enemyHp: { current_hp: number; max_hp: number },
  seed: number,
  ascension: number,
): CombatSetupV2 {
  return {
    schema_version: 2,
    package: manifest.package,
    ascension_level: ascension,
    rng: { run_seed: String(seed), profile: "isolated_combat_xoshiro_v1" },
    character: {
      id: character.id,
      current_hp: characterHp.current_hp,
      max_hp: characterHp.max_hp,
    },
    deck,
    relics: relicIds.map((id) => ({ id })),
    potions: [],
    encounter: {
      type: "custom",
      enemies: [{ id: enemy.id, current_hp: enemyHp.current_hp, max_hp: enemyHp.max_hp }],
    },
  };
}

function AppHeader({ manifest }: { manifest?: ContentManifest }) {
  return (
    <header className="app-header">
      <a className="brand" href="#top" aria-label="SLS2 Combat Lab home">
        <span className="brand-mark">S2</span>
        <span><strong>Combat Lab</strong><small>Slay the Spire 2</small></span>
      </a>
      <div className="header-actions">
        <div
          className="runtime-status"
          title={manifest ? `${manifest.package.package_id} ${manifest.package.sha256}` : "Loading combat package"}
        >
          <span className="status-dot" /> {manifest ? "Local Rust/WASM" : "Loading engine"}
        </div>
        <a
          className="github-link"
          href="https://github.com/ruimin276/rusty-spire"
          target="_blank"
          rel="noreferrer"
        >
          GitHub <span aria-hidden="true">↗</span>
        </a>
      </div>
    </header>
  );
}

export default function Home() {
  const [manifest, setManifest] = useState<ContentManifest | null>(null);
  const [manifestError, setManifestError] = useState<string | null>(null);
  const [character, setCharacter] = useState<CharacterId>("CHARACTER.SILENT");
  const [enemy, setEnemy] = useState<EnemyId>("MONSTER.NIBBIT");
  const [seed, setSeed] = useState(1);
  const [decksByCharacter, setDecksByCharacter] = useState<Record<CharacterId, CombatSetupV2["deck"]>>({});
  const [customCharacterHp, setCustomCharacterHp] = useState<CombatSetupV2["character"] | null>(null);
  const [customRelics, setCustomRelics] = useState<string[] | null>(null);
  const [customEnemyHp, setCustomEnemyHp] = useState<{ current_hp: number; max_hp: number } | null>(null);
  const [ascension, setAscension] = useState(0);
  const [quickCardScope, setQuickCardScope] = useState<CardScope>("character");
  const [quickCardQuery, setQuickCardQuery] = useState("");
  const [stateEditorOpen, setStateEditorOpen] = useState(false);
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
  const selectedCharacter = characters[character];
  const selectedEnemy = enemies[enemy];
  const activeCharacterHp = customCharacterHp ?? {
    id: character,
    current_hp: selectedCharacter?.max_hp ?? 1,
    max_hp: selectedCharacter?.max_hp ?? 1,
  };
  const activeRelicIds = customRelics ?? selectedCharacter?.starter_relics ?? [];
  const selectedRelics = activeRelicIds.map((id) => relics[id]).filter(Boolean);
  const defaultEnemyHp = useMemo(() => {
    if (!selectedEnemy) return { current_hp: 1, max_hp: 1 };
    const range = ascension >= 8 ? selectedEnemy.ascension_hp : selectedEnemy.hp;
    const hp = Math.floor((range[0] + range[1]) / 2);
    return { current_hp: hp, max_hp: hp };
  }, [ascension, selectedEnemy]);
  const activeEnemyHp = customEnemyHp ?? defaultEnemyHp;
  const activeDeckEntries = decksByCharacter[character] ?? selectedCharacter?.starter_deck ?? [];
  const activeDeck = useMemo<DeckItem[]>(() => {
    return activeDeckEntries.map((entry) => {
      const card = cards[entry.id];
      return {
        id: entry.id,
        name: card?.name ?? entry.id,
        type: card?.card_type ?? "Card",
        quantity: entry.quantity,
        upgradeLevel: entry.upgrade_level,
        asset: assetUrl(card?.asset ?? null),
      };
    });
  }, [activeDeckEntries, cards]);
  const availableCards = (manifest?.cards ?? []).filter((card) => {
    const matchesScope = quickCardScope === "character"
      ? card.character === character
      : card.character === null;
    const query = quickCardQuery.trim().toLowerCase();
    const matchesQuery = !query
      || card.name.toLowerCase().includes(query)
      || card.id.toLowerCase().includes(query);
    return matchesScope
      && matchesQuery
      && !activeDeckEntries.some((entry) => entry.id === card.id);
  });
  const totalCards = activeDeck.reduce((total, card) => total + card.quantity, 0);
  const deckIsModified = decksByCharacter[character] !== undefined
    && JSON.stringify(activeDeckEntries) !== JSON.stringify(selectedCharacter?.starter_deck ?? []);
  const setup = useMemo(
    () => manifest && selectedCharacter && selectedEnemy
      ? buildSetup(
          manifest,
          selectedCharacter,
          selectedEnemy,
          activeDeckEntries,
          activeCharacterHp,
          activeRelicIds,
          activeEnemyHp,
          seed,
          ascension,
        )
      : null,
    [
      activeCharacterHp,
      activeDeckEntries,
      activeEnemyHp,
      activeRelicIds,
      ascension,
      manifest,
      seed,
      selectedCharacter,
      selectedEnemy,
    ],
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
  const proofLabel = isSolving
    ? "Search running"
    : result?.optimality_proven
      ? result.won ? "Optimal result" : "No winning line"
      : result ? "Search incomplete" : solveError ? "Solver error" : "Ready";

  if (!manifest || !selectedCharacter || !selectedEnemy || !setup) {
    return (
      <div className="app-shell" id="top">
        <AppHeader />
        <main className="app-main">
          <section className="loading-state">
            <h1>Loading combat package…</h1>
            {manifestError ? <p>{manifestError}</p> : <p>Initializing the local Rust/WASM engine.</p>}
          </section>
        </main>
      </div>
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

  function changeCardQuantity(cardId: string, upgradeLevel: number, delta: number) {
    setDecksByCharacter((current) => {
      const next = (current[character] ?? selectedCharacter.starter_deck).map((entry) => ({ ...entry }));
      const index = next.findIndex(
        (entry) => entry.id === cardId && entry.upgrade_level === upgradeLevel,
      );
      const currentQuantity = index >= 0 ? next[index].quantity : 0;
      const currentTotal = next.reduce((total, entry) => total + entry.quantity, 0);
      if (delta < 0 && currentQuantity === 1 && currentTotal === 1) return current;

      const nextQuantity = Math.min(99, Math.max(0, currentQuantity + delta));
      if (nextQuantity === 0 && index >= 0) next.splice(index, 1);
      else if (index >= 0) next[index].quantity = nextQuantity;
      else if (nextQuantity > 0) next.push({ id: cardId, quantity: nextQuantity, upgrade_level: upgradeLevel });
      return { ...current, [character]: next };
    });
  }

  function resetCurrentDeck() {
    setDecksByCharacter((current) => {
      const next = { ...current };
      delete next[character];
      return next;
    });
  }

  function applyCombatState(
    next: CombatSetupV2,
    characterDecks: Record<string, CombatSetupV2["deck"]>,
  ) {
    const nextEnemy = next.encounter.enemies[0];
    if (!characters[next.character.id] || !nextEnemy || !enemies[nextEnemy.id]) return;
    setCharacter(next.character.id);
    setCustomCharacterHp({ ...next.character });
    setDecksByCharacter(characterDecks);
    setCustomRelics(next.relics.map((relic) => relic.id));
    setEnemy(nextEnemy.id);
    setCustomEnemyHp({ current_hp: nextEnemy.current_hp, max_hp: nextEnemy.max_hp });
    setSeed(Number(next.rng.run_seed));
    setAscension(next.ascension_level);
    setStateEditorOpen(false);
  }

  return (
    <div className="app-shell" id="top">
      <AppHeader manifest={manifest} />
      <main className="app-main">
        <section className="workspace" id="workspace" aria-label="Combat solver workspace">
          <aside className="setup-panel">
            <div className="panel-heading">
              <div><span className="step-number">1</span><h2>Combat setup</h2></div>
              <div className="panel-actions">
                <button className="copy-button" type="button" onClick={copySetup}>
                  {copied ? "Copied" : "Copy JSON"}
                </button>
              </div>
            </div>

            <button className="full-editor-button" type="button" onClick={() => setStateEditorOpen(true)}>
              <span className="full-editor-icon" aria-hidden="true">☷</span>
              <span><strong>Edit full combat state</strong><small>Cards, upgrades, relics, HP and encounter</small></span>
              <b aria-hidden="true">→</b>
            </button>

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
                      onClick={() => {
                        if (id !== character) {
                          setCharacter(id);
                          setCustomCharacterHp(null);
                          setCustomRelics(null);
                          setQuickCardScope("character");
                          setQuickCardQuery("");
                        }
                      }}
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
                      onClick={() => {
                        setEnemy(id);
                        setCustomEnemyHp(null);
                      }}
                    >
                      <span className="enemy-art"><img src={assetUrl(item.asset)} alt="" /></span>
                      <span className="enemy-name"><strong>{item.name}</strong><small>{item.hp[0]}–{item.hp[1]} HP</small></span>
                      <span className="selection-mark" />
                    </button>
                  );
                })}
              </div>
            </fieldset>

            <div className="parameter-grid compact-parameters">
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
            </div>

            <div className="seed-presets" aria-label="Seed presets">
              <span>Presets</span>
              {quickSeeds.map((value) => (
                <button type="button" key={value} className={seed === value ? "active" : ""} onClick={() => setSeed(value)}>{value}</button>
              ))}
            </div>

            <fieldset className="quick-ascension">
              <legend>Ascension</legend>
              <div className="segmented-control ascension-options">
                {Array.from({ length: 11 }, (_, value) => (
                  <button
                    type="button"
                    key={value}
                    className={ascension === value ? "active" : ""}
                    aria-pressed={ascension === value}
                    onClick={() => setAscension(value)}
                  >A{value}</button>
                ))}
              </div>
            </fieldset>

            <section className="loadout" aria-labelledby="loadout-title">
              <div className="section-heading">
                <div><h3 id="loadout-title">Combat deck</h3><span>{totalCards} cards</span></div>
                <div className="relic-summary">
                  <img
                    src={assetUrl(selectedRelics[0]?.asset ?? null)}
                    alt=""
                  />
                  <span>
                    {selectedRelics.length === 0
                      ? "No relics"
                      : `${selectedRelics[0].name}${selectedRelics.length > 1 ? ` +${selectedRelics.length - 1}` : ""}`}
                  </span>
                </div>
              </div>
              <div className="deck-toolbar">
                <span>Adjust quantities for this simulation.</span>
                <button type="button" onClick={resetCurrentDeck} disabled={!deckIsModified}>
                  Reset starter deck
                </button>
              </div>
              <div className="deck-list">
                {activeDeck.map((card) => (
                  <div className="deck-row" key={`${card.id}-${card.upgradeLevel}`}>
                    <img src={card.asset} alt="" />
                    <span>
                      <strong>{card.name}{card.upgradeLevel === 1 ? "+" : ""}</strong>
                      <small>{card.type}{card.upgradeLevel === 1 ? " · Upgraded" : ""}</small>
                    </span>
                    <div className="deck-quantity" aria-label={`${card.name}${card.upgradeLevel === 1 ? " upgraded" : ""} quantity`}>
                      <button
                        type="button"
                        aria-label={`Remove one ${card.upgradeLevel === 1 ? "upgraded " : ""}${card.name}`}
                        onClick={() => changeCardQuantity(card.id, card.upgradeLevel, -1)}
                        disabled={totalCards === 1 && card.quantity === 1}
                      >
                        −
                      </button>
                      <output aria-label={`${card.name} count`}>{card.quantity}</output>
                      <button
                        type="button"
                        aria-label={`Add one ${card.upgradeLevel === 1 ? "upgraded " : ""}${card.name}`}
                        onClick={() => changeCardQuantity(card.id, card.upgradeLevel, 1)}
                        disabled={card.quantity >= 99}
                      >
                        +
                      </button>
                    </div>
                  </div>
                ))}
              </div>
              <div className="card-library" aria-label="Cards available to add">
                <div className="card-library-heading">
                  <strong>Add card</strong>
                  <span>Class cards shown by default</span>
                </div>
                <div className="quick-card-filters">
                  <div className="segmented-control" aria-label="Card color filter">
                    <button type="button" className={quickCardScope === "character" ? "active" : ""} aria-pressed={quickCardScope === "character"} onClick={() => setQuickCardScope("character")}>Class</button>
                    <button type="button" className={quickCardScope === "neutral" ? "active" : ""} aria-pressed={quickCardScope === "neutral"} onClick={() => setQuickCardScope("neutral")}>Neutral</button>
                  </div>
                  <input type="search" aria-label="Search cards to add" placeholder="Search cards" value={quickCardQuery} onChange={(event) => setQuickCardQuery(event.target.value)} />
                </div>
                {availableCards.length > 0 ? (
                  <div className="card-library-list">
                    {availableCards.map((card) => (
                      <button
                        type="button"
                        key={card.id}
                        aria-label={`Add ${card.name} to deck`}
                        onClick={() => changeCardQuantity(card.id, 0, 1)}
                      >
                        <img src={assetUrl(card.asset)} alt="" />
                        <span><strong>{card.name}</strong><small>{card.card_type ?? "Card"}</small></span>
                        <b aria-hidden="true">+</b>
                      </button>
                    ))}
                  </div>
                ) : <p className="card-library-empty">No matching cards available to add.</p>}
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
                <section className="result-overview">
                  <div className="compact-matchup" aria-hidden="true">
                    <img className="result-character" src={assetUrl(selectedCharacter.asset)} alt="" />
                    <span>vs</span>
                    <img className="result-enemy" src={assetUrl(selectedEnemy.asset)} alt="" />
                  </div>
                  <div className="result-copy">
                    <span className="success-label">Victory · optimum proven</span>
                    <div><h3>{winningResult.hpLoss === 0 ? "No HP lost" : `${winningResult.hpLoss} HP lost`}</h3><p>{selectedCharacter.name} finishes at <strong>{winningResult.finalHp} HP</strong>.</p></div>
                  </div>
                  <div className="metric-strip compact-metrics">
                    <div><span>HP</span><strong>{winningResult.finalHp}</strong></div>
                    <div><span>Turns</span><strong>{winningResult.turns}</strong></div>
                    <div><span>Actions</span><strong>{winningResult.actions}</strong></div>
                    <div><span>States</span><strong>{shortNumber(winningResult.explored)}</strong></div>
                    <div><span>Time</span><strong>{winningResult.runtime < 0.01 ? `${(winningResult.runtime * 1000).toFixed(1)}ms` : `${winningResult.runtime.toFixed(3)}s`}</strong></div>
                  </div>
                </section>

                {activeRun?.replay ? (
                  <CombatReplayView
                    replay={activeRun.replay}
                    cards={cards}
                    character={selectedCharacter}
                    enemy={selectedEnemy}
                    trace={activeRun.result.actions}
                  />
                ) : (
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
                )}
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
      </main>
      {stateEditorOpen && (
        <CombatStateEditor
          manifest={manifest}
          setup={setup}
          characterDecks={decksByCharacter}
          onApply={applyCombatState}
          onClose={() => setStateEditorOpen(false)}
        />
      )}
    </div>
  );
}
