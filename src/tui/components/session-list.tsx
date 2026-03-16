import React, { memo } from "react"
import { Box } from "ink"
import type { SessionState } from "../../types.js"
import { SessionRow } from "./session-row.js"

type SessionListProps = {
  sessions: SessionState[]
  selectedIndex: number
}

export const SessionList = memo(
  ({ sessions, selectedIndex }: SessionListProps) => (
    <Box flexDirection="column">
      {sessions.map((session, index) => (
        <SessionRow
          key={session.sessionId}
          session={session}
          selected={index === selectedIndex}
        />
      ))}
    </Box>
  ),
)
