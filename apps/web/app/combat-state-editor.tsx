"use client";

import { useEffect, useMemo, useState } from "react";
import type {
  CombatSetupV2,
  ContentCard,
  ContentEnemy,
  ContentManifest,
} from "../src/simulator";

type EditorSection = "player" | "deck" | "relics" | "encounter";
type DeckEntry = CombatSetupV2["deck"][number];

type CombatStateEditorProps = {
  manifest: ContentManifest;
  setup: CombatSetupV2;
  characterDecks: Record<string, CombatSetupV2["deck"]>;
  onApply: (setup: CombatSetupV2, characterDecks: Record<string, CombatSetupV2["deck"]>) => void;
  onClose: () => void;
};

const sections: Array<{ id: EditorSection; label: string }> = [
  { id: "player", label: "Player & run" },
  { id: "deck", label: "Deck" },
  { id: "relics", label: "Relics & potions" },
  { id: "encounter", label: "Encounter" },
];

function cloneSetup(setup: CombatSetupV2): CombatSetupV2 {
  return JSON.parse(JSON.stringify(setup)) as CombatSetupV2;
}

function assetUrl(asset: string | null) {
  return asset ? `./${asset}` : "./favicon.svg";
}

function midpointHp(enemy: ContentEnemy, ascension: number) {
  const range = ascension >= 8 ? enemy.ascension_hp : enemy.hp;
  return Math.floor((range[0] + range[1]) / 2);
}

function cardQuantity(deck: DeckEntry[], cardId: string, upgradeLevel: number) {
  return deck.find((entry) => entry.id === cardId && entry.upgrade_level === upgradeLevel)?.quantity ?? 0;
}

