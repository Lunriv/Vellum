import { useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertCircle,
  ArrowRight,
  Briefcase,
  Compass,
  Dices,
  Eye,
  FileUp,
  Globe2,
  Heart,
  Loader2,
  RefreshCw,
  Sparkles,
  Trash2,
  Wand2,
  Zap,
} from "lucide-react";
import { createIdempotencyKey, vellumApi } from "../../api/client";
import type {
  CreateSessionResponse,
  ExperienceMode,
  InvestigatorPresetDTO,
  ModuleSummaryDTO,
} from "../../api/types";
import { useGameStore } from "../../stores/gameStore";

interface TurnZeroSetupProps {
  onSessionCreated: (response: CreateSessionResponse) => void;
}

const MODE_CARDS: Array<{
  value: ExperienceMode;
  label: string;
  badge?: string;
  description: string;
}> = [
  {
    value: "guided",
    label: "引导",
    description: "全程保留行动建议与检定风险提示，适合一键轻松跑团。",
  },
  {
    value: "classic",
    label: "经典",
    badge: "推荐",
    description: "动态沙盘、实体骰盘与短促多声道对戏联动，自由输入与快捷出手并存。",
  },
  {
    value: "immersive",
    label: "沉浸",
    description: "隐藏快捷建议，纯靠沙盘交互与手写自由扮演破局。",
  },
];

const WORLD_SEEDS: Array<{ label: string; title: string; prompt: string }> = [
  {
    label: "暴风雪山庄 · 冒名顶替",
    title: "林海雪站",
    prompt:
      "1988年暴风雪封山的东北林场招待所，六名滞留住客里有人冒名顶替，锅炉房半夜传来规律敲击声，电话线刚被人为剪断。",
  },
  {
    label: "赛博雨夜 · 记忆黑市",
    title: "九龙暗诊",
    prompt:
      "2079年霓虹暴雨中的地下义体诊所，有人正在秘密倒卖被篡改的记忆芯片，三名关键病人同时失忆，安保系统即将全楼封锁。",
  },
  {
    label: "深海潜艇 · 声呐异响",
    title: "深渊七号",
    prompt:
      "下潜至三千米海沟的科考潜艇突然与母船失联，氧气倒计时开启，声呐回波显示舱外有东西正在模仿失踪船员的节奏敲门。",
  },
];

const CREST_PALETTE = ["#9d3338", "#2c546e", "#2a6249", "#8c5b22", "#683c7a"];

function investigatorTone(id: string): string {
  let hash = 0;
  for (let i = 0; i < id.length; i += 1) {
    hash = (hash * 31 + id.charCodeAt(i)) >>> 0;
  }
  return CREST_PALETTE[hash % CREST_PALETTE.length];
}

function sourceBadge(kind: string): { label: string; toneClass: string } {
  if (kind === "generated") {
    return {
      label: "AI 生成世界",
      toneClass: "border-[var(--vellum-accent)]/50 bg-[var(--vellum-accent-soft)] text-[var(--vellum-accent)]",
    };
  }
  if (kind === "imported") {
    return {
      label: "导入模组",
      toneClass: "border-[var(--vellum-spk-skill)]/50 bg-[var(--vellum-spk-skill-bg)] text-[var(--vellum-spk-skill)]",
    };
  }
  return {
    label: "内置演示样板",
    toneClass: "border-[var(--vellum-border-strong)] bg-[var(--vellum-elevated)] text-[var(--vellum-muted)]",
  };
}

