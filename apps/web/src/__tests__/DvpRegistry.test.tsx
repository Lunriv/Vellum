import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { DirectiveType, DynamicViewDirective } from "../api/types";
import { DvpDirectiveView } from "../components/dynamic_views/DvpRegistry";

function buildDirective(
  directiveType: DirectiveType,
  payload: Record<string, unknown>,
): DynamicViewDirective {
  return {
    schemaVersion: 1,
    directiveId: `dir-${directiveType}`,
    sessionId: "sess-1",
    turnId: "turn-1",
    narrationGeneration: 1,
    source: "kernel_owned",
    directiveType,
    displayTiming: "after_narration",
    payload,
  };
}

describe("DvpRegistry whitelisted component rendering", () => {
  it("renders all 7 whitelisted directive types accurately", () => {
    const { unmount: u1 } = render(
      <DvpDirectiveView
        experienceMode="classic"
        directive={buildDirective("inline_dice", {
          checkId: "chk-1",
          attemptIndex: 1,
          pushCount: 0,
          skill: "listen",
          skillLabel: "聆听",
          difficulty: "regular",
          targetNumber: 55,
          bonusPenaltyDice: 0,
          rollValue: 24,
          resultLevel: "hard_success",
          passed: true,
        })}
      />,
    );
    expect(screen.getByText("聆听（常规）")).toBeInTheDocument();
    expect(screen.getByText("24")).toBeInTheDocument();
    u1();

    const { unmount: u2 } = render(
      <DvpDirectiveView
        experienceMode="classic"
        directive={buildDirective("diegetic_handout", {
          handoutId: "h-letter",
          kind: "letter",
          title: "沈老爷的绝笔信",
          canonical: true,
          sourceFacts: ["落款日期为三日前雨夜"],
          presentationText: "若我未能走出地下酒窖，请勿相信医生的诊断书。",
        })}
      />,
    );
    expect(screen.getByText(/书信 · 沈老爷的绝笔信/)).toBeInTheDocument();
    expect(screen.getByText("已归档物证")).toBeInTheDocument();
    expect(screen.getByText(/落款日期为三日前雨夜/)).toBeInTheDocument();
    u2();

    const { unmount: u3 } = render(
      <DvpDirectiveView
        experienceMode="classic"
        directive={buildDirective("evidence_unlocked", {
          clueId: "clue-key",
          title: "温室铜钥匙上的泥垢",
          summary: "沾有后山特有的红黏土。",
          sourceLabel: "管家挂衣架",
        })}
      />,
    );
    expect(screen.getByText("已记录线索：温室铜钥匙上的泥垢")).toBeInTheDocument();
    expect(screen.getByText("来源：管家挂衣架")).toBeInTheDocument();
    u3();

    const { unmount: u4 } = render(
      <DvpDirectiveView
        experienceMode="classic"
        directive={buildDirective("clock_pulse", {
          clockId: "clk-storm",
          name: "山洪封路倒计时",
          previousValue: 2,
          currentValue: 3,
          maxValue: 4,
          stageLabel: "木桥水位逼近桥面",
          reason: "暴雨持续加剧",
        })}
      />,
    );
    expect(screen.getByText("山洪封路倒计时")).toBeInTheDocument();
    expect(screen.getByText("3 / 4")).toBeInTheDocument();
    expect(screen.getByText("木桥水位逼近桥面")).toBeInTheDocument();
    u4();

    const { unmount: u5 } = render(
      <DvpDirectiveView
        experienceMode="classic"
        directive={buildDirective("character_state_delta", {
          deltas: [{ label: "理智 -2" }, { label: "获得：黄铜钥匙" }],
          reason: "目睹酒窖墙上的抓痕",
        })}
      />,
    );
    expect(screen.getByText("状态变化：")).toBeInTheDocument();
    expect(screen.getByText("理智 -2")).toBeInTheDocument();
    expect(screen.getByText("获得：黄铜钥匙")).toBeInTheDocument();
    u5();

    const { unmount: u6 } = render(
      <DvpDirectiveView
        experienceMode="classic"
        directive={buildDirective("atmosphere_shift", {
          theme: "candlelight",
          sceneTitle: "二楼藏书室",
          note: "壁炉余烬渐暗",
        })}
      />,
    );
    expect(screen.getByText("二楼藏书室")).toBeInTheDocument();
    expect(screen.getByText(/壁炉余烬渐暗/)).toBeInTheDocument();
    u6();

    const { unmount: u7 } = render(
      <DvpDirectiveView
        experienceMode="classic"
        directive={{
          ...buildDirective("narrative_card", {
            title: "檐角雨声",
            body: "风从破碎的花窗灌入，吹得书页哗哗作响。",
          }),
          source: "narrative_owned",
        }}
      />,
    );
    expect(screen.getByText("檐角雨声")).toBeInTheDocument();
    expect(screen.getByText("风从破碎的花窗灌入，吹得书页哗哗作响。")).toBeInTheDocument();
    u7();
  });

  it("safely ignores unknown directive types and malformed payloads without throwing", () => {
    const unknownDirective = {
      ...buildDirective("narrative_card", { html: "<script>alert(1)</script>" }),
      directiveType: "untrusted_custom_html" as unknown as DirectiveType,
    };

    const { container: c1 } = render(
      <DvpDirectiveView experienceMode="classic" directive={unknownDirective} />,
    );
    expect(c1.firstChild).toBeNull();

    const { container: c2 } = render(
      <DvpDirectiveView
        experienceMode="classic"
        directive={buildDirective("diegetic_handout", { handoutId: "broken-only" })}
      />,
    );
    expect(c2.firstChild).toBeNull();
  });
});
