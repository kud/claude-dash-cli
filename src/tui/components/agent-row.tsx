import React from "react"
import { Box, Text } from "ink"
import type { AgentNode } from "../../types.js"
import { formatDuration } from "../utils.js"

type AgentRowProps = {
  agent: AgentNode
}

export const AgentRow = ({ agent }: AgentRowProps) => {
  const elapsed = formatDuration(Date.now() - agent.startedAt)
  return (
    <Box>
      <Text dimColor>{"  └─ "}</Text>
      <Text dimColor>agent:{agent.agentId.slice(0, 8)}</Text>
      <Text>{"  "}</Text>
      <Text color={agent.status === "running" ? "green" : "gray"}>
        {agent.status}
      </Text>
      <Text>{"  "}</Text>
      <Text dimColor>{elapsed}</Text>
    </Box>
  )
}
