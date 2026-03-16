import net from "node:net"
import { useEffect, useReducer, useRef, useCallback } from "react"
import type {
  SessionState,
  PendingPermission,
  DaemonMessage,
  TuiMessage,
} from "../../types.js"

const TUI_SOCKET_PATH = "/tmp/claude-dash-tui.sock"
const RENDER_DEBOUNCE_MS = 150

type DaemonClientState = {
  sessions: SessionState[]
  pendingPermissions: PendingPermission[]
  connected: boolean
}

type DaemonAction =
  | { type: "connected" }
  | { type: "disconnected" }
  | { type: "message"; msg: DaemonMessage }

const initialState: DaemonClientState = {
  sessions: [],
  pendingPermissions: [],
  connected: false,
}

const reducer = (
  state: DaemonClientState,
  action: DaemonAction,
): DaemonClientState => {
  switch (action.type) {
    case "connected":
      return { ...state, connected: true }
    case "disconnected":
      return { ...state, connected: false }
    case "message":
      return {
        ...state,
        sessions: action.msg.sessions,
        pendingPermissions: action.msg.pendingPermissions,
      }
  }
}

export const useDaemon = () => {
  const [state, dispatch] = useReducer(reducer, initialState)
  const socketRef = useRef<net.Socket | null>(null)
  const pendingMsgRef = useRef<DaemonMessage | null>(null)
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const sendDecision = useCallback(
    (connectionId: string, decision: "allow" | "deny") => {
      const socket = socketRef.current
      if (!socket || socket.destroyed) return
      const msg: TuiMessage = {
        type: "PermissionDecision",
        connectionId,
        decision,
      }
      socket.write(JSON.stringify(msg) + "\n")
    },
    [],
  )

  useEffect(() => {
    let buffer = ""
    let destroyed = false

    const flushPending = () => {
      const msg = pendingMsgRef.current
      if (msg) {
        pendingMsgRef.current = null
        dispatch({ type: "message", msg })
      }
    }

    const scheduleDispatch = (msg: DaemonMessage) => {
      // Permission requests must be dispatched immediately (user needs to act)
      if (msg.pendingPermissions.length > 0) {
        pendingMsgRef.current = null
        if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current)
        dispatch({ type: "message", msg })
        return
      }
      pendingMsgRef.current = msg
      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current)
      debounceTimerRef.current = setTimeout(flushPending, RENDER_DEBOUNCE_MS)
    }

    const connect = () => {
      if (destroyed) return
      const socket = net.createConnection(TUI_SOCKET_PATH)
      socketRef.current = socket

      socket.on("connect", () => dispatch({ type: "connected" }))

      socket.on("data", (chunk) => {
        buffer += chunk.toString()
        const lines = buffer.split("\n")
        buffer = lines.pop() ?? ""

        for (const line of lines) {
          if (!line.trim()) continue
          try {
            const msg = JSON.parse(line) as DaemonMessage
            scheduleDispatch(msg)
          } catch {}
        }
      })

      socket.on("close", () => {
        dispatch({ type: "disconnected" })
        if (!destroyed) setTimeout(connect, 2000)
      })

      socket.on("error", () => {
        dispatch({ type: "disconnected" })
      })
    }

    connect()

    return () => {
      destroyed = true
      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current)
      socketRef.current?.destroy()
    }
  }, [])

  return { ...state, sendDecision }
}
