import React, { memo } from "react"
import { Box, Text } from "ink"
import type { SessionState } from "../../types.js"
import { ToolHistoryRow } from "./tool-history-row.js"

type ToolHistoryPanelProps = {
  session: SessionState | null
}

export const ToolHistoryPanel = memo(({ session }: ToolHistoryPanelProps) => {
  if (!session) {
    return (
      <Box flexDirection="column" paddingX={1}>
        <Text dimColor>No session selected</Text>
      </Box>
    )
  }

  const entries = [...session.toolHistory].reverse().slice(0, 20)

  return (
    <Box flexDirection="column" paddingX={1}>
      <Box marginBottom={1}>
        <Text bold>Tool History</Text>
        <Text dimColor> — {session.sessionId}</Text>
      </Box>
      {entries.length === 0 && <Text dimColor>No tool calls yet</Text>}
      {entries.map((entry, index) => (
        <ToolHistoryRow key={entry.toolUseId ?? index} entry={entry} />
      ))}
    </Box>
  )
})
