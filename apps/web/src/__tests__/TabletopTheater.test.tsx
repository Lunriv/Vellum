import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../App";
import { vellumApi } from "../api/client";
import * as sseModule from "../api/sse";
import type { ClientVisibleStateDTO } from "../api/types";
import { parseTheaterNarration } from "../components/narrative/TheaterTranscriptBlock";
import { useGameStore } from "../stores/gameStore";

function createTestQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false, refetchOnWindowFocus: false },
    },
  });
}

const MULTI_VOICE_NARRATION = [
  "【守秘人】暴风雪把林场招待所的木窗吹得吱呀作响，壁炉火苗猛地一矮。",
  "【值班主管老周】「电话线是被风刮断的，谁也不准进锅炉房。」",
  "【直觉·心理学】他说到锅炉房时，右手下意识按住了腰间少了一把钥匙的铜环。",
  "【抉择】你是直接逼问备用钥匙的下落，还是先溜进档案室？",
  "【建议行动】\n1. [心理学] 盯住老周钥匙环追问昨夜去向\n2. [侦查] 翻看值班台被涂改的登记簿\n3. 先前往档案室避开视线",
].join("\n\n");

function buildCustomWorldState(): ClientVisibleStateDTO {
  return {
    session: {
      sessionId: "sess-snow-1",
      moduleId: "snowbound_lodge",
      moduleVersion: "1.0.0",
      investigatorId: "inv_scout",
      experienceMode: "classic",
      status: "active",
      sessionVersion: 2,
      activeTurnId: null,
      createdAt: "2026-11-27T00:00:00Z",
      updatedAt: "2026-11-27T00:00:02Z",
    },
    moduleTitle: "林海雪站",
    investigator: {
      investigatorId: "inv_scout",
      name: "陈默",
      occupation: "独立调查员",
      hp: 11,
      hpMax: 11,
      mp: 12,
      mpMax: 12,
      san: 55,
      sanMax: 99,
      luck: 50,
      luckMax: 99,
      characteristics: { INT: 75, EDU: 70, DEX: 65 },
      skills: { spot_hidden: 70, psychology: 65, locksmith: 50 },
      inventory: [
        { itemId: "flashlight", name: "金属手电", description: "强光手电。", quantity: 1 },
      ],
      conditions: [],
    },
    currentScene: {
      sceneId: "hub_lobby",
      title: "接待大厅",
      atmosphere: "fog",
      description: "值班台后的登记簿还摊开在昨晚那一页。",
      tags: ["hub"],
      visitedSceneIds: ["hub_lobby"],
      exits: [
        { targetSceneId: "records_office", label: "前往档案室", accessible: true },
        { targetSceneId: "boiler_room", label: "前往锅炉房", accessible: false, blockedReason: "铁门反锁" },
      ],
      presentEntities: [
        {
          entityId: "npc_warden",
          name: "值班主管老周",
          role: "gatekeeper",
          kind: "npc",
          summary: "攥着一串钥匙，神情紧绷。",
        },
        {
          entityId: "obj_register",
          name: "值班登记簿",
          role: "object",
          kind: "object",
          summary: "纸页有异常涂改。",
        },
      ],
    },
    mapRooms: [
      {
        sceneId: "hub_lobby",
        title: "接待大厅",
        atmosphere: "fog",
        visited: true,
        isCurrent: true,
        accessibleFromCurrent: false,
        exitLabelFromCurrent: null,
        knownNpcs: [{ entityId: "npc_warden", name: "值班主管老周" }],
      },
      {
        sceneId: "records_office",
        title: "档案室",
        atmosphere: "candlelight",
        visited: false,
        isCurrent: false,
        accessibleFromCurrent: true,
        exitLabelFromCurrent: "前往档案室",
        knownNpcs: [],
      },
      {
        sceneId: "service_corridor",
        title: "后勤通道",
        atmosphere: "rain_manor",
        visited: false,
        isCurrent: false,
        accessibleFromCurrent: false,
        exitLabelFromCurrent: null,
        knownNpcs: [],
      },
      {
        sceneId: "boiler_room",
        title: "锅炉房",
        atmosphere: "san_strain",
        visited: false,
        isCurrent: false,
        accessibleFromCurrent: false,
        exitLabelFromCurrent: "前往锅炉房",
        knownNpcs: [],
      },
    ],
    mapEdges: [
      { fromSceneId: "hub_lobby", toSceneId: "records_office", bidirectional: true, locked: false },
      { fromSceneId: "hub_lobby", toSceneId: "service_corridor", bidirectional: true, locked: false },
      { fromSceneId: "hub_lobby", toSceneId: "boiler_room", bidirectional: true, locked: true },
    ],
    worldTags: ["generator_sabotaged"],
    visibleClocks: [
      {
        clockId: "alert_pressure",
        name: "警戒压力",
        current: 1,
        max: 6,
        stageLabel: "表面平静",
      },
    ],
    clues: [],
    handouts: [],
    pendingInterrupt: null,
    transcript: [
      {
        entryId: "tr-0",
        turnId: "turn-0",
        turnIndex: 0,
        sceneId: "hub_lobby",
        playerInput: null,
        narration: MULTI_VOICE_NARRATION,
        narrationGeneration: 1,
        directives: [],
        createdAt: "2026-11-27T00:00:02Z",
      },
    ],
  };
}