export function TurnZeroSetup({ onSessionCreated }: TurnZeroSetupProps) {
  const queryClient = useQueryClient();
  const experienceMode = useGameStore((s) => s.experienceMode);
  const setExperienceMode = useGameStore((s) => s.setExperienceMode);

  const [selectedModuleId, setSelectedModuleId] = useState<string | null>(null);
  const [selectedInvId, setSelectedInvId] = useState<string | null>(null);
  const [studioTab, setStudioTab] = useState<"closed" | "generate" | "import">("closed");

  // World Generator form state
  const [genPrompt, setGenPrompt] = useState("");
  const [genTitle, setGenTitle] = useState("");
  const [genSceneCount, setGenSceneCount] = useState<number>(4);
  const [generating, setGenerating] = useState(false);
  const [studioError, setStudioError] = useState<string | null>(null);
  const [studioNotice, setStudioNotice] = useState<string | null>(null);

  // Module Importer form state
  const [importFormat, setImportFormat] = useState<"text" | "yaml" | "json">("text");
  const [importTitle, setImportTitle] = useState("");
  const [importContent, setImportContent] = useState("");
  const [importing, setImporting] = useState(false);

  // Start Session state
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const submittingLockRef = useRef(false);
  const idempotencyKeyRef = useRef<string>(createIdempotencyKey());

  const modulesQuery = useQuery({
    queryKey: ["modules"],
    queryFn: () => vellumApi.listModules(),
    retry: false,
  });

  const modules: ModuleSummaryDTO[] = modulesQuery.data?.modules ?? [];
  const activeModuleId = selectedModuleId ?? modules[0]?.moduleId ?? null;
  const activeModule = modules.find((m) => m.moduleId === activeModuleId) ?? null;

  const {
    data: invData,
    isLoading: isLoadingInv,
    isError: isErrorInv,
    error: invError,
    refetch: refetchInv,
    isFetching: isFetchingInv,
  } = useQuery({
    queryKey: ["investigators", activeModuleId],
    queryFn: () => vellumApi.listInvestigators(activeModuleId!),
    enabled: Boolean(activeModuleId),
    staleTime: 30_000,
  });

  const investigators: InvestigatorPresetDTO[] = invData?.investigators ?? [];
  const activeInvestigatorId =
    (selectedInvId && investigators.some((i) => i.investigatorId === selectedInvId)
      ? selectedInvId
      : investigators[0]?.investigatorId) ?? null;

  const handleSelectModule = (moduleId: string) => {
    setSelectedModuleId(moduleId);
    setSelectedInvId(null);
    setSubmitError(null);
    idempotencyKeyRef.current = createIdempotencyKey();
  };

  const handleGenerateWorld = async () => {
    const trimmed = genPrompt.trim();
    if (!trimmed || generating) return;
    setGenerating(true);
    setStudioError(null);
    setStudioNotice(null);
    try {
      const res = await vellumApi.generateModule({
        prompt: trimmed,
        title: genTitle.trim() || undefined,
        sceneCount: genSceneCount,
      });
      await queryClient.invalidateQueries({ queryKey: ["modules"] });
      queryClient.setQueryData(["investigators", res.module.moduleId], {
        moduleId: res.module.moduleId,
        moduleTitle: res.module.title,
        openingPrologue: res.module.openingPrologue,
        investigators: res.investigators,
      });
      handleSelectModule(res.module.moduleId);
      setStudioNotice(
        `已编译生成「${res.module.title}」（${res.module.sceneCount} 处场景、${res.module.npcCount} 位人物、${res.module.clueCount} 条线索），可直接选角开团！`,
      );
      setStudioTab("closed");
      setGenPrompt("");
      setGenTitle("");
    } catch (err) {
      setStudioError(
        err instanceof Error && err.message
          ? `世界生成未通过校验：${err.message}`
          : "世界生成失败，请稍后重试。",
      );
    } finally {
      setGenerating(false);
    }
  };

  const handleImportModule = async () => {
    const trimmed = importContent.trim();
    if (!trimmed || importing) return;
    setImporting(true);
    setStudioError(null);
    setStudioNotice(null);
    try {
      const res = await vellumApi.importModule({
        format: importFormat,
        content: trimmed,
        title: importTitle.trim() || undefined,
      });
      await queryClient.invalidateQueries({ queryKey: ["modules"] });
      queryClient.setQueryData(["investigators", res.module.moduleId], {
        moduleId: res.module.moduleId,
        moduleTitle: res.module.title,
        openingPrologue: res.module.openingPrologue,
        investigators: res.investigators,
      });
      handleSelectModule(res.module.moduleId);
      setStudioNotice(
        `已成功导入「${res.module.title}」（${res.module.sceneCount} 处场景、${res.module.clueCount} 条线索），已通过静态图校验。`,
      );
      setStudioTab("closed");
      setImportContent("");
      setImportTitle("");
    } catch (err) {
      setStudioError(
        err instanceof Error && err.message
          ? `模组导入校验失败：${err.message}`
          : "模组导入失败，请检查格式后重试。",
      );
    } finally {
      setImporting(false);
    }
  };

  const handleDeleteModule = async (moduleId: string) => {
    try {
      await vellumApi.deleteModule(moduleId);
      if (selectedModuleId === moduleId) {
        setSelectedModuleId(null);
      }
      await queryClient.invalidateQueries({ queryKey: ["modules"] });
    } catch (err) {
      setStudioError(err instanceof Error ? err.message : "删除失败");
    }
  };

  const handleStartAdventure = async () => {
    if (!activeModuleId || !activeInvestigatorId || submittingLockRef.current) return;
    submittingLockRef.current = true;
    setSubmitting(true);
    setSubmitError(null);

    try {
      const res = await vellumApi.createSession(
        {
          moduleId: activeModuleId,
          investigatorId: activeInvestigatorId,
          experienceMode,
        },
        idempotencyKeyRef.current,
      );
      onSessionCreated(res);
    } catch (err) {
      idempotencyKeyRef.current = createIdempotencyKey();
      setSubmitError(
        err instanceof Error && err.message
          ? `暂时无法开启跑团：${err.message}`
          : "暂时无法连接守秘人服务，请检查服务是否已启动后重试。",
      );
      submittingLockRef.current = false;
      setSubmitting(false);
    }
  };

  const currentWorldTitle = invData?.moduleTitle ?? activeModule?.title ?? "未选择世界";
  const currentPrologue =
    invData?.openingPrologue ??
    activeModule?.openingPrologue ??
    "在上方生成一个全新的世界，或导入你的跑团模组，引擎会自动构建可交互沙盘与规则局势。";

  return (
    <main className="mx-auto max-w-6xl px-4 py-7 sm:px-6 sm:py-9">
      {/* Engine Lobby Header + World Generation & Import Entry */}
      <header className="vellum-wood-panel relative overflow-hidden rounded-[var(--vellum-radius-lg)] p-6">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="max-w-2xl">
            <p className="inline-flex items-center gap-1.5 text-xs font-bold tracking-[0.16em] text-[var(--vellum-brass)]">
              <Dices className="h-3.5 w-3.5" />
              <span>VELLUM 通用桌面跑团引擎 · 规则内核与世界编译台</span>
            </p>
            <h1 className="vellum-prose mt-2 text-2xl font-bold text-[var(--vellum-ink)] sm:text-3xl">
              选择世界模组，或一句话造出新世界
            </h1>
            <p className="mt-2 text-xs leading-6 text-[var(--vellum-muted)] sm:text-sm">
              引擎本身不绑定任何单一剧本：所有房间拓扑、战争迷雾、检定裁定、线索解锁与明暗危机钟全部由通用数据驱动。你可以直接导入剧本，或输入一个脑洞当场编译开团。
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              onClick={() => {
                setStudioError(null);
                setStudioTab((prev) => (prev === "generate" ? "closed" : "generate"));
              }}
              aria-expanded={studioTab === "generate"}
              className="inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-md)] bg-[var(--vellum-accent)] px-4 py-2 text-xs font-bold text-[var(--vellum-accent-ink)] shadow-[var(--vellum-shadow-sm)] transition-colors hover:bg-[var(--vellum-accent-hover)]"
            >
              <Wand2 className="h-3.5 w-3.5" />
              <span>一句话生成世界</span>
            </button>
            <button
              type="button"
              onClick={() => {
                setStudioError(null);
                setStudioTab((prev) => (prev === "import" ? "closed" : "import"));
              }}
              aria-expanded={studioTab === "import"}
              className="inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-wood-border)] bg-[var(--vellum-surface)] px-3.5 py-2 text-xs font-semibold text-[var(--vellum-ink)] transition-colors hover:border-[var(--vellum-brass)]"
            >
              <FileUp className="h-3.5 w-3.5 text-[var(--vellum-brass)]" />
              <span>导入模组</span>
            </button>
          </div>
        </div>

        {/* Inline World Generator Studio */}
        {studioTab === "generate" ? (
          <section
            aria-label="世界生成器"
            className="mt-5 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-accent)]/50 bg-[var(--vellum-bg)]/90 p-4"
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="inline-flex items-center gap-1.5 text-xs font-bold text-[var(--vellum-brass)]">
                <Sparkles className="h-3.5 w-3.5" />
                <span>输入世界观、危机背景或案件设定（任意题材均可）</span>
              </div>
              <div className="flex flex-wrap items-center gap-1.5">
                {WORLD_SEEDS.map((seed) => (
                  <button
                    key={seed.title}
                    type="button"
                    onClick={() => {
                      setGenTitle(seed.title);
                      setGenPrompt(seed.prompt);
                    }}
                    className="rounded-full border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-2.5 py-0.5 text-[11px] text-[var(--vellum-ink)] hover:border-[var(--vellum-accent)] hover:text-[var(--vellum-accent)]"
                  >
                    {seed.label}
                  </button>
                ))}
              </div>
            </div>

            <div className="mt-3 grid gap-2.5 sm:grid-cols-[1fr_140px]">
              <input
                type="text"
                aria-label="世界名称（选填）"
                value={genTitle}
                onChange={(e) => setGenTitle(e.target.value)}
                placeholder="世界名称（选填，留空自动起名）"
                className="rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-3 py-1.5 text-xs text-[var(--vellum-ink)] placeholder:text-[var(--vellum-muted)] focus:border-[var(--vellum-accent)] focus:outline-none"
              />
              <select
                aria-label="场景规模"
                value={genSceneCount}
                onChange={(e) => setGenSceneCount(Number(e.target.value))}
                className="rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-2.5 py-1.5 text-xs text-[var(--vellum-ink)] focus:border-[var(--vellum-accent)] focus:outline-none"
              >
                <option value={4}>4 处连通场景</option>
                <option value={5}>5 处连通场景</option>
                <option value={6}>6 处连通场景</option>
              </select>
            </div>

            <textarea
              rows={3}
              aria-label="世界设定描述"
              value={genPrompt}
              onChange={(e) => setGenPrompt(e.target.value)}
              placeholder="例如：1988年暴风雪封山的东北林场招待所，住客里有人冒名顶替，锅炉房半夜传来规律敲击声……"
              className="mt-2.5 w-full resize-y rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-3 py-2 text-xs leading-6 text-[var(--vellum-ink)] placeholder:text-[var(--vellum-muted)] focus:border-[var(--vellum-accent)] focus:outline-none"
            />

            <div className="mt-3 flex flex-wrap items-center justify-between gap-2">
              <span className="text-[11px] text-[var(--vellum-muted)]">
                将自动生成房间拓扑图、预制角色、立场各异的 NPC、多路径线索与明暗危机钟，并通过静态图校验。
              </span>
              <button
                type="button"
                disabled={generating || !genPrompt.trim()}
                onClick={() => void handleGenerateWorld()}
                className="inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-sm)] bg-[var(--vellum-accent)] px-4 py-1.5 text-xs font-bold text-[var(--vellum-accent-ink)] hover:bg-[var(--vellum-accent-hover)] disabled:opacity-50"
              >
                {generating ? (
                  <>
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    <span>正在编译世界拓扑与规则…</span>
                  </>
                ) : (
                  <>
                    <Wand2 className="h-3.5 w-3.5" />
                    <span>编译生成可玩世界</span>
                  </>
                )}
              </button>
            </div>
          </section>
        ) : null}

        {/* Inline Module Importer Studio */}
        {studioTab === "import" ? (
          <section
            aria-label="导入自定义模组"
            className="mt-5 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-wood-border)] bg-[var(--vellum-bg)]/90 p-4"
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className="text-xs font-bold text-[var(--vellum-ink)]">
                粘贴任意跑团模组大纲，或标准 JSON / YAML 模组定义
              </span>
              <div className="inline-flex rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] p-0.5 text-xs">
                {(
                  [
                    { id: "text", label: "纯文本自动编译" },
                    { id: "yaml", label: "YAML 模组" },
                    { id: "json", label: "JSON 模组" },
                  ] as const
                ).map((fmt) => (
                  <button
                    key={fmt.id}
                    type="button"
                    onClick={() => setImportFormat(fmt.id)}
                    className={`rounded-[3px] px-2.5 py-1 text-[11px] font-medium ${
                      importFormat === fmt.id
                        ? "bg-[var(--vellum-accent)] text-[var(--vellum-accent-ink)]"
                        : "text-[var(--vellum-muted)] hover:text-[var(--vellum-ink)]"
                    }`}
                  >
                    {fmt.label}
                  </button>
                ))}
              </div>
            </div>

            <input
              type="text"
              aria-label="导入世界标题（选填）"
              value={importTitle}
              onChange={(e) => setImportTitle(e.target.value)}
              placeholder="自定义世界名称（选填）"
              className="mt-2.5 w-full rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-3 py-1.5 text-xs text-[var(--vellum-ink)] placeholder:text-[var(--vellum-muted)] focus:border-[var(--vellum-accent)] focus:outline-none"
            />
            <textarea
              rows={4}
              aria-label="模组导入内容"
              value={importContent}
              onChange={(e) => setImportContent(e.target.value)}
              placeholder={
                importFormat === "text"
                  ? "直接粘贴你手头的 CoC/跑团剧本简介、关键场景、NPC 与真相大纲，引擎会自动编译为可运行的动态沙盘世界……"
                  : `粘贴符合 Vellum ModuleIr 结构的 ${importFormat.toUpperCase()} 内容……`
              }
              className="vellum-mono mt-2 w-full resize-y rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] px-3 py-2 text-xs leading-5 text-[var(--vellum-ink)] placeholder:text-[var(--vellum-muted)] focus:border-[var(--vellum-accent)] focus:outline-none"
            />
            <div className="mt-2.5 flex justify-end">
              <button
                type="button"
                disabled={importing || !importContent.trim()}
                onClick={() => void handleImportModule()}
                className="inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-sm)] bg-[var(--vellum-accent)] px-4 py-1.5 text-xs font-bold text-[var(--vellum-accent-ink)] hover:bg-[var(--vellum-accent-hover)] disabled:opacity-50"
              >
                {importing ? (
                  <>
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    <span>正在校验并导入…</span>
                  </>
                ) : (
                  <>
                    <FileUp className="h-3.5 w-3.5" />
                    <span>校验并导入世界</span>
                  </>
                )}
              </button>
            </div>
          </section>
        ) : null}

        {studioError ? (
          <div
            role="alert"
            className="mt-4 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-danger)]/40 bg-[var(--vellum-danger-soft)] px-3.5 py-2.5 text-xs text-[var(--vellum-ink)]"
          >
            {studioError}
          </div>
        ) : null}

        {studioNotice ? (
          <div
            role="status"
            className="mt-4 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-success)]/40 bg-[var(--vellum-success-soft)] px-3.5 py-2.5 text-xs font-medium text-[var(--vellum-ink)]"
          >
            {studioNotice}
          </div>
        ) : null}
      </header>

      {/* Module Library Shelf */}
      {modules.length > 0 ? (
        <section className="mt-6" aria-label="可用世界模组库">
          <div className="flex items-baseline justify-between gap-2">
            <h2 className="inline-flex items-center gap-1.5 text-sm font-bold text-[var(--vellum-ink)]">
              <Globe2 className="h-4 w-4 text-[var(--vellum-brass)]" />
              <span>已装载世界模组</span>
            </h2>
            <span className="text-xs text-[var(--vellum-muted)]">共 {modules.length} 个可运行世界</span>
          </div>

          <div className="mt-3 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {modules.map((mod) => {
              const selected = mod.moduleId === activeModuleId;
              const badge = sourceBadge(mod.sourceKind);
              return (
                <div
                  key={mod.moduleId}
                  onClick={() => handleSelectModule(mod.moduleId)}
                  className={`flex cursor-pointer flex-col justify-between rounded-[var(--vellum-radius-md)] border p-4 transition-all ${
                    selected
                      ? "border-[var(--vellum-accent)] bg-[var(--vellum-surface)] ring-1 ring-[var(--vellum-accent)]/60 shadow-[var(--vellum-shadow-sm)]"
                      : "border-[var(--vellum-border)] bg-[var(--vellum-surface)]/75 hover:border-[var(--vellum-brass)]"
                  }`}
                >
                  <div>
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0">
                        <h3 className="vellum-prose truncate text-base font-bold text-[var(--vellum-ink)]">
                          {mod.title}
                        </h3>
                        <span
                          className={`mt-1 inline-block rounded border px-1.5 py-0.2 text-[10px] font-semibold ${badge.toneClass}`}
                        >
                          {badge.label}
                        </span>
                      </div>
                      {mod.sourceKind !== "builtin" ? (
                        <button
                          type="button"
                          aria-label={`删除世界 ${mod.title}`}
                          onClick={(e) => {
                            e.stopPropagation();
                            void handleDeleteModule(mod.moduleId);
                          }}
                          className="rounded p-1 text-[var(--vellum-muted)] hover:bg-[var(--vellum-danger-soft)] hover:text-[var(--vellum-danger)]"
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </button>
                      ) : null}
                    </div>
                    <p className="mt-2 line-clamp-2 text-xs leading-5 text-[var(--vellum-muted)]">
                      {mod.openingPrologue}
                    </p>
                  </div>
                  <div className="vellum-mono mt-3 flex items-center gap-3 border-t border-[var(--vellum-border)] pt-2 text-[11px] text-[var(--vellum-muted)]">
                    <span>{mod.sceneCount} 场景</span>
                    <span>{mod.npcCount} NPC</span>
                    <span>{mod.clueCount} 线索</span>
                  </div>
                </div>
              );
            })}
          </div>
        </section>
      ) : null}

      {/* Selected World Briefing */}
      {activeModuleId ? (
        <section className="mt-6 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)]/80 px-4 py-3 text-xs">
          <span className="font-bold text-[var(--vellum-brass)]">当前选中世界 · {currentWorldTitle}：</span>
          <span className="text-[var(--vellum-ink)]">{currentPrologue}</span>
        </section>
      ) : null}

      {isLoadingInv || modulesQuery.isLoading ? (
        <section
          aria-live="polite"
          className="mt-6 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-border)] bg-[var(--vellum-surface)] p-6 shadow-[var(--vellum-shadow-sm)]"
        >
          <p className="text-sm font-medium text-[var(--vellum-ink)]">正在装载世界与角色卡……</p>
        </section>
      ) : null}

      {isErrorInv || modulesQuery.isError ? (
        <section
          role="alert"
          className="mt-6 rounded-[var(--vellum-radius-md)] border border-[var(--vellum-danger)]/40 bg-[var(--vellum-danger-soft)] px-5 py-4 text-[var(--vellum-ink)]"
        >
          <div className="flex items-start gap-3">
            <AlertCircle className="mt-0.5 h-5 w-5 shrink-0 text-[var(--vellum-danger)]" />
            <div className="flex-1">
              <h2 className="text-sm font-semibold">无法读取引擎模组列表</h2>
              <p className="mt-1 text-xs leading-6 text-[var(--vellum-muted)]">
                {(invError ?? modulesQuery.error) instanceof Error
                  ? `连接未能完成（${((invError ?? modulesQuery.error) as Error).message}）。请确认本地服务已运行，随后点击重试。`
                  : "当前无法连接后端服务，请稍后重试。"}
              </p>
              <button
                type="button"
                disabled={isFetchingInv || modulesQuery.isFetching}
                onClick={() => {
                  void modulesQuery.refetch();
                  void refetchInv();
                }}
                className="mt-3 inline-flex items-center gap-1.5 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border-strong)] bg-[var(--vellum-surface)] px-3 py-1.5 text-xs font-medium text-[var(--vellum-ink)] hover:bg-[var(--vellum-elevated)] disabled:opacity-50"
              >
                <RefreshCw className="h-3.5 w-3.5" />
                <span>重新加载</span>
              </button>
            </div>
          </div>
        </section>
      ) : null}

      {!isLoadingInv && !isErrorInv && investigators.length > 0 ? (
        <>
          <section className="mt-7" aria-labelledby="investigator-heading">
            <div className="flex items-baseline justify-between gap-2">
              <h2 id="investigator-heading" className="text-sm font-bold text-[var(--vellum-ink)]">
                选择调查员身份
              </h2>
              <span className="text-xs text-[var(--vellum-muted)]">
                共 {investigators.length} 张可扮演角色卡
              </span>
            </div>

            <div className="mt-3 grid gap-4 lg:grid-cols-3">
              {investigators.map((inv) => {
                const selected = inv.investigatorId === activeInvestigatorId;
                return (
                  <article
                    key={inv.investigatorId}
                    onClick={() => setSelectedInvId(inv.investigatorId)}
                    className={`flex cursor-pointer flex-col justify-between rounded-[var(--vellum-radius-lg)] border p-5 transition-all ${
                      selected
                        ? "border-[var(--vellum-accent)] bg-[var(--vellum-surface)] ring-1 ring-[var(--vellum-accent)]/60 shadow-[var(--vellum-shadow-md)]"
                        : "border-[var(--vellum-wood-border)]/70 bg-[var(--vellum-surface)]/85 hover:border-[var(--vellum-brass)]"
                    }`}
                  >
                    <div>
                      <div className="flex items-start justify-between gap-2">
                        <div className="flex items-center gap-3">
                          <div
                            className="vellum-prose flex h-11 w-11 shrink-0 items-center justify-center rounded-full border-2 border-[var(--vellum-brass)] text-base font-bold text-[#fff8eb] shadow-xs"
                            style={{ backgroundColor: investigatorTone(inv.investigatorId) }}
                            aria-hidden="true"
                          >
                            {inv.name.slice(0, 1)}
                          </div>
                          <div>
                            <h3 className="vellum-prose text-lg font-bold text-[var(--vellum-ink)]">
                              {inv.name}
                            </h3>
                            <p className="mt-0.5 inline-flex items-center gap-1.5 text-xs font-medium text-[var(--vellum-brass)]">
                              <Briefcase className="h-3.5 w-3.5" />
                              <span>{inv.occupation}</span>
                            </p>
                          </div>
                        </div>
                        <button
                          type="button"
                          aria-pressed={selected}
                          onClick={(e) => {
                            e.stopPropagation();
                            setSelectedInvId(inv.investigatorId);
                          }}
                          className={`rounded-[var(--vellum-radius-sm)] px-2.5 py-1 text-xs font-semibold ${
                            selected
                              ? "bg-[var(--vellum-accent)] text-[var(--vellum-accent-ink)]"
                              : "border border-[var(--vellum-border)] bg-[var(--vellum-elevated)] text-[var(--vellum-muted)] hover:text-[var(--vellum-ink)]"
                          }`}
                        >
                          {selected ? "已选定" : "选择"}
                        </button>
                      </div>

                      {inv.tagline ? (
                        <p className="mt-3 border-l-2 border-[var(--vellum-accent)]/70 pl-3 text-xs font-medium leading-6 text-[var(--vellum-ink)]">
                          {inv.tagline}
                        </p>
                      ) : null}

                      {inv.biography ? (
                        <p className="mt-2.5 text-xs leading-6 text-[var(--vellum-muted)]">
                          {inv.biography}
                        </p>
                      ) : null}

                      <dl className="mt-3 space-y-2 border-t border-[var(--vellum-border)] pt-3 text-xs">
                        {inv.motivation ? (
                          <div>
                            <dt className="font-semibold text-[var(--vellum-ink)]">此行动机</dt>
                            <dd className="mt-0.5 leading-5 text-[var(--vellum-muted)]">
                              {inv.motivation}
                            </dd>
                          </div>
                        ) : null}
                        {inv.manorConnection ? (
                          <div>
                            <dt className="font-semibold text-[var(--vellum-ink)]">现场渊源</dt>
                            <dd className="mt-0.5 leading-5 text-[var(--vellum-muted)]">
                              {inv.manorConnection}
                            </dd>
                          </div>
                        ) : null}
                      </dl>

                      <div className="mt-3 grid grid-cols-4 gap-1.5 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-elevated)]/70 p-2 text-center">
                        <div>
                          <div className="inline-flex items-center justify-center gap-1 text-[11px] text-[var(--vellum-muted)]">
                            <Heart className="h-3 w-3 text-[var(--vellum-danger)]" />
                            <span>生命</span>
                          </div>
                          <div className="vellum-mono text-xs font-semibold text-[var(--vellum-ink)]">
                            {inv.hpMax}
                          </div>
                        </div>
                        <div>
                          <div className="inline-flex items-center justify-center gap-1 text-[11px] text-[var(--vellum-muted)]">
                            <Zap className="h-3 w-3 text-[var(--vellum-accent)]" />
                            <span>魔力</span>
                          </div>
                          <div className="vellum-mono text-xs font-semibold text-[var(--vellum-ink)]">
                            {inv.mpMax}
                          </div>
                        </div>
                        <div>
                          <div className="inline-flex items-center justify-center gap-1 text-[11px] text-[var(--vellum-muted)]">
                            <Eye className="h-3 w-3 text-[var(--vellum-warning)]" />
                            <span>理智</span>
                          </div>
                          <div className="vellum-mono text-xs font-semibold text-[var(--vellum-ink)]">
                            {inv.sanStart}
                          </div>
                        </div>
                        <div>
                          <div className="inline-flex items-center justify-center gap-1 text-[11px] text-[var(--vellum-muted)]">
                            <Sparkles className="h-3 w-3 text-[var(--vellum-success)]" />
                            <span>幸运</span>
                          </div>
                          <div className="vellum-mono text-xs font-semibold text-[var(--vellum-ink)]">
                            {inv.luckStart}
                          </div>
                        </div>
                      </div>

                      {inv.strengths.length > 0 ? (
                        <div className="mt-3">
                          <div className="text-xs font-semibold text-[var(--vellum-ink)]">核心专长</div>
                          <div className="mt-1.5 flex flex-wrap gap-1.5">
                            {inv.strengths.map((s) => (
                              <span
                                key={s.skill}
                                className="inline-flex items-center gap-1 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-border)] bg-[var(--vellum-elevated)] px-2 py-0.5 text-[11px] text-[var(--vellum-ink)]"
                              >
                                <span>{s.label}</span>
                                <span className="vellum-mono font-semibold text-[var(--vellum-brass)]">
                                  {s.rating}%
                                </span>
                              </span>
                            ))}
                          </div>
                        </div>
                      ) : null}
                    </div>

                    {inv.startingItems.length > 0 ? (
                      <div className="mt-4 border-t border-[var(--vellum-border)] pt-3">
                        <div className="inline-flex items-center gap-1 text-xs font-semibold text-[var(--vellum-ink)]">
                          <Compass className="h-3.5 w-3.5 text-[var(--vellum-brass)]" />
                          <span>随身物品</span>
                        </div>
                        <ul className="mt-1.5 space-y-1 text-xs text-[var(--vellum-muted)]">
                          {inv.startingItems.map((item) => (
                            <li key={item.itemId}>
                              <span className="font-medium text-[var(--vellum-ink)]">{item.name}</span>
                              {item.description ? `：${item.description}` : ""}
                            </li>
                          ))}
                        </ul>
                      </div>
                    ) : null}
                  </article>
                );
              })}
            </div>
          </section>

          <section className="mt-7" aria-labelledby="experience-mode-heading">
            <h2 id="experience-mode-heading" className="text-sm font-bold text-[var(--vellum-ink)]">
              选择跑团交互模式
            </h2>
            <p className="mt-1 text-xs text-[var(--vellum-muted)]">
              仅调整快捷回应与提示显示，不改变世界规则与检定裁定，入桌后可随时在顶部切换。
            </p>

            <div className="mt-3 grid gap-3 sm:grid-cols-3" role="radiogroup" aria-label="体验模式选择">
              {MODE_CARDS.map((mode) => {
                const active = experienceMode === mode.value;
                return (
                  <button
                    key={mode.value}
                    type="button"
                    role="radio"
                    aria-checked={active}
                    onClick={() => setExperienceMode(mode.value)}
                    className={`rounded-[var(--vellum-radius-md)] border p-4 text-left transition-colors ${
                      active
                        ? "border-[var(--vellum-accent)] bg-[var(--vellum-accent-soft)]"
                        : "border-[var(--vellum-border)] bg-[var(--vellum-surface)] hover:border-[var(--vellum-brass)]"
                    }`}
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-sm font-semibold text-[var(--vellum-ink)]">{mode.label}</span>
                      {mode.badge ? (
                        <span className="rounded border border-[var(--vellum-brass)]/60 bg-[var(--vellum-surface)] px-1.5 py-0.5 text-[10px] font-semibold text-[var(--vellum-brass)]">
                          {mode.badge}
                        </span>
                      ) : null}
                    </div>
                    <p className="mt-1.5 text-xs leading-5 text-[var(--vellum-muted)]">
                      {mode.description}
                    </p>
                  </button>
                );
              })}
            </div>
          </section>

          {submitError ? (
            <div
              role="alert"
              className="mt-6 rounded-[var(--vellum-radius-sm)] border border-[var(--vellum-danger)]/40 bg-[var(--vellum-danger-soft)] px-4 py-3 text-xs text-[var(--vellum-ink)]"
            >
              {submitError}
            </div>
          ) : null}

          <div className="mt-7 flex flex-wrap items-center justify-between gap-4 border-t border-[var(--vellum-wood-border)]/70 pt-6">
            <p className="text-xs text-[var(--vellum-muted)]">
              即将装载「{currentWorldTitle}」的动态沙盘与规则时钟，随时允许自由扮演破局。
            </p>
            <button
              type="button"
              disabled={!activeModuleId || !activeInvestigatorId || submitting}
              onClick={() => void handleStartAdventure()}
              className="inline-flex items-center gap-2 rounded-[var(--vellum-radius-md)] bg-[var(--vellum-accent)] px-6 py-2.5 text-sm font-bold text-[var(--vellum-accent-ink)] shadow-[var(--vellum-shadow-md)] transition-colors hover:bg-[var(--vellum-accent-hover)] disabled:opacity-55"
            >
              <span>{submitting ? "正在装载世界沙盘…" : `落座开团 · 进入《${currentWorldTitle}》`}</span>
              <ArrowRight className="h-4 w-4" />
            </button>
          </div>
        </>
      ) : null}
    </main>
  );
}
