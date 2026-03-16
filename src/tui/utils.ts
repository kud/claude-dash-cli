export const formatDuration = (ms: number): string => {
  const seconds = Math.floor(ms / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  if (hours >= 1) return `${hours}h ${minutes % 60}m`
  return minutes >= 1 ? `${minutes}m ${seconds % 60}s` : `${seconds}s`
}

export const truncMid = (s: string, max: number): string => {
  if (s.length <= max) return s
  const tail = Math.floor(max / 3)
  const head = max - tail - 3
  return s.slice(0, head) + "..." + s.slice(-tail)
}

export const abbreviateHome = (cwd: string): string =>
  cwd.replace(process.env["HOME"] ?? "", "~")

export const progressBar = (pct: number, width = 20): string => {
  const clamped = Math.min(100, Math.max(0, pct))
  const filled = Math.round((clamped / 100) * width)
  return "●".repeat(filled) + "○".repeat(width - filled)
}

export const fmtCost = (n: number): string => `$${n.toFixed(2)}`

export const fmtTokens = (n: number): string =>
  n >= 1_000_000
    ? `${(n / 1_000_000).toFixed(2)}M tok`
    : n >= 1_000
      ? `${(n / 1_000).toFixed(0)}k tok`
      : `${n} tok`

export const fmtDelta = (now: number, prev: number): string => {
  const diff = now - prev
  const sign = diff >= 0 ? "+" : ""
  const pct = prev > 0 ? ((diff / prev) * 100).toFixed(1) : "—"
  return `${sign}${fmtCost(diff)} (${sign}${pct}%)`
}

export const toolSummary = (
  toolName: string,
  toolInput: Record<string, unknown>,
): string => {
  const file = (toolInput["file_path"] ??
    toolInput["path"] ??
    toolInput["filePath"]) as string | undefined
  const pattern = toolInput["pattern"] as string | undefined
  const command = toolInput["command"] as string | undefined
  const description = toolInput["description"] as string | undefined
  const query = toolInput["query"] as string | undefined
  const url = toolInput["url"] as string | undefined

  const hint = file ?? command ?? pattern ?? description ?? query ?? url

  if (!hint) return toolName
  const abbrev = abbreviateHome(String(hint))
  return `${toolName}: ${truncMid(abbrev, 48)}`
}

const shortPath = (p: string): string =>
  abbreviateHome(p).split("/").slice(-2).join("/")

const lineCount = (s: unknown): number =>
  typeof s === "string" ? s.split("\n").length : 0

export type ToolDetail = {
  summary: string
  diff?: { removed: number; added: number }
  color: string
}

export const formatToolInput = (
  toolName: string,
  toolInput: Record<string, unknown>,
): ToolDetail => {
  const file = (toolInput["file_path"] ?? toolInput["path"]) as
    | string
    | undefined
  const command = toolInput["command"] as string | undefined

  switch (toolName) {
    case "Read":
      return { summary: file ? shortPath(file) : "—", color: "cyan" }

    case "Write": {
      const lines = lineCount(toolInput["content"])
      return {
        summary: file ? shortPath(file) : "—",
        diff: { removed: 0, added: lines },
        color: "green",
      }
    }

    case "Edit":
    case "MultiEdit": {
      const removed = lineCount(toolInput["old_string"])
      const added = lineCount(toolInput["new_string"])
      return {
        summary: file ? shortPath(file) : "—",
        diff: { removed, added },
        color: "yellow",
      }
    }

    case "Bash":
      return {
        summary: truncMid(String(command ?? "").replace(/\s+/g, " "), 52),
        color: "magenta",
      }

    case "Grep":
      return {
        summary:
          `/${toolInput["pattern"]}/ ${file ? `in ${shortPath(file)}` : ""}`.trim(),
        color: "blue",
      }

    case "Glob":
      return {
        summary: String(toolInput["pattern"] ?? ""),
        color: "blue",
      }

    case "Agent":
      return {
        summary: truncMid(
          String(toolInput["description"] ?? toolInput["prompt"] ?? "agent"),
          52,
        ),
        color: "magenta",
      }

    case "WebFetch":
    case "WebSearch":
      return {
        summary: truncMid(
          String(toolInput["url"] ?? toolInput["query"] ?? ""),
          52,
        ),
        color: "cyan",
      }

    default: {
      const hint = file ?? command ?? Object.values(toolInput)[0]
      return {
        summary: hint ? truncMid(abbreviateHome(String(hint)), 52) : "—",
        color: "white",
      }
    }
  }
}
