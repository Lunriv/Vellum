# Vellum Design Manifesto

> Extracted and synchronized with [ARCHITECTURE.md](ARCHITECTURE.md) v0.3.1 (Section 1 & Section 9.8).

## Product Goal

Build an AI Keeper (AI KP) that makes a player want to keep playing for hours—and immediately start a second adventure once the first scenario ends. Vellum is neither a dark-mode chat window nor a visual novel pretending to accept free text.

## Ten Principles

1. **Experience First**: Player experience outranks architectural complexity.
2. **Player Sovereignty**: Except for explicit rule-enforced involuntary effects, AI never decides the investigator's thoughts, emotions, or irreversible choices.
3. **Freedom within Rules**: Rules arbitrate consequences; they never trap the player inside pre-scripted dialogue trees.
4. **Invisible Engineering**: State machines, permissions, event logs, and model routing remain behind the curtain.
5. **Deterministic Truth, Creative Narrative**: Code owns dice, numbers, items, permissions, and committed facts; models own prose, character voice, and plausible improvisation.
6. **Dramatic World**: World progression serves dramatic tension through Progress Clocks rather than pointless background simulation.
7. **Minimal Necessary Agents**: Nodes are not agents. Add model calls only when cognitive separation measurably improves experience.
8. **Playable First**: Ship one complete, genuinely playable short scenario (*Crimson Rain Manor*) before platform expansion.
9. **Evaluate the Experience**: Measure both engineering invariants and whether real players want to keep playing.
10. **Fast Feedback, Honest Latency**: Deliver immediate status hints and truthful state progression; never mask dead waiting with fake "thinking" theater.

## Priority Order

- **P0**: Player autonomy, narrative immersion, world & rule trustworthiness, interaction fluidity.
- **P1**: Expressive NPCs, long-term consistency, information fairness, dynamic audiovisual feedback (DVP).
- **P2**: Generic module ecosystem / compiler, timeline forks, multiplayer.

## Visual Direction Guardrails

- **References**: *Disco Elysium* (narrative atmosphere & character presence), *Pentiment* (manuscript & historical materiality), *Book of Hours* (tactile objects, knowledge & occult clarity).
- **Keywords**: Restrained, editorial typography, paper-and-ink materiality, crisp hierarchy, measured motion, high readability, interaction woven into prose.
- **Anti-patterns**: Generic dark SaaS dashboards; heavy parchment noise or horror distortion that hurts text legibility; arbitrary LLM-generated HTML/JS.
- **Accessibility**: Low-SAN effects must be toggleable; honor `prefers-reduced-motion`; never rely on color alone for critical game state; no rapid flashing.
