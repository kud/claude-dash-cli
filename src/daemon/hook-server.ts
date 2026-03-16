import net from "node:net"
import crypto from "node:crypto"
import type { HookEvent } from "../types.js"
import type { DaemonState } from "./state.js"

export const createHookServer = (
  state: DaemonState,
  onEvent: (event: HookEvent) => void,
  pendingConnections: Map<string, net.Socket>,
): net.Server => {
  const server = net.createServer((socket) => {
    let buffer = ""

    socket.on("data", (chunk) => {
      buffer += chunk.toString()
      const lines = buffer.split("\n")
      buffer = lines.pop() ?? ""

      for (const line of lines) {
        if (!line.trim()) continue
        try {
          const event = JSON.parse(line) as HookEvent

          if (event.hookEventName === "PermissionRequest") {
            const connectionId = crypto.randomUUID()
            event.connectionId = connectionId
            pendingConnections.set(connectionId, socket)
            onEvent(event)
          } else {
            onEvent(event)
            socket.end()
          }
        } catch {}
      }
    })

    socket.on("error", () => {})
  })

  return server
}
