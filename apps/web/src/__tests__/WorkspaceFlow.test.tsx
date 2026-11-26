import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../App";
import { ApiError, vellumApi } from "../api/client";
import * as sseModule from "../api/sse";
import type { ClientVisibleStateDTO, PendingInterruptDTO } from "../api/types";
import { useGameStore } from "../stores/gameStore";

function createTestQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        refetchOnWindowFocus: false,
      },
    },
  });
}

function buildMockSessionState(
  overrides: Partial<ClientVisibleStateDTO> = {},
): ClientVisibleStateDTO {
  return {
    session: {
      sessionId: "sess-flow-1",
      moduleId: "crimson_rain_manor",
      moduleVersion: "0.5.0",
      investigatorId: "inv-reporter",
      experienceMode: "classic",
      status: "active",
      sessionVersion: 3,
      activeTurnId: null,
      createdAt: "2026-11-26T12:00:00Z",
      updatedAt: "2026-11-26T12:05:00Z",
    },
    investigator: {
      investigatorId: "inv-reporter",
      name: "林知夏",
      occupation: "调查记者",
      hp: 10,
      hpMax: 11,
      mp: 12,
      mpMax: 12,
      san: 28,
      sanMax: 65,
      luck: 55,
      luckMax: 99,
      characteristics: { STR: 45, CON: 55, DEX: 60, INT: 75, POW: 60, EDU: 70 },
      skills: {
        spot_hidden: 70,
        library_use: 65,
        psychology: 60,
        persuade: 60,
        listen: 50,
        stealth: 40,
        history: 35,
      },
      inventory: [
        {
          itemId: "item-notebook",
          name: "速记本与钢笔",
          description: "夹着半页剪报。",
          quantity: 1,
        },
      ],
      conditions: [],
    },
    currentScene: {
      sceneId: "foyer",
      title: "绯雨庄园 · 门厅",
      atmosphere: "rain_manor",
      description: "潮湿的羊毛地毯一直延伸向二楼旋梯。",
      tags: ["indoor"],
      exits: [
        { targetSceneId: "study", label: "二楼书房", accessible: true },
        {
          targetSceneId: "cellar",
          label: "地下酒窖",
          accessible: false,
          blockedReason: "铁门上锁",
        },
      ],
      presentEntities: [
        {
          entityId: "npc-butler",
          name: "老陈",
          role: "庄园管家",
          kind: "npc",
          summary: "手里提着一盏煤油灯，神情拘谨。",
        },
      ],
    },
    visibleClocks: [
      {
        clockId: "clk-1",
        name: "暴雨封山",
        current: 2,
        max: 4,
        stageLabel: "山溪漫过石阶",
      },
    ],
    clues: [
      {
        clueId: "clue-1",
        title: "伞架旁的新鲜红泥",
        summary: "有人刚从后山温室方向回来。",
        sourceLabel: "庄园门厅",
        discoveredAtTurnId: "turn-0",
      },
    ],
    handouts: [],
    pendingInterrupt: null,
    transcript: [
      {
        entryId: "entry-0",
        turnId: "turn-0",
        turnIndex: 0,
        sceneId: "foyer",
        playerInput: null,
        narration: "铁门在你身后合拢，雨水顺着檐角砸在青石阶上。",
        narrationGeneration: 1,
        directives: [],
        createdAt: "2026-11-26T12:00:02Z",
      },
    ],
    ...overrides,
  };
}

