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
  "【守秘人】暴雨顺着门厅彩色玻璃淌下，壁炉火星噼啪作响。",
  "【老管家 Arthur】「外头山路早已断了，阁下还是先留在门厅暖暖身子吧。」",
  "【直觉·心理学】他提到酒窖时，右手拇指明显摩挲了袖口边缘。",
  "【抉择】灯影摇晃间，你打算直接拆穿他的戒备，还是先去书房？",
  "【建议行动】\n1. [心理学] 盯住 Arthur 的袖口追问三年前的访客\n2. [侦查] 检查壁炉旁的家族肖像\n3. 礼貌颔首，转身前往书房",
].join("\n\n");

function buildManorState(): ClientVisibleStateDTO {
  return {
    session: {
      sessionId: "sess-ab-1",
      moduleId: "crimson_rain_manor",
      moduleVersion: "1.0.0",
      investigatorId: "investigator_reporter",
      experienceMode: "classic",
      status: "active",
      sessionVersion: 2,
      activeTurnId: null,
      createdAt: "2026-11-27T00:00:00Z",
      updatedAt: "2026-11-27T00:00:02Z",
    },
    investigator: {
      investigatorId: "investigator_reporter",
      name: "林砚",
      occupation: "敏锐记者",
      hp: 10,
      hpMax: 10,
      mp: 12,
      mpMax: 12,
      san: 55,
      sanMax: 99,
      luck: 50,
      luckMax: 99,
      characteristics: { INT: 75, EDU: 70, DEX: 60 },
      skills: { spot_hidden: 70, psychology: 65, persuade: 60, locksmith: 25 },
      inventory: [
        { itemId: "fountain_pen", name: "钢笔", description: "报馆墨水钢笔。", quantity: 1 },
      ],
      conditions: [],
    },
    currentScene: {
      sceneId: "manor_foyer",
      title: "门厅与外廊",
      atmosphere: "rain_manor",
      description: "高窗外雨线如帘。",
      tags: ["entry"],
      visitedSceneIds: ["manor_foyer"],
      exits: [
        { targetSceneId: "study_room", label: "前往书房", accessible: true },
        { targetSceneId: "maids_parlor", label: "前往女仆偏厅", accessible: true },
        {
          targetSceneId: "cellar_entrance",
          label: "前往酒窖入口",
          accessible: false,
          blockedReason: "需先查看委托函",
        },
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
    mapRooms: [
      {
        sceneId: "manor_foyer",
        title: "门厅与外廊",
        atmosphere: "rain_manor",
        visited: true,
        isCurrent: true,
        accessibleFromCurrent: false,
        exitLabelFromCurrent: null,
        knownNpcs: [{ entityId: "butler_arthur", name: "老管家 Arthur" }],
      },
      {
        sceneId: "study_room",
        title: "书房",
        atmosphere: "candlelight",
        visited: false,
        isCurrent: false,
        accessibleFromCurrent: true,
        exitLabelFromCurrent: "前往书房",
        knownNpcs: [],
      },
      {
        sceneId: "maids_parlor",
        title: "女仆偏厅",
        atmosphere: "candlelight",
        visited: false,
        isCurrent: false,
        accessibleFromCurrent: true,
        exitLabelFromCurrent: "前往女仆偏厅",
        knownNpcs: [],
      },
      {
        sceneId: "rain_courtyard",
        title: "暴雨庭院",
        atmosphere: "san_strain",
        visited: false,
        isCurrent: false,
        accessibleFromCurrent: false,
        exitLabelFromCurrent: null,
        knownNpcs: [],
      },
      {
        sceneId: "cellar_entrance",
        title: "酒窖入口",
        atmosphere: "fog",
        visited: false,
        isCurrent: false,
        accessibleFromCurrent: false,
        exitLabelFromCurrent: "前往酒窖入口",
        knownNpcs: [],
      },
    ],
    visibleClocks: [
      {
        clockId: "manor_anomaly",
        name: "庄园异象",
        current: 1,
        max: 6,
        stageLabel: "雨声掩盖一切",
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
        sceneId: "manor_foyer",
        playerInput: null,
        narration: MULTI_VOICE_NARRATION,
        narrationGeneration: 1,
        directives: [],
        createdAt: "2026-11-27T00:00:02Z",
      },
    ],
  };
}

describe("A+B Fusion Tabletop Map & Multi-Voice Theater", () => {
  beforeEach(() => {
    window.localStorage.clear();
    useGameStore.setState({
      sessionId: "sess-ab-1",
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

  it("parses multi-voice tags and extracts 3 clickable choices cleanly", () => {
    const parsed = parseTheaterNarration(MULTI_VOICE_NARRATION);
    expect(parsed.beats.map((b) => b.kind)).toEqual(["kp", "npc", "skill", "hook"]);
    expect(parsed.beats[1].speaker).toBe("老管家 Arthur");
    expect(parsed.beats[2].speaker).toBe("直觉 · 心理学");
    expect(parsed.choices).toEqual([
      "[心理学] 盯住 Arthur 的袖口追问三年前的访客",
      "[侦查] 检查壁炉旁的家族肖像",
      "礼貌颔首，转身前往书房",
    ]);
  });

  it("renders multi-voice theater, clickable Manor map rooms, and 1-click Keeper choices", async () => {
    const user = userEvent.setup();
    vi.spyOn(vellumApi, "getSessionState").mockResolvedValue(buildManorState());
    const createTurnSpy = vi.spyOn(vellumApi, "createTurn").mockResolvedValue({
      turnId: "turn-next",
      turnIndex: 1,
      status: "routing",
      streamUrl: "/api/sessions/sess-ab-1/turns/turn-next/events",
    });

    render(
      <QueryClientProvider client={createTestQueryClient()}>
        <App />
      </QueryClientProvider>,
    );

    // Multi-voice theater beats rendered
    expect(await screen.findByText("守秘人 · KP")).toBeInTheDocument();
    expect(screen.getByText("直觉 · 心理学")).toBeInTheDocument();
    expect(screen.getByText("抉择时刻")).toBeInTheDocument();

    // Manor map accessible room is directly clickable
    const studyMapBtn = screen.getByRole("button", { name: "地图移动：前往书房" });
    await user.click(studyMapBtn);

    await waitFor(() => {
      expect(createTurnSpy).toHaveBeenCalledWith("sess-ab-1", { input: "前往书房" });
    });
  });

  it("executes a Keeper contextual choice immediately in one click", async () => {
    const user = userEvent.setup();
    vi.spyOn(vellumApi, "getSessionState").mockResolvedValue(buildManorState());
    const createTurnSpy = vi.spyOn(vellumApi, "createTurn").mockResolvedValue({
      turnId: "turn-choice",
      turnIndex: 1,
      status: "routing",
      streamUrl: "/api/sessions/sess-ab-1/turns/turn-choice/events",
    });

    render(
      <QueryClientProvider client={createTestQueryClient()}>
        <App />
      </QueryClientProvider>,
    );

    const choiceBtn = await screen.findByRole("button", {
      name: /盯住 Arthur 的袖口追问三年前的访客/,
    });
    await user.click(choiceBtn);

    await waitFor(() => {
      expect(createTurnSpy).toHaveBeenCalledWith("sess-ab-1", {
        input: "[心理学] 盯住 Arthur 的袖口追问三年前的访客",
      });
    });
  });
});
