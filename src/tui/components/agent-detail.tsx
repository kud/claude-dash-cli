import React, { memo } from "react"
import { Box, Text } from "ink"
import type { SessionState, ToolHistoryEntry } from "../../types.js"
import { abbreviateHome, formatDuration } from "../utils.js"
import {
  useTranscript,
  type TranscriptMessage,
} from "../hooks/use-transcript.js"
import { ChatMessage } from "./chat-message.js"
import { ToolHistoryRow } from "./tool-history-row.js"
import { ToolDiff } from "./tool-diff.js"

type TimelineItem =
  | { kind: "message"; message: TranscriptMessage }
  | { kind: "tool"; entry: ToolHistoryEntry }

const hasDiff = (toolName: string): boolean =>
  toolName === "Edit" || toolName === "MultiEdit" || toolName === "Write"

const buildTimeline = (
  messages: TranscriptMessage[],
  toolHistory: ToolHistoryEntry[],
): TimelineItem[] =>
  [
    ...messages.map((m): TimelineItem => ({ kind: "message", message: m })),
    ...toolHistory.map((e): TimelineItem => ({ kind: "tool", entry: e })),
  ].sort((a, b) => {
    const ta = a.kind === "message" ? a.message.timestamp : a.entry.startedAt
    const tb = b.kind === "message" ? b.message.timestamp : b.entry.startedAt
    return ta - tb
  })

type AgentDetailProps = {
  session: SessionState | null
}

export const AgentDetail = memo(({ session }: AgentDetailProps) => {
  const messages = useTranscript(session?.transcriptPath ?? null)

  if (!session) {
    return (
      <Box flexDirection="column" flexGrow={1} paddingX={1} paddingY={1}>
        <Text dimColor>No session selected</Text>
      </Box>
    )
  }

  const elapsed = formatDuration(Date.now() - session.startedAt)
  const timeline = buildTimeline(messages, session.toolHistory)

  return (
    <Box flexDirection="column" flexGrow={1}>
      <Box paddingX={1}>
        <Text color="cyan" bold>
          {session.sessionId.slice(0, 8)}
        </Text>
        <Text dimColor> {abbreviateHome(session.cwd)}</Text>
        <Text dimColor> {session.status}</Text>
        <Text dimColor> {elapsed}</Text>
      </Box>

      <Box
        flexDirection="column"
        overflow="hidden"
        flexGrow={1}
        paddingX={1}
        paddingTop={1}
      >
        {timeline.length === 0 && <Text dimColor>No activity yet</Text>}
        {timeline.map((item, i) => {
          if (item.kind === "message")
            return <ChatMessage key={item.message.id} message={item.message} />

          return (
            <Box key={item.entry.toolUseId ?? i} flexDirection="column">
              <ToolHistoryRow entry={item.entry} />
              {hasDiff(item.entry.toolName) && <ToolDiff entry={item.entry} />}
            </Box>
          )
        })}
      </Box>
    </Box>
  )
})
