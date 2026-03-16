import { exec } from "node:child_process"
import { useEffect, useReducer, useRef } from "react"

export type ModelBreakdown = {
  modelName: string
  inputTokens: number
  outputTokens: number
  totalTokens: number
  totalCost: number
}

export type DailyUsage = {
  cost: number
  inputTokens: number
  outputTokens: number
  totalTokens: number
  cacheCreationTokens: number
  cacheReadTokens: number
  date: string
}

export type MonthlyUsage = {
  month: string
  totalCost: number
  totalTokens: number
  inputTokens: number
  outputTokens: number
  cacheCreationTokens: number
  cacheReadTokens: number
  modelBreakdowns: ModelBreakdown[]
}

export type TotalUsage = {
  totalCost: number
  totalTokens: number
  inputTokens: number
  outputTokens: number
  cacheCreationTokens: number
  cacheReadTokens: number
  sessions: number
  modelBreakdowns: ModelBreakdown[]
}

export type RateLimitEntry = {
  utilization: number
  resetsAt: string
  rawTimestamp?: string
}

export type RateLimits = {
  fiveHour: RateLimitEntry
  sevenDay: RateLimitEntry
  sevenDaySonnet?: RateLimitEntry
}

export type UsageData = {
  today: DailyUsage | null
  yesterday: DailyUsage | null
  monthly: MonthlyUsage | null
  total: TotalUsage | null
  limits: RateLimits | null
  loading: boolean
  limitsLoading: boolean
  error: string | null
  lastFetched: number | null
}

type UsageAction =
  | { type: "loading" }
  | {
      type: "success"
      today: DailyUsage | null
      yesterday: DailyUsage | null
      monthly: MonthlyUsage | null
      total: TotalUsage | null
    }
  | { type: "limits_success"; limits: RateLimits }
  | { type: "error"; error: string }

const initialState: UsageData = {
  today: null,
  yesterday: null,
  monthly: null,
  total: null,
  limits: null,
  loading: true,
  limitsLoading: true,
  error: null,
  lastFetched: null,
}

const reducer = (state: UsageData, action: UsageAction): UsageData => {
  switch (action.type) {
    case "loading":
      return { ...state, loading: true, error: null }
    case "success":
      return {
        ...state,
        loading: false,
        error: null,
        lastFetched: Date.now(),
        today: action.today,
        yesterday: action.yesterday,
        monthly: action.monthly,
        total: action.total,
      }
    case "limits_success":
      return { ...state, limitsLoading: false, limits: action.limits }
    case "error":
      return { ...state, loading: false, error: action.error }
  }
}

const run = (cmd: string, timeout = 30_000): Promise<string> =>
  new Promise((resolve, reject) => {
    exec(cmd, { timeout }, (err, stdout) => {
      if (err) return reject(err)
      resolve(stdout)
    })
  })

const runCcusage = async (args: string): Promise<unknown> => {
  const parse = (s: string) => {
    const trimmed = s.trim()
    if (!trimmed) throw new Error("empty output")
    return JSON.parse(trimmed)
  }
  try {
    return parse(await run(`ccusage ${args}`, 15_000))
  } catch {
    return parse(await run(`npx --yes ccusage@latest ${args}`, 30_000))
  }
}

const normalizeDaily = (raw: unknown): DailyUsage | null => {
  if (!raw || typeof raw !== "object") return null
  const r = Array.isArray(raw)
    ? (raw[raw.length - 1] as Record<string, unknown>)
    : (raw as Record<string, unknown>)
  if (typeof r["cost"] !== "number" && typeof r["totalCost"] !== "number")
    return null
  return {
    cost: (r["cost"] ?? r["totalCost"] ?? 0) as number,
    inputTokens: (r["inputTokens"] ?? 0) as number,
    outputTokens: (r["outputTokens"] ?? 0) as number,
    totalTokens: (r["totalTokens"] ?? 0) as number,
    cacheCreationTokens: (r["cacheCreationTokens"] ?? 0) as number,
    cacheReadTokens: (r["cacheReadTokens"] ?? 0) as number,
    date: (r["date"] ?? "") as string,
  }
}

const normalizeMonthly = (raw: unknown): MonthlyUsage | null => {
  if (!raw || typeof raw !== "object") return null
  const r = raw as Record<string, unknown>
  if (typeof r["totalCost"] !== "number") return null
  return {
    month: (r["month"] as string) ?? "",
    totalCost: r["totalCost"] as number,
    totalTokens: (r["totalTokens"] as number) ?? 0,
    inputTokens: (r["inputTokens"] as number) ?? 0,
    outputTokens: (r["outputTokens"] as number) ?? 0,
    cacheCreationTokens: (r["cacheCreationTokens"] as number) ?? 0,
    cacheReadTokens: (r["cacheReadTokens"] as number) ?? 0,
    modelBreakdowns: (r["modelBreakdowns"] as ModelBreakdown[]) ?? [],
  }
}