export default function CombatStateEditor({
  manifest,
  setup,
  characterDecks,
  onApply,
  onClose,
}: CombatStateEditorProps) {
  const [draft, setDraft] = useState<CombatSetupV2>(() => cloneSetup(setup));
  const [section, setSection] = useState<EditorSection>("player");
  const [cardQuery, setCardQuery] = useState("");
  const [cardType, setCardType] = useState("all");
  const [cardScope, setCardScope] = useState<"character" | "neutral" | "all">("character");
  const [deckCache, setDeckCache] = useState<Record<string, CombatSetupV2["deck"]>>(() => ({
    ...characterDecks,
    [setup.character.id]: setup.deck.map((entry) => ({ ...entry })),
  }));
  const [relicQuery, setRelicQuery] = useState("");
  const [enemyQuery, setEnemyQuery] = useState("");

  const characters = useMemo(
    () => Object.fromEntries(manifest.characters.map((item) => [item.id, item])),
    [manifest],
  );
  const enemies = useMemo(
    () => Object.fromEntries(manifest.enemies.map((item) => [item.id, item])),
    [manifest],
  );
  const characterNames = useMemo(
    () => Object.fromEntries(manifest.characters.map((item) => [item.id, item.name])),
    [manifest],
  );
  const selectedCharacter = characters[draft.character.id];
  const selectedEnemy = enemies[draft.encounter.enemies[0]?.id];
  const totalCards = draft.deck.reduce((total, entry) => total + entry.quantity, 0);

  const filteredCards = manifest.cards.filter((card) => {
    const query = cardQuery.trim().toLowerCase();
    const matchesQuery = !query
      || card.name.toLowerCase().includes(query)
      || card.id.toLowerCase().includes(query)
      || (card.character ? characterNames[card.character]?.toLowerCase().includes(query) : "colorless".includes(query));
    const matchesType = cardType === "all" || card.card_type === cardType;
    const matchesScope = cardScope === "character"
      ? card.character === draft.character.id
      : cardScope === "neutral"
        ? card.character === null
        : true;
    return matchesQuery && matchesType && matchesScope;
  });
  const filteredRelics = manifest.relics.filter((relic) => {
    const query = relicQuery.trim().toLowerCase();
    return !query || relic.name.toLowerCase().includes(query) || relic.id.toLowerCase().includes(query);
  });
  const filteredEnemies = manifest.enemies.filter((enemy) => {
    const query = enemyQuery.trim().toLowerCase();
    return !query || enemy.name.toLowerCase().includes(query) || enemy.id.toLowerCase().includes(query);
  });

  useEffect(() => {
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [onClose]);

  function updateDraft(change: (next: CombatSetupV2) => void) {
    setDraft((current) => {
      const next = cloneSetup(current);
      change(next);
      return next;
    });
  }

  function selectCharacter(characterId: string) {
    const character = characters[characterId];
    if (!character) return;
    setDeckCache((current) => ({
      ...current,
      [draft.character.id]: draft.deck.map((entry) => ({ ...entry })),
    }));
    updateDraft((next) => {
      next.character = {
        id: character.id,
        current_hp: character.max_hp,
        max_hp: character.max_hp,
      };
      next.deck = (deckCache[character.id] ?? character.starter_deck).map((entry) => ({ ...entry }));
      next.relics = character.starter_relics.map((id) => ({ id }));
    });
    setCardScope("character");
    setCardQuery("");
  }

  function changeCard(card: ContentCard, upgradeLevel: number, delta: number) {
    const compatible = card.character === null || card.character === draft.character.id;
    if (!compatible) return;
    updateDraft((next) => {
      const index = next.deck.findIndex(
        (entry) => entry.id === card.id && entry.upgrade_level === upgradeLevel,
      );
      const currentQuantity = index >= 0 ? next.deck[index].quantity : 0;
      if (delta < 0 && currentQuantity === 1 && totalCards === 1) return;
      const nextQuantity = Math.min(99, Math.max(0, currentQuantity + delta));
      if (nextQuantity === 0 && index >= 0) next.deck.splice(index, 1);
      else if (index >= 0) next.deck[index].quantity = nextQuantity;
      else if (nextQuantity > 0) next.deck.push({ id: card.id, quantity: nextQuantity, upgrade_level: upgradeLevel });
    });
  }

  function toggleRelic(relicId: string) {
    updateDraft((next) => {
      const index = next.relics.findIndex((relic) => relic.id === relicId);
      if (index >= 0) next.relics.splice(index, 1);
      else next.relics.push({ id: relicId });
    });
  }

  function selectEnemy(enemy: ContentEnemy) {
    updateDraft((next) => {
      const hp = midpointHp(enemy, next.ascension_level);
      next.encounter.enemies = [{ id: enemy.id, current_hp: hp, max_hp: hp }];
    });
  }

  return (
    <div className="state-editor-backdrop" onMouseDown={(event) => {
      if (event.currentTarget === event.target) onClose();
    }}>
      <section className="state-editor" role="dialog" aria-modal="true" aria-labelledby="state-editor-title">
        <header className="state-editor-header">
          <div>
            <span className="state-editor-kicker">Advanced setup</span>
            <h2 id="state-editor-title">Edit combat state</h2>
          </div>
          <button type="button" className="state-editor-close" onClick={onClose} aria-label="Close combat state editor">×</button>
        </header>

        <div className="state-editor-body">
          <nav className="state-editor-nav" aria-label="Combat state sections">
            {sections.map((item) => (
              <button
                type="button"
                key={item.id}
                className={section === item.id ? "active" : ""}
                aria-current={section === item.id ? "page" : undefined}
                onClick={() => setSection(item.id)}
              >
                <span>{item.label}</span>
                {item.id === "deck" && <b>{totalCards}</b>}
                {item.id === "relics" && <b>{draft.relics.length}</b>}
                {item.id === "encounter" && <b>1</b>}
              </button>
            ))}
            <div className="state-editor-nav-note">
              <strong>Local draft</strong>
              <span>Changes apply only when you confirm.</span>
            </div>
          </nav>

          <div className="state-editor-content">
            {section === "player" && selectedCharacter && (
              <section className="editor-section" aria-labelledby="editor-player-title">
                <div className="editor-section-heading">
                  <div><span>Starting state</span><h3 id="editor-player-title">Player & run</h3></div>
                  <p>Each character keeps its own deck draft while you switch.</p>
                </div>

                <div className="editor-character-grid">
                  {manifest.characters.map((character) => (
                    <button
                      type="button"
                      key={character.id}
                      className={draft.character.id === character.id ? "active" : ""}
                      aria-pressed={draft.character.id === character.id}
                      onClick={() => selectCharacter(character.id)}
                    >
                      <img src={assetUrl(character.asset)} alt="" />
                      <span><strong>{character.name}</strong><small>{character.max_hp} base HP · {character.max_energy} energy</small></span>
                    </button>
                  ))}
                </div>

                <div className="editor-field-grid">
                  <label>
                    <span>Current HP</span>
                    <input
                      type="number"
                      min="1"
                      max={draft.character.max_hp}
                      value={draft.character.current_hp}
                      onChange={(event) => updateDraft((next) => {
                        next.character.current_hp = Math.min(next.character.max_hp, Math.max(1, Number(event.target.value)));
                      })}
                    />
                  </label>
                  <label>
                    <span>Maximum HP</span>
                    <input
                      type="number"
                      min="1"
                      value={draft.character.max_hp}
                      onChange={(event) => updateDraft((next) => {
                        next.character.max_hp = Math.max(1, Number(event.target.value));
                        next.character.current_hp = Math.min(next.character.current_hp, next.character.max_hp);
                      })}
                    />
                  </label>
                  <label>
                    <span>Run seed</span>
                    <input
                      type="number"
                      min="0"
                      max="4294967295"
                      value={draft.rng.run_seed}
                      onChange={(event) => updateDraft((next) => {
                        next.rng.run_seed = String(Math.min(4_294_967_295, Math.max(0, Number(event.target.value))));
                      })}
                    />
                  </label>
                  <div className="editor-field-group editor-ascension-group">
                    <span>Ascension</span>
                    <div className="segmented-control editor-ascension-options">
                      {Array.from({ length: 11 }, (_, value) => (
                        <button
                          type="button"
                          key={value}
                          className={draft.ascension_level === value ? "active" : ""}
                          aria-pressed={draft.ascension_level === value}
                          onClick={() => updateDraft((next) => { next.ascension_level = value; })}
                        >A{value}</button>
                      ))}
                    </div>
                  </div>
                </div>
              </section>
            )}

            {section === "deck" && (
              <section className="editor-section" aria-labelledby="editor-deck-title">
                <div className="editor-section-heading">
                  <div><span>Card catalog</span><h3 id="editor-deck-title">Deck</h3></div>
                  <p>{totalCards} cards · base and upgraded copies are tracked separately.</p>
                </div>

                <div className="editor-filter-stack">
                  <label className="editor-search">
                    <span aria-hidden="true">⌕</span>
                    <input
                      type="search"
                      placeholder="Search cards…"
                      value={cardQuery}
                      onChange={(event) => setCardQuery(event.target.value)}
                    />
                  </label>
                  <div className="editor-filter-groups">
                    <div className="segmented-control" aria-label="Filter cards by color">
                      {([ ["character", "Class"], ["neutral", "Neutral"], ["all", "All cards"] ] as const).map(([value, label]) => (
                        <button type="button" key={value} className={cardScope === value ? "active" : ""} aria-pressed={cardScope === value} onClick={() => setCardScope(value)}>{label}</button>
                      ))}
                    </div>
                    <div className="segmented-control" aria-label="Filter cards by type">
                      {([ ["all", "All types"], ["Attack", "Attacks"], ["Skill", "Skills"], ["Power", "Powers"] ] as const).map(([value, label]) => (
                        <button type="button" key={value} className={cardType === value ? "active" : ""} aria-pressed={cardType === value} onClick={() => setCardType(value)}>{label}</button>
                      ))}
                    </div>
                  </div>
                </div>

                <div className="editor-card-catalog">
                  {filteredCards.map((card) => {
                    const compatible = card.character === null || card.character === draft.character.id;
                    const baseQuantity = cardQuantity(draft.deck, card.id, 0);
                    const upgradedQuantity = cardQuantity(draft.deck, card.id, 1);
                    return (
                      <article className={`editor-card ${compatible ? "" : "incompatible"}`} key={card.id}>
                        <img src={assetUrl(card.asset)} alt="" />
                        <div className="editor-card-copy">
                          <strong>{card.name}</strong>
                          <span>{card.card_type ?? "Card"} · {card.cost} energy · {card.character ? characterNames[card.character] ?? card.character : "Colorless"}</span>
                          {!compatible && <small>Not compatible with {selectedCharacter?.name}</small>}
                        </div>
                        <div className="editor-card-levels">
                          {[{ level: 0, label: "Base", quantity: baseQuantity }, { level: 1, label: "Upgraded", quantity: upgradedQuantity }].map((variant) => (
                            <div key={variant.level}>
                              <span>{variant.label}</span>
                              <div className="editor-stepper">
                                <button
                                  type="button"
                                  aria-label={`Remove one ${variant.label.toLowerCase()} ${card.name}`}
                                  onClick={() => changeCard(card, variant.level, -1)}
                                  disabled={!compatible || variant.quantity === 0 || (totalCards === 1 && variant.quantity === 1)}
                                >−</button>
                                <output aria-label={`${variant.label} ${card.name} count`}>{variant.quantity}</output>
                                <button
                                  type="button"
                                  aria-label={`Add one ${variant.label.toLowerCase()} ${card.name}`}
                                  onClick={() => changeCard(card, variant.level, 1)}
                                  disabled={!compatible || variant.quantity >= 99}
                                >+</button>
                              </div>
                            </div>
                          ))}
                        </div>
                      </article>
                    );
                  })}
                  {filteredCards.length === 0 && <p className="editor-no-results">No cards match this filter.</p>}
                </div>
              </section>
            )}

            {section === "relics" && (
              <section className="editor-section" aria-labelledby="editor-relics-title">
                <div className="editor-section-heading">
                  <div><span>Loadout</span><h3 id="editor-relics-title">Relics & potions</h3></div>
                  <p>{draft.relics.length} relics selected.</p>
                </div>

                <div className="editor-filter-row">
                  <label className="editor-search">
                    <span aria-hidden="true">⌕</span>
                    <input
                      type="search"
                      placeholder="Search every relic…"
                      value={relicQuery}
                      onChange={(event) => setRelicQuery(event.target.value)}
                    />
                  </label>
                  <div className="editor-bulk-actions">
                    <button type="button" onClick={() => updateDraft((next) => {
                      next.relics = manifest.relics.map((relic) => ({ id: relic.id }));
                    })}>Select all</button>
                    <button type="button" onClick={() => updateDraft((next) => { next.relics = []; })}>Clear</button>
                  </div>
                </div>

                <div className="editor-relic-grid">
                  {filteredRelics.map((relic) => {
                    const selected = draft.relics.some((item) => item.id === relic.id);
                    return (
                      <button
                        type="button"
                        key={relic.id}
                        className={selected ? "active" : ""}
                        role="checkbox"
                        aria-checked={selected}
                        onClick={() => toggleRelic(relic.id)}
                      >
                        <img src={assetUrl(relic.asset)} alt="" />
                        <span><strong>{relic.name}</strong><small>{selected ? "Included" : "Not included"}</small></span>
                        <b aria-hidden="true">{selected ? "✓" : "+"}</b>
                      </button>
                    );
                  })}
                </div>

                <div className="editor-unavailable">
                  <div className="editor-unavailable-icon">!</div>
                  <div>
                    <strong>Potions are not executable yet</strong>
                    <p>The current combat contract requires an empty potion list, so this editor will not create an invalid potion state.</p>
                  </div>
                  <span>Unavailable</span>
                </div>
              </section>
            )}

            {section === "encounter" && selectedEnemy && (
              <section className="editor-section" aria-labelledby="editor-encounter-title">
                <div className="editor-section-heading">
                  <div><span>Single-enemy combat</span><h3 id="editor-encounter-title">Encounter</h3></div>
                  <p>Select any executable enemy and customize its starting HP.</p>
                </div>

                <label className="editor-search editor-search-wide">
                  <span aria-hidden="true">⌕</span>
                  <input
                    type="search"
                    placeholder="Search every enemy…"
                    value={enemyQuery}
                    onChange={(event) => setEnemyQuery(event.target.value)}
                  />
                </label>

                <div className="editor-enemy-grid">
                  {filteredEnemies.map((enemy) => {
                    const selected = draft.encounter.enemies[0]?.id === enemy.id;
                    return (
                      <button
                        type="button"
                        key={enemy.id}
                        className={selected ? "active" : ""}
                        aria-pressed={selected}
                        onClick={() => selectEnemy(enemy)}
                      >
                        <img src={assetUrl(enemy.asset)} alt="" />
                        <span><strong>{enemy.name}</strong><small>{enemy.hp[0]}–{enemy.hp[1]} HP · A8+ {enemy.ascension_hp[0]}–{enemy.ascension_hp[1]}</small></span>
                        <b aria-hidden="true">{selected ? "✓" : ""}</b>
                      </button>
                    );
                  })}
                </div>

                <div className="editor-field-grid editor-enemy-hp">
                  <label>
                    <span>Current enemy HP</span>
                    <input
                      type="number"
                      min="1"
                      max={draft.encounter.enemies[0].max_hp}
                      value={draft.encounter.enemies[0].current_hp}
                      onChange={(event) => updateDraft((next) => {
                        const enemy = next.encounter.enemies[0];
                        enemy.current_hp = Math.min(enemy.max_hp, Math.max(1, Number(event.target.value)));
                      })}
                    />
                  </label>
                  <label>
                    <span>Maximum enemy HP</span>
                    <input
                      type="number"
                      min="1"
                      value={draft.encounter.enemies[0].max_hp}
                      onChange={(event) => updateDraft((next) => {
                        const enemy = next.encounter.enemies[0];
                        enemy.max_hp = Math.max(1, Number(event.target.value));
                        enemy.current_hp = Math.min(enemy.current_hp, enemy.max_hp);
                      })}
                    />
                  </label>
                </div>

                <div className="editor-unavailable compact">
                  <div className="editor-unavailable-icon">1</div>
                  <div>
                    <strong>One enemy per simulation</strong>
                    <p>The current deterministic engine rejects multi-enemy starting states.</p>
                  </div>
                </div>
              </section>
            )}
          </div>
        </div>

        <footer className="state-editor-footer">
          <div>
            <strong>{selectedCharacter?.name}</strong>
            <span>{totalCards} cards · {draft.relics.length} relics · {selectedEnemy?.name}</span>
          </div>
          <div>
            <button type="button" className="editor-cancel" onClick={onClose}>Cancel</button>
            <button type="button" className="editor-apply" onClick={() => onApply(draft, {
              ...deckCache,
              [draft.character.id]: draft.deck.map((entry) => ({ ...entry })),
            })}>Apply combat state</button>
          </div>
        </footer>
      </section>
    </div>
  );
}
