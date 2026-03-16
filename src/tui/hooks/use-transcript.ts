import fs from "node:fs"
import { useEffect, useReducer, useRef } from "react"

export type TranscriptMessage = {
  id: string
  role: "user" | "assistant"
  text: string
  timestamp: number
}

type State = {
  messages: TranscriptMessage[]
  transcriptPath: string | null
}

type Action =
  | { type: "set_path"; path: string }
  | { type: "set_messages"; messages: TranscriptMessage[] }

const parseContentToText = (content: unknown): string => {
  if (typeof content === "string") return content.trim()
  if (!Array.isArray(content)) return ""

  return content
    .filter((block: unknown) => {
      if (typeof block !== "object" || block === null) return false
      const b = block as Record<string, unknown>
      return b["type"] === "text" && typeof b["text"] === "string"
    })
    .map(
      (block: unknown) => (block as Record<string, unknown>)["text"] as string,
    )
    .join("\n")
    .trim()
}

const parseTranscript = (raw: string): TranscriptMessage[] => {
  const messages: TranscriptMessage[] = []

  for (const line of raw.split("\n")) {
    if (!line.trim()) continue
    try {
      const entry = JSON.parse(line) as Record<string, unknown>
      if (entry["type"] !== "user" && entry["type"] !== "assistant") continue

      const msg = entry["message"] as Record<string, unknown> | undefined
      if (!msg) continue

      const text = parseContentToText(msg["content"])
      if (!text) continue

      const ts = entry["timestamp"]
      const timestamp = ts ? new Date(ts as string).getTime() : Date.now()
      const uuid = (entry["uuid"] as string | undefined) ?? `${timestamp}`

      messages.push({
        id: uuid,
        role: entry["type"] as "user" | "assistant",
        text,
        timestamp,
      })
    } catch {}
  }

  return messages
}

const reducer = (state: State, action: Action): State => {
  switch (action.type) {
    case "set_path":
      return { ...state, transcriptPath: action.path }
    case "set_messages":
      return { ...state, messages: action.messages }
  }
}

export const useTranscript = (transcriptPath: string | null) => {
  const [state, dispatch] = useReducer(reducer, {
    messages: [],
    transcriptPath: null,
  })
  const watcherRef = useRef<fs.StatWatcher | null>(null)

  useEffect(() => {
    if (!transcriptPath) {
      dispatch({ type: "set_messages", messages: [] })
      return
    }

    dispatch({ type: "set_path", path: transcriptPath })

    const load = () => {
      try {
        const raw = fs.readFileSync(transcriptPath, "utf8")
        dispatch({ type: "set_messages", messages: parseTranscript(raw) })
      } catch {}
    }

    load()

    watcherRef.current = fs.watchFile(
      transcriptPath,
      { interval: 500, persistent: false },
      load,
    )

    return () => {
      if (watcherRef.current) {
        fs.unwatchFile(transcriptPath)
        watcherRef.current = null
      }
    }
  }, [transcriptPath])

  return state.messages
}
