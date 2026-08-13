"use client";

import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import type {
  CombatReplay,
  ContentCard,
  ContentCharacter,
  ContentEnemy,
  EnemyIntent,
  ReplayCard,
  ReplayFrame,
  ReplayPower,
  TraceStep,
} from "../src/simulator";

type CombatReplayProps = {
  replay: CombatReplay;
  cards: Record<string, ContentCard>;
  character: ContentCharacter;
  enemy: ContentEnemy;
  trace: TraceStep[];
};

type InspectedCard = ReplayCard & { pile: string };

const SPEEDS = [0.5, 1, 2] as const;
const PILES = ["hand", "draw_pile", "discard_pile", "exhaust_pile", "play_pile"] as const;

function assetUrl(asset: string | null | undefined) {
  return asset ? `./${asset}` : "./favicon.svg";
}

function actionLabel(frame: ReplayFrame, cards: Record<string, ContentCard>) {
  const action = frame.action;
  if (!action) return "Combat ready";
  if (action.type === "card") return `Played ${cards[action.card_id]?.name ?? action.card_id}`;
  if (action.type === "end_turn") return "Ended turn";
  return `Selected ${action.selection.join(", ")}`;
}

function intentText(intent: EnemyIntent | null) {
  if (!intent) return "No intent";
  const effects = [
    intent.damage !== null ? `${intent.damage} damage` : null,
    intent.block !== null ? `${intent.block} block` : null,
    intent.power ? `${intent.power.name} ${intent.power.amount > 0 ? "+" : ""}${intent.power.amount}` : null,
  ].filter(Boolean);
  return effects.length ? effects.join(" · ") : "Utility move";
}

function powerList(powers: ReplayPower[]) {
  return powers.length ? powers.map((power) => `${power.name} ${power.amount}`).join(" · ") : "No powers";
}

function delta(value: number, previous: number, suffix = "") {
  const difference = value - previous;
  if (difference === 0) return null;
  return `${difference > 0 ? "+" : ""}${difference}${suffix}`;
}

