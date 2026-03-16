import React, { memo } from "react"
import { Box, Text } from "ink"
import type { SessionState, SessionStatus } from "../../types.js"
import {
  formatDuration,
  truncMid,
  abbreviateHome,
  toolSummary,
} from "../utils.js"
import { AgentRow } from "./agent-row.js"

type SessionRowProps = {
  session: SessionState
  selected: boolean
}

const STATUS_ICON: Record<SessionStatus, { char: string; color: string }> = {
  processing: { char: "●", color: "cyan" },
  running_tool: { char: "◆", color: "yellow" },
  waiting_for_input: { char: "○", color: "gray" },
  waiting_for_approval: { char: "⚠", color: "yellow" },
  compacting: { char: "⊘", color: "blue" },
  ended: { char: "✗", color: "red" },
}

const STATUS_LABEL: Record<SessionStatus, string> = {
  processing: "processing",
  running_tool: "running",
  waiting_for_input: "idle",
  waiting_for_approval: "approval",
  compacting: "compacting",
  ended: "ended",
}

export const SessionRow = memo(({ session, selected }: SessionRowProps) => {
  const { char, color } = STATUS_ICON[session.status]
  const elapsed = formatDuration(Date.now() - session.startedAt)
  const cwd = truncMid(abbreviateHome(session.cwd), 36)
  const activeAgents = session.agents.filter((a) => a.status === "running")

  const toolLabel =
    session.status === "running_tool" && session.currentTool
      ? toolSummary(session.currentTool, session.currentToolInput ?? {})
      : session.status === "waiting_for_input" && session.lastNotification
        ? truncMid(session.lastNotification, 46)
        : null

  return (
    <Box flexDirection="column">
      <Box paddingX={1}>
        <Text color="cyan">{selected ? "▶" : " "} </Text>
        <Text color={color as "cyan" | "yellow" | "gray" | "blue" | "red"}>
          {char}
        </Text>
        <Text> </Text>
        <Text bold={selected}>{session.sessionId.slice(0, 8)}</Text>
        <Text dimColor> </Text>
        <Text color={selected ? "white" : undefined}>{cwd}</Text>
        <Text dimColor> </Text>
        <Text
          color={color as "cyan" | "yellow" | "gray" | "blue" | "red"}
          dimColor
        >
          {STATUS_LABEL[session.status]}
        </Text>
        <Text dimColor> {elapsed}</Text>
        {activeAgents.length > 0 && (
          <Text color="magenta">
            {"  "}
            {activeAgents.length}
            {activeAgents.length > 1 ? " agents" : " agent"}
          </Text>
        )}
      </Box>

      {toolLabel && (
        <Box paddingLeft={5}>
          <Text color="yellow" dimColor>
            {toolLabel}
          </Text>
        </Box>
      )}

      {activeAgents.map((agent) => (
        <AgentRow key={agent.agentId} agent={agent} />
      ))}
    </Box>
  )
})
