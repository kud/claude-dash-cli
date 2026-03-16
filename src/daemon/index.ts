#!/usr/bin/env node
import fs from "node:fs"
import net from "node:net"
import { EventEmitter } from "node:events"
import type { HookEvent, TuiMessage, PermissionDecision } from "../types.js"
import { createInitialState, applyEvent } from "./state.js"
import { createHookServer } from "./hook-server.js"
import { createTuiServer } from "./tui-server.js"

const HOOK_SOCKET_PATH = "/tmp/claude-dash.sock"
const TUI_SOCKET_PATH = "/tmp/claude-dash-tui.sock"
const PID_PATH = "/tmp/claude-dash.pid"
const PERMISSION_TIMEOUT_MS = 30_000

const unlinkIfExists = (path: string): void => {
  try {
    fs.unlinkSync(path)
  } catch {}
}

unlinkIfExists(HOOK_SOCKET_PATH)
unlinkIfExists(TUI_SOCKET_PATH)
fs.writeFileSync(PID_PATH, String(process.pid))

let state = createInitialState()
const emitter = new EventEmitter()
const pendingConnections = new Map<string, net.Socket>()
const permissionTimers = new Map<string, ReturnType<typeof setTimeout>>()

const onEvent = (event: HookEvent): void => {
  state = applyEvent(state, event)
  emitter.emit("change")

  if (event.hookEventName === "PermissionRequest" && event.connectionId) {
    const { connectionId } = event
    const timer = setTimeout(() => {
      const socket = pendingConnections.get(connectionId)
      if (socket && !socket.destroyed) {
        const decision: PermissionDecision = { decision: "ask" }
        socket.write(JSON.stringify(decision) + "\n")
        socket.end()
      }
      pendingConnections.delete(connectionId)
      state = {
        ...state,
        pendingPermissions: state.pendingPermissions.filter(
          (p) => p.connectionId !== connectionId,
        ),
      }
      permissionTimers.delete(connectionId)
      emitter.emit("change")
    }, PERMISSION_TIMEOUT_MS)
    permissionTimers.set(connectionId, timer)
  }
}

const onDecision = (msg: TuiMessage): void => {
  if (msg.type !== "PermissionDecision") return

  const timer = permissionTimers.get(msg.connectionId)
  if (timer) {
    clearTimeout(timer)
    permissionTimers.delete(msg.connectionId)
  }

  const socket = pendingConnections.get(msg.connectionId)
  if (socket && !socket.destroyed) {
    const decision: PermissionDecision = { decision: msg.decision }
    socket.write(JSON.stringify(decision) + "\n")
    socket.end()
  }

  pendingConnections.delete(msg.connectionId)
  state = {
    ...state,
    pendingPermissions: state.pendingPermissions.filter(
      (p) => p.connectionId !== msg.connectionId,
    ),
  }
  emitter.emit("change")
}

const hookServer = createHookServer(state, onEvent, pendingConnections)
const tuiServer = createTuiServer(() => state, onDecision, emitter)

hookServer.listen(HOOK_SOCKET_PATH, () => {
  process.stderr.write(`claude-dash daemon listening on ${HOOK_SOCKET_PATH}\n`)
})

tuiServer.listen(TUI_SOCKET_PATH, () => {
  process.stderr.write(
    `claude-dash TUI server listening on ${TUI_SOCKET_PATH}\n`,
  )
})

hookServer.on("error", (err) =>
  process.stderr.write(`hook server error: ${err.message}\n`),
)
tuiServer.on("error", (err) =>
  process.stderr.write(`tui server error: ${err.message}\n`),
)

const shutdown = (): void => {
  unlinkIfExists(HOOK_SOCKET_PATH)
  unlinkIfExists(TUI_SOCKET_PATH)
  unlinkIfExists(PID_PATH)
  process.exit(0)
}

process.on("SIGTERM", shutdown)
process.on("SIGINT", shutdown)
