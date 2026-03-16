import React, { memo, useMemo } from "react"
import { Box, Text, Static } from "ink"
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

const timelineKey = (item: TimelineItem): string =>
  item.kind === "message"
    ? item.message.id
    : (item.entry.toolUseId ?? String(item.entry.startedAt))

const isSettled = (item: TimelineItem): boolean =>
  item.kind === "message" || item.entry.endedAt !== undefined

const TimelineItemView = ({ item }: { item: TimelineItem }) => {
  if (item.kind === "message") return <ChatMessage message={item.message} />
  return (
    <Box flexDirection="column">
      <ToolHistoryRow entry={item.entry} />
      {hasDiff(item.entry.toolName) && <ToolDiff entry={item.entry} />}
    </Box>
  )
}

type AgentDetailProps = {
  session: SessionState | null
}

export const AgentDetail = memo(({ session }: AgentDetailProps) => {
  const messages = useTranscript(session?.transcriptPath ?? null)

  const timeline = useMemo(
    () => (session ? buildTimeline(messages, session.toolHistory) : []),
    [messages, session?.toolHistory],
  )

  const settledItems = useMemo(() => timeline.filter(isSettled), [timeline])

  const activeItem = timeline.findLast((item) => !isSettled(item)) ?? null

  if (!session) {
    return (
      <Box flexDirection="column" flexGrow={1} paddingX={1} paddingY={1}>
        <Text dimColor>No session selected</Text>
      </Box>
    )
  }

  const elapsed = formatDuration(Date.now() - session.startedAt)

  return (
    <Box flexDirection="column" flexGrow={1}>
      {/* Settled items — printed once to terminal scroll buffer, never re-rendered */}
      <Static items={settledItems}>
        {(item) => (
          <Box key={timelineKey(item)} flexDirection="column" paddingX={1}>
            <TimelineItemView item={item} />
          </Box>
        )}
      </Static>

      {/* Live section — only re-renders with active state */}
      <Box paddingX={1}>
        <Text color="cyan" bold>
          {session.sessionId.slice(0, 8)}
        </Text>
        <Text dimColor> {abbreviateHome(session.cwd)}</Text>
        <Text dimColor> {session.status}</Text>
        <Text dimColor> {elapsed}</Text>
      </Box>

      <Box flexDirection="column" flexGrow={1} paddingX={1} paddingTop={1}>
        {activeItem ? (
          <TimelineItemView item={activeItem} />
        ) : (
          <Text dimColor>
            {timeline.length === 0 ? "No activity yet" : "Idle"}
          </Text>
        )}
      </Box>
    </Box>
  )
})
