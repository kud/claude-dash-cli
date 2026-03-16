import React from "react"
import { Box, Text } from "ink"
import type { UsageData } from "../hooks/use-usage.js"
import { fmtCost, fmtTokens } from "../utils.js"

type HeaderProps = {
  sessionCount: number
  activeCount: number
  pendingCount: number
  connected: boolean
  usage: UsageData
}

export const Header = ({
  sessionCount,
  activeCount,
  pendingCount,
  connected,
  usage,
}: HeaderProps) => (
  <Box paddingX={1} justifyContent="space-between">
    <Box>
      <Text bold color="cyan">
        ◆ claude-dash
      </Text>

      {activeCount > 0 ? (
        <Text color="green"> {activeCount} active</Text>
      ) : (
        <Text dimColor> {sessionCount} sessions</Text>
      )}

      {pendingCount > 0 && (
        <Text color="yellow" bold>
          {"  "}⚠ {pendingCount} pending
        </Text>
      )}

      {!connected && <Text color="red"> ⊘ disconnected</Text>}

      {usage.today && (
        <>
          <Text dimColor> │ today </Text>
          <Text color="green">{fmtCost(usage.today.cost)}</Text>
          <Text dimColor> · {fmtTokens(usage.today.totalTokens)}</Text>
        </>
      )}
      {usage.monthly && (
        <>
          <Text dimColor> month </Text>
          <Text color="cyan">{fmtCost(usage.monthly.totalCost)}</Text>
        </>
      )}
      {usage.limits && (
        <>
          <Text dimColor> 5hr </Text>
          <Text
            color={
              usage.limits.fiveHour.utilization >= 90
                ? "red"
                : usage.limits.fiveHour.utilization >= 70
                  ? "yellow"
                  : "green"
            }
          >
            {Math.round(usage.limits.fiveHour.utilization)}%
          </Text>
        </>
      )}
    </Box>
  </Box>
)