describe("WorkspaceFlow end-to-end UI interactions", () => {
  beforeEach(() => {
    window.localStorage.clear();
    document.documentElement.dataset.theme = "light";
    delete document.documentElement.dataset.lowSanFx;

    useGameStore.setState({
      sessionId: "sess-flow-1",
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

    vi.spyOn(sseModule, "subscribeTurnEvents").mockImplementation(() => ({
      close: vi.fn(),
      getLastEventId: () => null,
    }));
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("toggles light/dark theme on documentElement and switches low-SAN visual effects on/off", async () => {
    const user = userEvent.setup();
    vi.spyOn(vellumApi, "getSessionState").mockResolvedValue(buildMockSessionState());

    render(
      <QueryClientProvider client={createTestQueryClient()}>
        <App />
      </QueryClientProvider>,
    );

    await screen.findByText("铁门在你身后合拢，雨水顺着檐角砸在青石阶上。");

    expect(document.documentElement.dataset.theme).toBe("light");
    const atmosphereLayer = screen.getByTestId("atmosphere-layer");
    expect(atmosphereLayer).toHaveAttribute("data-low-san-fx", "on");

    // Toggle to dark theme (雨夜侦探稿)
    await user.click(screen.getByRole("button", { name: "切换到雨夜夜间模式" }));
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(window.localStorage.getItem("vellum:theme")).toBe("dark");

    // Toggle back to light theme (纸墨档案室)
    await user.click(screen.getByRole("button", { name: "切换到纸墨白天模式" }));
    expect(document.documentElement.dataset.theme).toBe("light");

    // Toggle low-SAN visual effect off and back on
    await user.click(screen.getByTitle("点击关闭低理智视觉效果"));
    expect(atmosphereLayer).toHaveAttribute("data-low-san-fx", "off");
    expect(document.documentElement.dataset.lowSanFx).toBe("off");

    await user.click(screen.getByTitle("点击开启低理智视觉效果"));
    expect(atmosphereLayer).toHaveAttribute("data-low-san-fx", "on");
  });

  it("locks main action composer during PendingInterrupt while keeping OOC drawer openable and functional", async () => {
    const user = userEvent.setup();

    const activeInterrupt: PendingInterruptDTO = {
      interruptId: "int-lock-1",
      sessionId: "sess-flow-1",
      turnId: "turn-2",
      status: "awaiting_recovery",
      actionSummary: "追问老陈昨夜的去向",
      skill: "psychology",
      skillLabel: "心理学",
      difficulty: "regular",
      targetNumber: 60,
      bonusPenaltyDice: 0,
      stakesHint: "追问过急会令管家彻底闭口。",
      firstRoll: {
        checkId: "chk-psy",
        attemptIndex: 1,
        pushCount: 0,
        skill: "psychology",
        skillLabel: "心理学",
        difficulty: "regular",
        targetNumber: 60,
        bonusPenaltyDice: 0,
        rollValue: 74,
        resultLevel: "failure",
        passed: false,
      },
      recovery: {
        canAcceptFailure: true,
        canSpendLuck: true,
        luckCostNeeded: 14,
        canPushRoll: true,
      },
    };

    vi.spyOn(vellumApi, "getSessionState").mockResolvedValue(
      buildMockSessionState({ pendingInterrupt: activeInterrupt }),
    );

    const askOocSpy = vi.spyOn(vellumApi, "askOoc").mockResolvedValue({
      answer: "你当前剩余 55 点幸运与 28 点理智；消耗 14 点幸运可直接将本次心理学检定改判为成功。",
      history: [
        {
          id: "ooc-q1",
          role: "player",
          content: "我目前还剩多少幸运和理智？",
          createdAt: "2026-11-26T12:06:00Z",
        },
        {
          id: "ooc-a1",
          role: "assistant",
          content:
            "你当前剩余 55 点幸运与 28 点理智；消耗 14 点幸运可直接将本次心理学检定改判为成功。",
          createdAt: "2026-11-26T12:06:01Z",
        },
      ],
    });

    render(
      <QueryClientProvider client={createTestQueryClient()}>
        <App />
      </QueryClientProvider>,
    );

    // InlineDiceCard is embedded and main composer textarea is disabled
    await screen.findByText("心理学（常规）");
    const mainInput = screen.getByLabelText("输入你的调查行动或发言");
    expect(mainInput).toBeDisabled();

    // Open OOC assistant directly from the locked composer notice button
    await user.click(screen.getByRole("button", { name: "打开 OOC 规则助手" }));
    expect(screen.getByRole("dialog", { name: "OOC 场外规则助手" })).toBeInTheDocument();
    expect(screen.getByText("场外规则与状态问答 · 不推进游戏时间")).toBeInTheDocument();

    // Submit OOC question while main turn remains locked
    const oocInput = screen.getByLabelText("输入场外规则或状态问题");
    await user.type(oocInput, "我目前还剩多少幸运和理智？");
    await user.click(screen.getByRole("button", { name: "提问" }));

    await waitFor(() => {
      expect(askOocSpy).toHaveBeenCalledWith("sess-flow-1", {
        question: "我目前还剩多少幸运和理智？",
      });
    });

    expect(
      await screen.findByText(/消耗 14 点幸运可直接将本次心理学检定改判为成功/),
    ).toBeInTheDocument();
    // Main action input stays locked throughout OOC Q&A
    expect(mainInput).toBeDisabled();
  });

  it("handles 409 conflict banner with automatic state resync, and supports spoiler-gated Case Debrief when ended", async () => {
    const user = userEvent.setup();

    const getStateSpy = vi
      .spyOn(vellumApi, "getSessionState")
      .mockResolvedValueOnce(buildMockSessionState())
      .mockResolvedValueOnce(
        buildMockSessionState({
          session: {
            ...buildMockSessionState().session,
            status: "ended",
            sessionVersion: 4,
          },
        }),
      );

    vi.spyOn(vellumApi, "createTurn").mockRejectedValueOnce(
      new ApiError(409, "TURN_ALREADY_ACTIVE", "Another turn is already active."),
    );

    const getDebriefSpy = vi.spyOn(vellumApi, "getDebrief").mockResolvedValue({
      sessionId: "sess-flow-1",
      moduleTitle: "绯雨庄园",
      spoilerWarning: true,
      endingTitle: "雨歇黎明",
      endingSummary: "你在山洪冲毁木桥前带出了账册与关键证人。",
      trueChronology: [
        {
          title: "三日前夜",
          detail: "庄园主人在酒窖封存了旧契约。",
          sourceEventIds: ["evt-01"],
        },
      ],
      npcMotives: [
        {
          entityId: "npc-butler",
          name: "老陈",
          motive: "设法替旧主守住遗产真相。",
          secretStages: [{ tier: "核心秘密", unlocked: true, truth: "他藏起了酒窖备用钥匙。" }],
        },
      ],
      missedClues: [],
      finalClocks: [
        {
          clockId: "clk-hidden-1",
          name: "幕后销毁证据进度",
          visibility: "keeper_hidden",
          current: 2,
          max: 4,
          stageLabel: "书房账册尚未被烧毁",
        },
      ],
      playerImpact: [
        {
          choice: "优先前往二楼书房",
          consequence: "赶在证据被转移前保住了原件。",
          sourceEventIds: ["evt-08"],
        },
      ],
      alternativeEndings: [
        {
          title: "雨歇黎明",
          conditionSummary: "取得核心账册并安全撤离。",
          achieved: true,
        },
      ],
    });

    render(
      <QueryClientProvider client={createTestQueryClient()}>
        <App />
      </QueryClientProvider>,
    );

    const mainInput = await screen.findByLabelText("输入你的调查行动或发言");
    await user.type(mainInput, "前往二楼书房查看账册");
    await user.click(screen.getByRole("button", { name: "行动" }));

    // 409 triggers automatic getSessionState resync -> transitions to ended session state
    await waitFor(() => {
      expect(getStateSpy).toHaveBeenCalledTimes(2);
    });

    // Ended session unlocks Case Debrief button; debrief API is NOT called before spoiler gate confirmation
    const openDebriefBtn = await screen.findByRole("button", { name: "查看结案真相复盘" });
    await user.click(openDebriefBtn);

    expect(screen.getByText("包含完整剧透")).toBeInTheDocument();
    expect(getDebriefSpy).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "确认查看完整真相" }));
    expect(
      await screen.findByRole("heading", { level: 1, name: "雨歇黎明" }),
    ).toBeInTheDocument();
    expect(screen.getByText("守秘人隐藏钟")).toBeInTheDocument();
    expect(getDebriefSpy).toHaveBeenCalledTimes(1);
  });

  it("recovers interrupted_before_commit turn with explicit notice and resubmits as a fresh turn command", async () => {
    const user = userEvent.setup();

    vi.spyOn(vellumApi, "getSessionState").mockResolvedValue(
      buildMockSessionState({
        session: {
          ...buildMockSessionState().session,
          activeTurnId: "turn-halted-1",
        },
      }),
    );

    vi.spyOn(vellumApi, "getTurn").mockResolvedValue({
      turnId: "turn-halted-1",
      sessionId: "sess-flow-1",
      turnIndex: 1,
      status: "interrupted_before_commit",
      playerInput: "推开二楼书房的雕花木门",
      narrationGeneration: 1,
      narrationText: "",
      pendingInterrupt: null,
      directives: [],
    });

    const createTurnSpy = vi.spyOn(vellumApi, "createTurn").mockResolvedValue({
      turnId: "turn-fresh-2",
      turnIndex: 2,
      status: "narrating",
      streamUrl: "/api/sessions/sess-flow-1/turns/turn-fresh-2/events",
    });

    render(
      <QueryClientProvider client={createTestQueryClient()}>
        <App />
      </QueryClientProvider>,
    );

    expect(
      await screen.findByText("本次行动尚未结算，未产生任何检定或消耗"),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "重新提交本次行动" }));

    await waitFor(() => {
      expect(createTurnSpy).toHaveBeenCalledWith("sess-flow-1", {
        input: "推开二楼书房的雕花木门",
      });
    });
  });

  it("localizes present entity roles and avoids duplicating 前往 prefix when quick-filling exits", async () => {
    const user = userEvent.setup();
    vi.spyOn(vellumApi, "getSessionState").mockResolvedValue(
      buildMockSessionState({
        currentScene: {
          ...buildMockSessionState().currentScene,
          exits: [
            { targetSceneId: "maids_parlor", label: "前往女仆偏厅", accessible: true },
            { targetSceneId: "study", label: "二楼书房", accessible: true },
          ],
          presentEntities: [
            {
              entityId: "butler_arthur",
              name: "老管家 Arthur",
              role: "gatekeeper",
              kind: "npc",
              summary: "守在壁炉旁的老管家。",
            },
            {
              entityId: "manor_portrait",
              name: "家族肖像",
              role: "object",
              kind: "object",
              summary: "发暗的旧画像。",
            },
          ],
        },
      }),
    );

    render(
      <QueryClientProvider client={createTestQueryClient()}>
        <App />
      </QueryClientProvider>,
    );

    const exitWithPrefix = await screen.findByRole("button", { name: "前往女仆偏厅" });
    expect(screen.getByText("守门人")).toBeInTheDocument();
    expect(screen.getByText("可查物件")).toBeInTheDocument();
    expect(screen.queryByText("gatekeeper")).not.toBeInTheDocument();
    expect(screen.queryByText("object")).not.toBeInTheDocument();

    const mainInput = screen.getByLabelText("输入你的调查行动或发言");
    await user.click(exitWithPrefix);
    expect(mainInput).toHaveValue("前往女仆偏厅");

    await user.click(screen.getByRole("button", { name: "二楼书房" }));
    expect(mainInput).toHaveValue("前往二楼书房");
  });
});
