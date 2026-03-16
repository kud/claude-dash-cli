import React from "react"
import { Box, Text } from "ink"
import type { UsageData } from "../hooks/use-usage.js"
import { fmtCost, fmtTokens } from "../utils.js"

type UsageStripProps = {
  usage: UsageData
}

export const UsageStrip = ({ usage }: UsageStripProps) => {
  const { today, monthly, limits, loading } = usage

  return (
    <Box paddingX={1} paddingBottom={0}>
      {loading && !today && <Text dimColor>loading…</Text>}
      {today && (
        <>
          <Text dimColor>today </Text>
          <Text color="green" bold>
            {fmtCost(today.cost)}
          </Text>
          <Text dimColor> · {fmtTokens(today.totalTokens)}</Text>
        </>
      )}
      {monthly && (
        <>
          <Text dimColor>{"   month "}</Text>
          <Text color="cyan">{fmtCost(monthly.totalCost)}</Text>
          <Text dimColor> · {fmtTokens(monthly.totalTokens)}</Text>
        </>
      )}
      {limits && (
        <>
          <Text dimColor>{"   5hr "}</Text>
          <Text
            color={
              limits.fiveHour.utilization >= 90
                ? "red"
                : limits.fiveHour.utilization >= 70
                  ? "yellow"
                  : "green"
            }
          >
            {Math.round(limits.fiveHour.utilization)}%
          </Text>
          <Text dimColor>{"  7day "}</Text>
          <Text
            color={
              limits.sevenDay.utilization >= 90
                ? "red"
                : limits.sevenDay.utilization >= 70
                  ? "yellow"
                  : "green"
            }
          >
            {Math.round(limits.sevenDay.utilization)}%
          </Text>
        </>
      )}
      {loading && today && <Text dimColor> ↻</Text>}
    </Box>
  )
}
