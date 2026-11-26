import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { PendingInterruptDTO } from "../api/types";
import { InlineDiceCard } from "../components/dice/InlineDiceCard";

function buildBaseInterrupt(overrides: Partial<PendingInterruptDTO> = {}): PendingInterruptDTO {
  return {
    interruptId: "int-101",
    sessionId: "sess-1",
    turnId: "turn-3",
    status: "awaiting_roll",
    actionSummary: "仔细辨认壁炉灰烬里的残纸",
    skill: "spot_hidden",
    skillLabel: "侦查",
    difficulty: "hard",
    targetNumber: 60,
    bonusPenaltyDice: 0,
    stakesHint: "灰烬一碰即碎，莽撞翻找可能毁掉字迹。",
    ...overrides,
  };
}

describe("InlineDiceCard check & recovery flows", () => {
  it("handles awaiting_roll confirm, revise action, and cancel", async () => {
    const user = userEvent.setup();
    const onResolve = vi.fn();

    render(
      <InlineDiceCard
        interrupt={buildBaseInterrupt()}
        experienceMode="classic"
        onResolve={onResolve}
      />,
    );

    expect(screen.getByText("侦查（困难）")).toBeInTheDocument();
    expect(screen.getByText(/目标值 60（困难 ≤ 30）/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "确认掷骰" }));
    expect(onResolve).toHaveBeenCalledWith({ action: "confirm_roll" });

    await user.click(screen.getByRole("button", { name: "撤回" }));
    expect(onResolve).toHaveBeenCalledWith({ action: "cancel" });

    await user.click(screen.getByRole("button", { name: "修改行动" }));
    const input = screen.getByPlaceholderText(/输入修改后的行动/);
    await user.type(input, "先用镊子夹起边缘完好的纸角");
    await user.click(screen.getByRole("button", { name: "提交替代行动" }));

    expect(onResolve).toHaveBeenCalledWith({
      action: "revise_action",
      replacementAction: "先用镊子夹起边缘完好的纸角",
    });
  });

  it("renders awaiting_recovery options (accept failure, spend luck, push roll) and invokes callbacks", async () => {
    const user = userEvent.setup();
    const onResolve = vi.fn();

    const recoveryInterrupt = buildBaseInterrupt({
      status: "awaiting_recovery",
      firstRoll: {
        checkId: "chk-1",
        attemptIndex: 1,
        pushCount: 0,
        skill: "spot_hidden",
        skillLabel: "侦查",
        difficulty: "hard",
        targetNumber: 60,
        bonusPenaltyDice: 0,
        rollValue: 38,
        resultLevel: "regular_success",
        passed: false,
      },
      recovery: {
        canAcceptFailure: true,
        canSpendLuck: true,
        luckCostNeeded: 8,
        canPushRoll: true,
      },
    });

    render(
      <InlineDiceCard
        interrupt={recoveryInterrupt}
        experienceMode="guided"
        onResolve={onResolve}
      />,
    );

    expect(screen.getByText("38")).toBeInTheDocument();
    expect(screen.getByText(/首次结果已锁定，不可取消重掷/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "接受失败" }));
    expect(onResolve).toHaveBeenCalledWith({ action: "accept_failure" });

    await user.click(screen.getByRole("button", { name: /消耗 8 点幸运改判/ }));
    expect(onResolve).toHaveBeenCalledWith({ action: "spend_luck" });

    await user.click(screen.getByRole("button", { name: /孤注一掷/ }));
    expect(onResolve).toHaveBeenCalledWith({ action: "push_roll" });
  });

  it("completes awaiting_push_description and awaiting_push_consequence_ack interactions", async () => {
    const user = userEvent.setup();
    const onResolve = vi.fn();

    const { rerender } = render(
      <InlineDiceCard
        interrupt={buildBaseInterrupt({ status: "awaiting_push_description" })}
        experienceMode="classic"
        onResolve={onResolve}
      />,
    );

    const submitPushDescBtn = screen.getByRole("button", { name: "提交重试方式" });
    expect(submitPushDescBtn).toBeDisabled();

    await user.type(
      screen.getByPlaceholderText(/例如：趁他转身倒茶时/),
      "趴到炉膛边借煤油灯近距离辨认残墨",
    );
    expect(submitPushDescBtn).toBeEnabled();
    await user.click(submitPushDescBtn);

    expect(onResolve).toHaveBeenCalledWith({
      action: "submit_push_description",
      description: "趴到炉膛边借煤油灯近距离辨认残墨",
    });

    rerender(
      <InlineDiceCard
        interrupt={buildBaseInterrupt({
          status: "awaiting_push_consequence_ack",
          pushConsequence: "若再次失手，热灰翻落将彻底焚毁剩余纸片，并惊动走廊外的管家。",
        })}
        experienceMode="classic"
        onResolve={onResolve}
      />,
    );

    expect(
      screen.getByText("若再次失手，热灰翻落将彻底焚毁剩余纸片，并惊动走廊外的管家。"),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "确认承担风险并再次掷骰" }));
    expect(onResolve).toHaveBeenCalledWith({ action: "confirm_push_consequence" });

    await user.click(screen.getByRole("button", { name: "放弃孤注一掷，接受首次失败" }));
    expect(onResolve).toHaveBeenCalledWith({ action: "accept_initial_failure" });
  });
});