describe("Universal Tabletop Engine: Dynamic World Map, Multi-Voice Theater & World Generator", () => {
  beforeEach(() => {
    window.localStorage.clear();
    useGameStore.setState({
      sessionId: "sess-snow-1",
      theme: "dark",
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

  it("parses multi-voice tags and extracts clickable choices cleanly", () => {
    const parsed = parseTheaterNarration(MULTI_VOICE_NARRATION);
    expect(parsed.beats.map((b) => b.kind)).toEqual(["kp", "npc", "skill", "hook"]);
    expect(parsed.beats[1].speaker).toBe("值班主管老周");
    expect(parsed.beats[2].speaker).toBe("直觉 · 心理学");
    expect(parsed.choices).toEqual([
      "[心理学] 盯住老周钥匙环追问昨夜去向",
      "[侦查] 翻看值班台被涂改的登记簿",
      "先前往档案室避开视线",
    ]);
  });

  it("renders arbitrary custom world dynamically on Auto-Layout World Map with world tags and 1-click travel", async () => {
    const user = userEvent.setup();
    vi.spyOn(vellumApi, "getSessionState").mockResolvedValue(buildCustomWorldState());
    const createTurnSpy = vi.spyOn(vellumApi, "createTurn").mockResolvedValue({
      turnId: "turn-next",
      turnIndex: 1,
      status: "routing",
      streamUrl: "/api/sessions/sess-snow-1/turns/turn-next/events",
    });

    render(
      <QueryClientProvider client={createTestQueryClient()}>
        <App />
      </QueryClientProvider>,
    );

    expect(await screen.findByText("林海雪站 · 沙盘")).toBeInTheDocument();
    expect(screen.getByText("generator_sabotaged")).toBeInTheDocument();
    expect(screen.getByText("守秘人 · KP")).toBeInTheDocument();

    const mapBtn = screen.getByRole("button", { name: "地图移动：前往档案室" });
    await user.click(mapBtn);

    await waitFor(() => {
      expect(createTurnSpy).toHaveBeenCalledWith("sess-snow-1", { input: "前往档案室" });
    });
  });

  it("supports generating a brand-new playable world directly from the Engine Lobby", async () => {
    const user = userEvent.setup();
    useGameStore.setState({ sessionId: null });

    const listModulesSpy = vi
      .spyOn(vellumApi, "listModules")
      .mockResolvedValueOnce({
        modules: [
          {
            moduleId: "demo_world",
            version: "1.0.0",
            title: "内置演示",
            openingPrologue: "样板开场。",
            sourceKind: "builtin",
            sceneCount: 4,
            npcCount: 2,
            investigatorCount: 1,
            clueCount: 4,
          },
        ],
      })
      .mockResolvedValue({
        modules: [
          {
            moduleId: "gen_cyber_01",
            version: "1.0.0",
            title: "九龙暗诊",
            openingPrologue: "2079年霓虹暴雨中的地下义体诊所。",
            sourceKind: "generated",
            sceneCount: 4,
            npcCount: 2,
            investigatorCount: 1,
            clueCount: 5,
          },
        ],
      });

    vi.spyOn(vellumApi, "listInvestigators").mockResolvedValue({
      moduleId: "demo_world",
      moduleTitle: "内置演示",
      investigators: [
        {
          investigatorId: "inv_1",
          name: "测试员",
          occupation: "调查员",
          tagline: "测试",
          biography: "简介",
          motivation: "动机",
          manorConnection: "渊源",
          hpMax: 10,
          mpMax: 10,
          sanStart: 50,
          luckStart: 50,
          strengths: [],
          startingItems: [],
        },
      ],
    });

    const generateSpy = vi.spyOn(vellumApi, "generateModule").mockResolvedValue({
      module: {
        moduleId: "gen_cyber_01",
        version: "1.0.0",
        title: "九龙暗诊",
        openingPrologue: "2079年霓虹暴雨中的地下义体诊所。",
        sourceKind: "generated",
        sceneCount: 4,
        npcCount: 2,
        investigatorCount: 1,
        clueCount: 5,
      },
      investigators: [
        {
          investigatorId: "inv_hacker",
          name: "阿哲",
          occupation: "义体维修师",
          tagline: "芯片不会撒谎",
          biography: "熟悉黑市义体接口。",
          motivation: "找回失踪搭档的记忆备份。",
          manorConnection: "曾为诊所修过神经终端。",
          hpMax: 10,
          mpMax: 12,
          sanStart: 55,
          luckStart: 50,
          strengths: [{ skill: "spot_hidden", label: "侦查", rating: 70 }],
          startingItems: [{ itemId: "deck", name: "解码器", description: "读取芯片日志。" }],
        },
      ],
    });

    render(
      <QueryClientProvider client={createTestQueryClient()}>
        <App />
      </QueryClientProvider>,
    );

    await screen.findByText("选择调查员身份");
    await user.click(screen.getByRole("button", { name: "一句话生成世界" }));
    await user.type(screen.getByLabelText("世界设定描述"), "2079年赛博地下义体诊所记忆失窃案");
    await user.click(screen.getByRole("button", { name: "编译生成可玩世界" }));

    await waitFor(() => {
      expect(generateSpy).toHaveBeenCalledWith(
        expect.objectContaining({ prompt: "2079年赛博地下义体诊所记忆失窃案" }),
      );
    });
    expect(await screen.findByText("阿哲")).toBeInTheDocument();
    expect(listModulesSpy).toHaveBeenCalled();
  });
});