const normalizeTotal = (raw: unknown): TotalUsage | null => {
  if (!raw || typeof raw !== "object") return null
  const r = raw as Record<string, unknown>
  if (typeof r["totalCost"] !== "number") return null
  return {
    totalCost: r["totalCost"] as number,
    totalTokens: (r["totalTokens"] as number) ?? 0,
    inputTokens: (r["inputTokens"] as number) ?? 0,
    outputTokens: (r["outputTokens"] as number) ?? 0,
    cacheCreationTokens: (r["cacheCreationTokens"] as number) ?? 0,
    cacheReadTokens: (r["cacheReadTokens"] as number) ?? 0,
    sessions: (r["sessions"] as number) ?? 0,
    modelBreakdowns: (r["modelBreakdowns"] as ModelBreakdown[]) ?? [],
  }
}

const yesterdayIso = (): string => {
  const d = new Date()
  d.setDate(d.getDate() - 1)
  return d.toISOString().slice(0, 10)
}

const getClaudeToken = async (): Promise<string | null> => {
  const fromEnv =
    process.env["ANTHROPIC_API_KEY"] ?? process.env["CLAUDE_API_KEY"]
  if (fromEnv) return fromEnv
  try {
    const token = await run(
      `security find-generic-password -s "Claude" -w 2>/dev/null`,
      3_000,
    )
    const trimmed = token.trim()
    if (trimmed) return trimmed
  } catch {}
  return null
}

const formatResetsAt = (isoTs: string): string => {
  const diff = new Date(isoTs).getTime() - Date.now()
  if (diff <= 0) return "now"
  const totalMinutes = Math.floor(diff / 60_000)
  const days = Math.floor(totalMinutes / (60 * 24))
  const hours = Math.floor((totalMinutes % (60 * 24)) / 60)
  const mins = totalMinutes % 60
  if (days > 0) return `${days}d ${hours}h`
  if (hours > 0) return `${hours}h ${mins}m`
  return `${mins}m`
}

const fetchRateLimits = async (): Promise<RateLimits | null> => {
  const token = await getClaudeToken()
  if (!token) return null
  try {
    const body = await run(
      `curl -sf -X POST https://api.anthropic.com/api/oauth/usage -H "Authorization: Bearer ${token}" -H "Content-Type: application/json" -d "{}"`,
      10_000,
    )
    const data = JSON.parse(body) as Record<string, unknown>

    const normalizeEntry = (raw: unknown): RateLimitEntry | null => {
      if (!raw || typeof raw !== "object") return null
      const r = raw as Record<string, unknown>
      const resets = (r["resets_at"] ?? r["resetsAt"] ?? "") as string
      return {
        utilization: (r["utilization"] ?? 0) as number,
        resetsAt: resets ? formatResetsAt(resets) : "—",
        rawTimestamp: resets,
      }
    }

    const fiveHour =
      normalizeEntry(data["five_hour"]) ??
      normalizeEntry(data["fiveHour"]) ??
      normalizeEntry((data["limits"] as Record<string, unknown>)?.["five_hour"])
    const sevenDay =
      normalizeEntry(data["seven_day"]) ?? normalizeEntry(data["sevenDay"])
    const sevenDaySonnet =
      normalizeEntry(data["seven_day_sonnet"]) ??
      normalizeEntry(data["sevenDaySonnet"]) ??
      undefined

    if (!fiveHour || !sevenDay) return null
    return { fiveHour, sevenDay, sevenDaySonnet: sevenDaySonnet ?? undefined }
  } catch {
    return null
  }
}

const USAGE_REFRESH_MS = 60_000
const LIMITS_REFRESH_MS = 30_000

export const useUsage = (): UsageData & { refresh: () => void } => {
  const [state, dispatch] = useReducer(reducer, initialState)
  const fetchingRef = useRef(false)
  const usageTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const limitsTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const fetchUsage = async () => {
    if (fetchingRef.current) return
    fetchingRef.current = true
    dispatch({ type: "loading" })
    try {
      const yestDate = yesterdayIso()
      const [rawToday, rawYest, rawMonthly, rawTotal] = await Promise.all([
        runCcusage("daily --json").catch(() => null),
        runCcusage(`daily --json --date ${yestDate}`).catch(() => null),
        runCcusage("monthly --json").catch(() => null),
        runCcusage("--json").catch(() => null),
      ])
      dispatch({
        type: "success",
        today: normalizeDaily(rawToday),
        yesterday: normalizeDaily(rawYest),
        monthly: normalizeMonthly(rawMonthly),
        total: normalizeTotal(rawTotal),
      })
    } catch (err) {
      dispatch({ type: "error", error: String(err) })
    } finally {
      fetchingRef.current = false
    }
  }

  const fetchLimits = async () => {
    const limits = await fetchRateLimits()
    if (limits) dispatch({ type: "limits_success", limits })
  }

  useEffect(() => {
    fetchUsage()
    fetchLimits()
    usageTimerRef.current = setInterval(fetchUsage, USAGE_REFRESH_MS)
    limitsTimerRef.current = setInterval(fetchLimits, LIMITS_REFRESH_MS)
    return () => {
      if (usageTimerRef.current) clearInterval(usageTimerRef.current)
      if (limitsTimerRef.current) clearInterval(limitsTimerRef.current)
    }
  }, [])

  return { ...state, refresh: fetchUsage }
}
