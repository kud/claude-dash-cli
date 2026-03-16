import React, { memo } from "react"
import { Box, Text } from "ink"
import type { ToolHistoryEntry } from "../../types.js"
import { formatDuration, formatToolInput } from "../utils.js"

type ToolHistoryRowProps = {
  entry: ToolHistoryEntry
}

const formatTimestamp = (ms: number): string => {
  const d = new Date(ms)
  const hh = String(d.getHours()).padStart(2, "0")
  const mm = String(d.getMinutes()).padStart(2, "0")
  const ss = String(d.getSeconds()).padStart(2, "0")
  return `${hh}:${mm}:${ss}`
}

export const ToolHistoryRow = memo(({ entry }: ToolHistoryRowProps) => {
  const pending = entry.endedAt === undefined
  const statusIcon = pending ? "·" : entry.success !== false ? "✓" : "✗"
  const statusColor = pending
    ? "yellow"
    : entry.success !== false
      ? "green"
      : "red"
  const duration = entry.endedAt
    ? formatDuration(entry.endedAt - entry.startedAt)
    : undefined

  const { summary, diff, color } = formatToolInput(
    entry.toolName,
    entry.toolInput,
  )

  return (
    <Box>
      <Text dimColor>{formatTimestamp(entry.startedAt)} </Text>
      <Text color={statusColor}>{statusIcon} </Text>
      <Text color={color} bold>
        {entry.toolName.padEnd(10)}
      </Text>
      <Text dimColor> {summary}</Text>
      {diff && diff.removed > 0 && <Text color="red"> -{diff.removed}</Text>}
      {diff && diff.added > 0 && <Text color="green"> +{diff.added}</Text>}
      {duration && <Text dimColor> {duration}</Text>}
    </Box>
  )
})
