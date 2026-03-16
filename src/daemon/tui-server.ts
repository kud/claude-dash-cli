import net from "node:net"
import { EventEmitter } from "node:events"
import type { DaemonMessage, TuiMessage } from "../types.js"
import type { DaemonState } from "./state.js"

const serializeState = (
  state: DaemonState,
  type: "StateSnapshot" | "StateDelta",
): string => {
  const msg: DaemonMessage = {
    type,
    sessions: Array.from(state.sessions.values()),
    pendingPermissions: state.pendingPermissions,
  }
  return JSON.stringify(msg) + "\n"
}

export const createTuiServer = (
  getState: () => DaemonState,
  onDecision: (msg: TuiMessage) => void,
  emitter: EventEmitter,
): net.Server => {
  const server = net.createServer((socket) => {
    socket.write(serializeState(getState(), "StateSnapshot"))

    const onStateChange = () => {
      if (!socket.destroyed) {
        socket.write(serializeState(getState(), "StateDelta"))
      }
    }

    emitter.on("change", onStateChange)

    let buffer = ""
    socket.on("data", (chunk) => {
      buffer += chunk.toString()
      const lines = buffer.split("\n")
      buffer = lines.pop() ?? ""

      for (const line of lines) {
        if (!line.trim()) continue
        try {
          const msg = JSON.parse(line) as TuiMessage
          onDecision(msg)
        } catch {}
      }
    })

    socket.on("close", () => emitter.off("change", onStateChange))
    socket.on("error", () => emitter.off("change", onStateChange))
  })

  return server
}
