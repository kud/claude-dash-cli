import React, { memo } from "react"
import { Box, Text } from "ink"
import type { UsageData, ModelBreakdown } from "../hooks/use-usage.js"
import { progressBar, fmtCost, fmtTokens, fmtDelta } from "../utils.js"

const Divider = () => (
  <Box>
    <Text dimColor>{"─".repeat(60)}</Text>
  </Box>
)

const SectionTitle = ({ children }: { children: string }) => (
  <Box marginBottom={1}>
    <Text dimColor bold>
      {children}
    </Text>
  </Box>
)

const RateLimitRow = ({
  label,
  utilization,
  resetsAt,
}: {
  label: string
  utilization: number
  resetsAt: string
}) => {
  const bar = progressBar(utilization, 24)
  const color =
    utilization >= 90 ? "red" : utilization >= 70 ? "yellow" : "green"
  return (
    <Box marginBottom={0}>
      <Text dimColor>{label.padEnd(10)}</Text>
      <Text color={color}>{bar}</Text>
      <Text> </Text>
      <Text color={color} bold>
        {String(Math.round(utilization)).padStart(3)}%
      </Text>
      <Text dimColor> resets </Text>
      <Text>{resetsAt}</Text>
    </Box>
  )
}

const StatRow = ({
  label,
  cost,
  tokens,
  delta,
  extra,
}: {
  label: string
  cost: number
  tokens: number
  delta?: string
  extra?: string
}) => (
  <Box marginBottom={0}>
    <Text dimColor>{label.padEnd(14)}</Text>
    <Text bold color="white">
      {fmtCost(cost)}
    </Text>
    <Text dimColor> · </Text>
    <Text>{fmtTokens(tokens)}</Text>
    {delta && (
      <>
        <Text dimColor> </Text>
        <Text color={delta.startsWith("+") ? "red" : "green"} dimColor>
          vs yesterday: {delta}
        </Text>
      </>
    )}
    {extra && (
      <>
        <Text dimColor> </Text>
        <Text dimColor>{extra}</Text>
      </>
    )}
  </Box>
)

const ModelBar = ({
  model,
  maxTokens,
}: {
  model: ModelBreakdown
  maxTokens: number
}) => {
  const pct = maxTokens > 0 ? (model.totalTokens / maxTokens) * 100 : 0
  const bar = progressBar(pct, 18)
  const shortName = model.modelName
    .replace("claude-", "")
    .replace(/-\d{8}$/, "")
  return (
    <Box marginBottom={0}>
      <Text dimColor>{shortName.padEnd(22)}</Text>
      <Text color="cyan">{bar}</Text>
      <Text>{"  "}</Text>
      <Text dimColor>{String(Math.round(pct)).padStart(3)}%</Text>
      <Text>{"  "}</Text>
      <Text bold>{fmtCost(model.totalCost).padStart(8)}</Text>
      <Text dimColor>{"  "}</Text>
      <Text dimColor>{fmtTokens(model.totalTokens)}</Text>
    </Box>
  )
}

type UsagePanelProps = {
  usage: UsageData
}

export const UsagePanel = memo(({ usage }: UsagePanelProps) => {
  const { today, yesterday, monthly, total, limits, loading, limitsLoading } =
    usage

  const delta =
    today && yesterday ? fmtDelta(today.cost, yesterday.cost) : undefined

  const maxTokens = (
    total?.modelBreakdowns ??
    monthly?.modelBreakdowns ??
    []
  ).reduce((acc, m) => Math.max(acc, m.totalTokens), 0)

  const models = total?.modelBreakdowns ?? monthly?.modelBreakdowns ?? []

  const cacheCreated = total?.cacheCreationTokens ?? 0
  const cacheHits = total?.cacheReadTokens ?? 0
  const cacheSavedCost = cacheHits * 0.000003

  return (
    <Box flexDirection="column" paddingX={2} paddingY={1}>
      {/* Rate Limits */}
      <SectionTitle>Rate Limits · Remaining</SectionTitle>
      {limitsLoading && !limits && (
        <Box marginBottom={1}>
          <Text dimColor>fetching limits…</Text>
        </Box>
      )}
      {!limitsLoading && !limits && (
        <Box marginBottom={1}>
          <Text dimColor>
            limits unavailable — set ANTHROPIC_API_KEY to enable
          </Text>
        </Box>
      )}
      {limits && (
        <Box flexDirection="column" marginBottom={1}>
          <RateLimitRow
            label="5-Hour"
            utilization={limits.fiveHour.utilization}
            resetsAt={limits.fiveHour.resetsAt}
          />
          <RateLimitRow
            label="7-Day"
            utilization={limits.sevenDay.utilization}
            resetsAt={limits.sevenDay.resetsAt}
          />
          {limits.sevenDaySonnet && (
            <RateLimitRow
              label="Sonnet"
              utilization={limits.sevenDaySonnet.utilization}
              resetsAt={limits.sevenDaySonnet.resetsAt}
            />
          )}
        </Box>
      )}

      <Divider />

      {/* Usage Stats */}
      <Box flexDirection="column" marginTop={1} marginBottom={1}>
        {loading && !today && <Text dimColor>loading usage data…</Text>}
        {today && (
          <StatRow
            label="Today"
            cost={today.cost}
            tokens={today.totalTokens}
            delta={delta}
          />
        )}
        {monthly && (
          <StatRow
            label="This Month"
            cost={monthly.totalCost}
            tokens={monthly.totalTokens}
          />
        )}
        {total && (
          <StatRow
            label="All Time"
            cost={total.totalCost}
            tokens={total.totalTokens}
            extra={`${total.sessions} sessions`}
          />
        )}
      </Box>

      {models.length > 0 && (
        <>
          <Divider />
          <Box flexDirection="column" marginTop={1} marginBottom={1}>
            <SectionTitle>Model Breakdown</SectionTitle>
            {models
              .sort((a, b) => b.totalTokens - a.totalTokens)
              .map((m) => (
                <ModelBar key={m.modelName} model={m} maxTokens={maxTokens} />
              ))}
          </Box>
        </>
      )}

      {(cacheCreated > 0 || cacheHits > 0) && (
        <>
          <Divider />
          <Box flexDirection="column" marginTop={1}>
            <SectionTitle>Cache</SectionTitle>
            <Box>
              <Text dimColor>created </Text>
              <Text>{fmtTokens(cacheCreated)}</Text>
              <Text dimColor> · hits </Text>
              <Text color="green">{fmtTokens(cacheHits)}</Text>
              <Text dimColor> · saved </Text>
              <Text color="green">~{fmtCost(cacheSavedCost)}</Text>
            </Box>
          </Box>
        </>
      )}

      {usage.lastFetched && (
        <Box marginTop={1}>
          <Text dimColor>
            last updated {new Date(usage.lastFetched).toLocaleTimeString()}
          </Text>
        </Box>
      )}
    </Box>
  )
})
