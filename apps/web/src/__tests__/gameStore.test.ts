import { beforeEach, describe, expect, it } from "vitest";
import type { DynamicViewDirective, PendingInterruptDTO } from "../api/types";
import { useGameStore } from "../stores/gameStore";

function makeDirective(
  overrides: Partial<DynamicViewDirective> & Pick<DynamicViewDirective, "directiveId">,
): DynamicViewDirective {
  return {
    schemaVersion: 1,
    sessionId: "sess-1",
    turnId: "turn-1",
    narrationGeneration: 1,
    source: "kernel_owned",
    directiveType: "evidence_unlocked",
    displayTiming: "after_narration",
    payload: {
      clueId: "clue-1",
      title: "书房暗格的封蜡残片",
      summary: "与委托信上的火漆印纹吻合。",
      sourceLabel: "二楼书房",
    },
    ...overrides,
  };
}

describe("gameStore stream & directive invariants", () => {
  beforeEach(() => {
    window.localStorage.clear();
    useGameStore.setState({
      sessionId: "sess-1",
      theme: "light",
      experienceMode: "classic",
      lowSanEffectsEnabled: true,
      activeDrawer: null,
      connectionStatus: "idle",
      activeTurn: null,
      pendingInterrupt: null,
      displayedDirectiveIds: {},
      newlyReleasedDirectiveIds: [],
    });
  });

  it("holds after_narration directives pending until narration_complete, then releases in seq order and records newlyReleasedDirectiveIds", () => {
    const store = useGameStore.getState();
    store.beginTurn("turn-1", "检查红木书桌抽屉");

    const dSecond = makeDirective({
      directiveId: "dir-handout-2",
      directiveType: "diegetic_handout",
      seq: 20,
      payload: {
        handoutId: "h-1",
        title: "残缺的来信",
        presentationText: "切勿在午夜后前往温室。",
      },
    });
    const dFirst = makeDirective({
      directiveId: "dir-clue-1",
      seq: 10,
    });

    store.applyStreamEvent({ type: "ui_directive", directive: dSecond });
    store.applyStreamEvent({ type: "ui_directive", directive: dFirst });
    store.applyStreamEvent({
      type: "narration_delta",
      turnId: "turn-1",
      generationId: 1,
      chunk: "你拉开抽屉，暗层里露出一角泛黄信纸。",
    });

    const beforeComplete = useGameStore.getState();
    expect(beforeComplete.activeTurn?.pendingAfterNarration).toHaveLength(2);
    expect(beforeComplete.activeTurn?.releasedAfterNarration).toHaveLength(0);
    expect(beforeComplete.newlyReleasedDirectiveIds).toEqual([]);

    // Skipping visual typewriter before narration_complete must NOT prematurely release after_narration directives
    store.skipTypewriter();
    expect(useGameStore.getState().activeTurn?.typewriterSkipped).toBe(true);
    expect(useGameStore.getState().activeTurn?.releasedAfterNarration).toHaveLength(0);

    store.applyStreamEvent({
      type: "narration_complete",
      turnId: "turn-1",
      generationId: 1,
    });

    const afterComplete = useGameStore.getState();
    expect(afterComplete.activeTurn?.narrationComplete).toBe(true);
    expect(afterComplete.activeTurn?.pendingAfterNarration).toHaveLength(0);
    expect(afterComplete.activeTurn?.releasedAfterNarration.map((d) => d.directiveId)).toEqual([
      "dir-clue-1",
      "dir-handout-2",
    ]);
    expect(afterComplete.newlyReleasedDirectiveIds).toEqual(["dir-clue-1", "dir-handout-2"]);

    const consumed = store.consumeNewlyReleasedDirectives();
    expect(consumed).toEqual(["dir-clue-1", "dir-handout-2"]);
    expect(useGameStore.getState().newlyReleasedDirectiveIds).toEqual([]);
  });

  it("clears partial text on narration_reset, rebinds kernel_owned pending directives to new generationId, and ignores stale generation chunks", () => {
    const store = useGameStore.getState();
    store.beginTurn("turn-1", "推开书房暗门");

    const kernelDir = makeDirective({
      directiveId: "dir-kernel-clock",
      source: "kernel_owned",
      narrationGeneration: 1,
      directiveType: "clock_pulse",
    });
    const narrativeDir = makeDirective({
      directiveId: "dir-narrative-card",
      source: "narrative_owned",
      narrationGeneration: 1,
      directiveType: "narrative_card",
      payload: { title: "冷风", body: "旧世代的半截描写" },
    });

    store.applyStreamEvent({ type: "ui_directive", directive: kernelDir });
    store.applyStreamEvent({ type: "ui_directive", directive: narrativeDir });
    store.applyStreamEvent({
      type: "narration_delta",
      turnId: "turn-1",
      generationId: 1,
      chunk: "半截中断的旧叙事……",
    });

    store.applyStreamEvent({
      type: "recovery_started",
      turnId: "turn-1",
      generationId: 2,
    });
    store.applyStreamEvent({
      type: "narration_reset",
      turnId: "turn-1",
      generationId: 2,
      clearPartial: true,
    });

    const afterReset = useGameStore.getState().activeTurn!;
    expect(afterReset.generationId).toBe(2);
    expect(afterReset.narrationText).toBe("");
    expect(afterReset.pendingAfterNarration).toHaveLength(1);
    expect(afterReset.pendingAfterNarration[0].directiveId).toBe("dir-kernel-clock");
    expect(afterReset.pendingAfterNarration[0].narrationGeneration).toBe(2);

    // Late delta & complete from stale generation 1 must be ignored
    store.applyStreamEvent({
      type: "narration_delta",
      turnId: "turn-1",
      generationId: 1,
      chunk: "迟到的旧世代残片",
    });
    store.applyStreamEvent({
      type: "narration_complete",
      turnId: "turn-1",
      generationId: 1,
    });
    expect(useGameStore.getState().activeTurn?.narrationText).toBe("");
    expect(useGameStore.getState().activeTurn?.narrationComplete).toBe(false);

    // New generation 2 streams and completes cleanly
    store.applyStreamEvent({
      type: "narration_delta",
      turnId: "turn-1",
      generationId: 2,
      chunk: "暗门后是一道向下延伸的潮湿石阶。",
    });
    store.applyStreamEvent({
      type: "narration_complete",
      turnId: "turn-1",
      generationId: 2,
    });

    const finalTurn = useGameStore.getState().activeTurn!;
    expect(finalTurn.narrationText).toBe("暗门后是一道向下延伸的潮湿石阶。");
    expect(finalTurn.releasedAfterNarration.map((d) => d.directiveId)).toEqual([
      "dir-kernel-clock",
    ]);
  });

  it("deduplicates repeated directiveId deliveries idempotently", () => {
    const store = useGameStore.getState();
    store.beginTurn("turn-1", "翻看账册");

    const dir = makeDirective({ directiveId: "dir-dup-1", seq: 1 });
    store.applyStreamEvent({ type: "ui_directive", directive: dir });
    store.applyStreamEvent({ type: "ui_directive", directive: { ...dir } });

    expect(useGameStore.getState().activeTurn?.pendingAfterNarration).toHaveLength(1);

    store.applyStreamEvent({
      type: "narration_complete",
      turnId: "turn-1",
      generationId: 1,
    });

    expect(useGameStore.getState().activeTurn?.releasedAfterNarration).toHaveLength(1);
    expect(useGameStore.getState().newlyReleasedDirectiveIds).toEqual(["dir-dup-1"]);

    // Re-delivering the same directive after completion must not duplicate or re-trigger newlyReleasedDirectiveIds
    store.consumeNewlyReleasedDirectives();
    store.applyStreamEvent({ type: "ui_directive", directive: dir });

    expect(useGameStore.getState().activeTurn?.releasedAfterNarration).toHaveLength(1);
    expect(useGameStore.getState().newlyReleasedDirectiveIds).toEqual([]);
  });

  it("recovers active turn state and partitions directives accurately on stream_reset", () => {
    const store = useGameStore.getState();
    store.beginTurn("turn-2", "撬开铁盒");

    const interrupt: PendingInterruptDTO = {
      interruptId: "int-2",
      sessionId: "sess-1",
      turnId: "turn-2",
      status: "awaiting_recovery",
      actionSummary: "撬开生锈铁盒",
      skill: "locksmith",
      skillLabel: "锁匠",
      difficulty: "regular",
      targetNumber: 45,
      bonusPenaltyDice: 0,
      stakesHint: "用力过猛可能折断锁芯。",
    };

    const immediateDir = makeDirective({
      directiveId: "dir-imm",
      turnId: "turn-2",
      displayTiming: "immediate",
      directiveType: "atmosphere_shift",
      payload: { theme: "candlelight", sceneTitle: "阁楼" },
    });
    const alreadyDisplayedDir = makeDirective({
      directiveId: "dir-shown",
      turnId: "turn-2",
      displayTiming: "after_narration",
      displayed: true,
    });
    const stillPendingDir = makeDirective({
      directiveId: "dir-pending",
      turnId: "turn-2",
      displayTiming: "after_narration",
      displayed: false,
    });

    store.applyStreamEvent({
      type: "stream_reset",
      turnId: "turn-2",
      snapshot: {
        turnId: "turn-2",
        generationId: 3,
        status: "narrating",
        lastSeq: 42,
        narrationSnapshot: "烛火摇晃间，锁簧发出一声轻响。",
        pendingInterrupt: interrupt,
        directives: [immediateDir, alreadyDisplayedDir, stillPendingDir],
      },
    });

    const nextState = useGameStore.getState();
    expect(nextState.pendingInterrupt?.interruptId).toBe("int-2");
    expect(nextState.displayedDirectiveIds["dir-shown"]).toBe(true);
    expect(nextState.activeTurn?.generationId).toBe(3);
    expect(nextState.activeTurn?.narrationText).toBe("烛火摇晃间，锁簧发出一声轻响。");
    expect(nextState.activeTurn?.immediateDirectives.map((d) => d.directiveId)).toEqual([
      "dir-imm",
    ]);
    expect(nextState.activeTurn?.releasedAfterNarration.map((d) => d.directiveId)).toEqual([
      "dir-shown",
    ]);
    expect(nextState.activeTurn?.pendingAfterNarration.map((d) => d.directiveId)).toEqual([
      "dir-pending",
    ]);
  });
});