export default function CombatReplayView({ replay, cards, character, enemy, trace }: CombatReplayProps) {
  const [index, setIndex] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState<(typeof SPEEDS)[number]>(1);
  const [inspected, setInspected] = useState<InspectedCard | null>(null);
  const replayRef = useRef<HTMLDivElement>(null);
  const frames = replay.frames;
  const frame = frames[index];
  const previous = index > 0 ? frames[index - 1] : null;
  const activeEnemy = frame.state.enemies[0];
  const previousEnemy = previous?.state.enemies[0];
  const displayedEnemy = activeEnemy ?? previousEnemy;

  useEffect(() => {
    setIndex(0);
    setPlaying(false);
  }, [replay]);

  useEffect(() => {
    if (!playing) return;
    if (index >= frames.length - 1) {
      setPlaying(false);
      return;
    }
    const timer = window.setTimeout(() => setIndex((current) => current + 1), 900 / speed);
    return () => window.clearTimeout(timer);
  }, [frames.length, index, playing, speed]);

  useEffect(() => {
    if (!inspected) return;
    const close = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") setInspected(null);
    };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, [inspected]);

  const groupedFrames = useMemo(() => {
    const groups = new Map<number, ReplayFrame[]>();
    for (const item of frames) {
      const group = groups.get(item.turn) ?? [];
      group.push(item);
      groups.set(item.turn, group);
    }
    return [...groups.entries()];
  }, [frames]);

  const changes = useMemo(() => {
    if (!previous) return ["Initial combat state"];
    const values = [
      delta(frame.state.player.hp, previous.state.player.hp, " player HP"),
      delta(frame.state.player.block, previous.state.player.block, " player block"),
      delta(frame.state.player.energy, previous.state.player.energy, " energy"),
      previousEnemy ? delta(activeEnemy?.hp ?? 0, previousEnemy.hp, " enemy HP") : null,
      previousEnemy ? delta(activeEnemy?.block ?? 0, previousEnemy.block, " enemy block") : null,
      ...PILES.map((pile) => delta(frame.state[pile].length, previous.state[pile].length, ` ${pile.replace("_", " ")}`)),
    ].filter((value): value is string => Boolean(value));
    return values.length ? values : ["State advanced without visible stat changes"];
  }, [activeEnemy, frame, previous, previousEnemy]);

  function jump(next: number) {
    setPlaying(false);
    setIndex(Math.min(frames.length - 1, Math.max(0, next)));
  }

  function togglePlayback() {
    if (index >= frames.length - 1) setIndex(0);
    setPlaying((current) => !current);
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.target instanceof HTMLButtonElement || event.target instanceof HTMLDetailsElement) return;
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      jump(index - 1);
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      jump(index + 1);
    } else if (event.key === " ") {
      event.preventDefault();
      togglePlayback();
    }
  }

  function inspect(card: ReplayCard, pile: string) {
    setPlaying(false);
    setInspected({ ...card, pile });
  }

  return (
    <section
      className="combat-replay"
      aria-label="Interactive combat replay"
      tabIndex={0}
      ref={replayRef}
      onKeyDown={handleKeyDown}
    >
      <div className="replay-heading">
        <div><h3>Combat replay</h3><span>Turn-by-turn optimal line</span></div>
        <span className="replay-position">Step {index} / {frames.length - 1}</span>
      </div>

      <div className="replay-layout">
        <nav className="replay-timeline" aria-label="Replay timeline">
          {groupedFrames.map(([turn, turnFrames]) => (
            <section className="timeline-turn" key={turn}>
              <h4>Turn {turn}</h4>
              {turnFrames.map((item) => {
                const cardId = item.action?.type === "card" ? item.action.card_id : null;
                return (
                  <button
                    className={item.index === index ? "active" : ""}
                    type="button"
                    key={item.index}
                    aria-current={item.index === index ? "step" : undefined}
                    onClick={() => jump(item.index)}
                  >
                    <span className="timeline-index">{item.index}</span>
                    {cardId ? <img src={assetUrl(cards[cardId]?.asset)} alt="" /> : <span className="timeline-icon">{item.action?.type === "end_turn" ? "↻" : "●"}</span>}
                    <span>
                      <strong>{actionLabel(item, cards)}</strong>
                      <small>{item.resolved_enemy_intents.length ? `Enemy: ${intentText(item.resolved_enemy_intents[0])}` : `After action · ${item.state.player.hp} HP`}</small>
                    </span>
                  </button>
                );
              })}
            </section>
          ))}
        </nav>

        <div className="replay-stage-column">
          <div className="replay-action-banner">
            <span>Turn {frame.turn} · Step {frame.index}</span>
            <strong>{actionLabel(frame, cards)}</strong>
            <small>{changes.join(" · ")}</small>
          </div>

          <div className={`battle-stage ${frame.state.status}`}>
            <article className="combatant player-combatant">
              <img src={assetUrl(character.asset)} alt={character.name} />
              <div className="combatant-name"><strong>{character.name}</strong><span>{powerList(frame.state.player.powers)}</span></div>
              <div className="health-bar"><span style={{ width: `${Math.max(0, frame.state.player.hp / frame.state.player.max_hp * 100)}%` }} /></div>
              <div className="combatant-stats">
                <b>{frame.state.player.hp}/{frame.state.player.max_hp} HP</b>
                <span>{frame.state.player.block} Block</span>
                <span>{frame.state.player.energy}/{frame.state.player.max_energy} Energy</span>
              </div>
            </article>

            <div className="battle-center">
              <span>Turn {frame.state.turn}</span>
              {frame.state.status === "won" ? <strong className="terminal-intent">Victory</strong> : frame.state.status === "lost" ? <strong className="terminal-intent">Defeated</strong> : (
                <div className="intent-card">
                  <small>Enemy intent</small>
                  <strong>{activeEnemy?.current_intent?.name ?? "Unknown"}</strong>
                  <span>{intentText(activeEnemy?.current_intent ?? null)}</span>
                </div>
              )}
            </div>

            <article className="combatant enemy-combatant">
              <img src={assetUrl(enemy.asset)} alt={enemy.name} />
              <div className="combatant-name"><strong>{enemy.name}</strong><span>{activeEnemy ? powerList(activeEnemy.powers) : "Defeated"}</span></div>
              <div className="health-bar enemy-health"><span style={{ width: `${activeEnemy ? Math.max(0, activeEnemy.hp / activeEnemy.max_hp * 100) : 0}%` }} /></div>
              <div className="combatant-stats">
                <b>{activeEnemy?.hp ?? 0}/{displayedEnemy?.max_hp ?? 0} HP</b>
                <span>{activeEnemy?.block ?? 0} Block</span>
              </div>
            </article>
          </div>

          <div className="replay-hand-section">
            <div className="section-title"><h3>Current hand</h3><span>{frame.state.hand.length} cards</span></div>
            {frame.state.hand.length ? (
              <div className="replay-hand">
                {frame.state.hand.map((card) => (
                  <button type="button" key={card.instance_id} onClick={() => inspect(card, "Hand")}>
                    <span className="card-cost">{card.effective_cost}</span>
                    <img src={assetUrl(cards[card.card_id]?.asset)} alt={cards[card.card_id]?.name ?? card.card_id} />
                    <strong>{cards[card.card_id]?.name ?? card.card_id}{card.upgrade_level ? "+" : ""}</strong>
                  </button>
                ))}
              </div>
            ) : <p className="empty-hand">No cards in hand at this step.</p>}
          </div>

          <div className="replay-controls" aria-label="Replay controls">
            <button type="button" aria-label="Previous step" onClick={() => jump(index - 1)} disabled={index === 0}>←</button>
            <button className="play-control" type="button" onClick={togglePlayback}>{playing ? "Pause" : index === frames.length - 1 ? "Replay" : "Play"}</button>
            <button type="button" aria-label="Next step" onClick={() => jump(index + 1)} disabled={index === frames.length - 1}>→</button>
            <div className="speed-controls" aria-label="Playback speed">
              {SPEEDS.map((value) => <button type="button" className={speed === value ? "active" : ""} key={value} onClick={() => setSpeed(value)}>{value}×</button>)}
            </div>
            <span>← → navigate · Space plays</span>
          </div>

          <details className="replay-details">
            <summary>State details and raw trace</summary>
            <div className="state-identity"><span>State hash</span><code>{frame.state.state_id}</code></div>
            <div className="pile-grid">
              {PILES.map((pile) => (
                <section key={pile}>
                  <h4>{pile.replace("_", " ")} <span>{frame.state[pile].length}</span></h4>
                  {frame.state[pile].length ? frame.state[pile].map((card) => (
                    <button type="button" key={card.instance_id} onClick={() => inspect(card, pile.replace("_", " "))}>
                      <span>{cards[card.card_id]?.name ?? card.card_id}{card.upgrade_level ? "+" : ""}</span>
                      <code>{card.effective_cost}e · #{card.instance_id}</code>
                    </button>
                  )) : <p>Empty</p>}
                </section>
              ))}
            </div>
            <div className="raw-trace">
              <h4>Legacy action trace</h4>
              {trace.map((step, traceIndex) => (
                <div key={`${step.state_hash}-${traceIndex}`}><span>{traceIndex + 1}. {step.action.card_id ? cards[step.action.card_id]?.name ?? step.action.card_id : step.action.type.replace("_", " ")}</span><code>{step.state_hash.slice(0, 12)}</code></div>
              ))}
            </div>
          </details>
        </div>
      </div>

      {inspected && (
        <div className="card-preview-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setInspected(null); }}>
          <section className="card-preview" role="dialog" aria-modal="true" aria-labelledby="card-preview-title">
            <button className="card-preview-close" type="button" aria-label="Close card details" autoFocus onClick={() => setInspected(null)}>×</button>
            <img src={assetUrl(cards[inspected.card_id]?.asset)} alt="" />
            <div>
              <span>{inspected.pile}</span>
              <h3 id="card-preview-title">{cards[inspected.card_id]?.name ?? inspected.card_id}{inspected.upgrade_level ? "+" : ""}</h3>
              <p>{cards[inspected.card_id]?.card_type ?? "Card"} · {inspected.upgrade_level ? "Upgraded" : "Base"}</p>
              <dl><dt>Effective cost</dt><dd>{inspected.effective_cost}</dd><dt>Instance</dt><dd>{inspected.instance_id}</dd><dt>Card ID</dt><dd>{inspected.card_id}</dd></dl>
            </div>
          </section>
        </div>
      )}
    </section>
  );
}
