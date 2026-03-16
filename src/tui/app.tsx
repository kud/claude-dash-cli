import React, { useState } from "react"
import fs from "node:fs"
import { Box, Text, useInput } from "ink"
import { useDaemon } from "./hooks/use-daemon.js"
import { useUsage } from "./hooks/use-usage.js"
import { useTerminalSize } from "./hooks/use-terminal-size.js"
import { Header } from "./components/header.js"
import { Footer } from "./components/footer.js"
import { SessionList } from "./components/session-list.js"
import { AgentDetail } from "./components/agent-detail.js"
import { PermissionOverlay } from "./components/permission-overlay.js"
import { UsagePanel } from "./components/usage-panel.js"
import {
  NewSessionOverlay,
  extractRecentCwds,
} from "./components/new-session-overlay.js"

const ACTIVE_STATUSES = new Set([
  "processing",
  "running_tool",
  "waiting_for_approval",
  "compacting",
])

export const App = () => {
  const { sessions, pendingPermissions, sendDecision, connected } = useDaemon()
  const usage = useUsage()
  const { rows } = useTerminalSize()

  const [selectedIndex, setSelectedIndex] = useState(0)
  const [showNewSession, setShowNewSession] = useState(false)

  const clampedIndex =
    sessions.length > 0 ? Math.min(selectedIndex, sessions.length - 1) : 0
  const selectedSession = sessions[clampedIndex] ?? null
  const pendingPermission = pendingPermissions[0] ?? null
  const activeCount = sessions.filter((s) =>
    ACTIVE_STATUSES.has(s.status),
  ).length

  const handleQuitAndKill = () => {
    try {
      const pid = parseInt(
        fs.readFileSync("/tmp/claude-dash.pid", "utf8").trim(),
        10,
      )
      process.kill(pid, "SIGTERM")
    } catch {}
    process.exit(0)
  }

  useInput((input, key) => {
    if (showNewSession) return

    if (key.upArrow || input === "k")
      return setSelectedIndex((i) => Math.max(0, i - 1))
    if (key.downArrow || input === "j")
      return setSelectedIndex((i) =>
        Math.min(Math.max(0, sessions.length - 1), i + 1),
      )
    if (input === "a" && pendingPermission)
      return sendDecision(pendingPermission.connectionId, "allow")
    if (input === "d" && pendingPermission)
      return sendDecision(pendingPermission.connectionId, "deny")
    if (input === "n") return setShowNewSession(true)
    if (input === "r") return usage.refresh()
    if (input === "q") return process.exit(0)
    if (input === "Q" || (key.ctrl && input === "q")) return handleQuitAndKill()
  })

  // Left panel: agents top 60%, usage bottom 40%
  const contentRows = rows - 3
  const agentsRows = Math.floor(contentRows * 0.6)
  const usageRows = contentRows - agentsRows - 1

  return (
    <Box flexDirection="column" height={rows}>
      <Header
        sessionCount={sessions.length}
        activeCount={activeCount}
        pendingCount={pendingPermissions.length}
        connected={connected}
        usage={usage}
      />

      <Box flexDirection="row" flexGrow={1}>
        {/* Left column — agents + usage */}
        <Box
          flexDirection="column"
          width="35%"
          borderStyle="single"
          borderColor="gray"
        >
          {/* Agents panel */}
          <Box paddingX={1}>
            <Text bold color="cyan">
              Agents
            </Text>
            {activeCount > 0 && (
              <Text color="green"> · {activeCount} active</Text>
            )}
            <Text dimColor> · {sessions.length} total</Text>
          </Box>

          <Box flexDirection="column" height={agentsRows} overflow="hidden">
            {sessions.length === 0 ? (
              <Box paddingX={1} paddingY={1}>
                <Text dimColor>
                  No active sessions — press [n] to start one.
                </Text>
              </Box>
            ) : (
              <SessionList sessions={sessions} selectedIndex={clampedIndex} />
            )}
          </Box>

          <Box paddingX={1}>
            <Text dimColor>{"─".repeat(38)}</Text>
          </Box>

          {/* Usage panel */}
          <Box flexDirection="column" height={usageRows} overflow="hidden">
            <Box paddingX={1}>
              <Text bold color="cyan">
                Usage
              </Text>
              {usage.loading && <Text dimColor> ↻</Text>}
            </Box>
            <UsagePanel usage={usage} />
          </Box>
        </Box>

        {/* Right column — agent detail + tool history + diffs */}
        <Box
          flexDirection="column"
          width="65%"
          borderStyle="single"
          borderColor="gray"
          overflow="hidden"
        >
          <AgentDetail
            key={selectedSession?.sessionId ?? "none"}
            session={selectedSession}
          />
        </Box>
      </Box>

      <Footer hasPendingPermissions={pendingPermissions.length > 0} />

      {pendingPermission && !showNewSession && (
        <PermissionOverlay
          permission={pendingPermission}
          onAllow={() => sendDecision(pendingPermission.connectionId, "allow")}
          onDeny={() => sendDecision(pendingPermission.connectionId, "deny")}
        />
      )}

      {showNewSession && (
        <Box paddingX={2} paddingY={1}>
          <NewSessionOverlay
            recentCwds={extractRecentCwds(sessions)}
            onClose={() => setShowNewSession(false)}
          />
        </Box>
      )}
    </Box>
  )
}
