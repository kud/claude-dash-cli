#!/usr/bin/env node
import fs from "node:fs"
import net from "node:net"
import { spawnSync } from "node:child_process"
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

const log = (...args: unknown[]): void => {
  process.stderr.write(`[${new Date().toISOString()}] ${args.join(" ")}\n`)
}

const onEvent = (event: HookEvent): void => {
  log(
    `← ${event.hookEventName} session=${event.sessionId.slice(0, 8)} ${event.toolName ?? ""}`,
  )
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

const getPpid = (pid: number): number | null => {
  const r = spawnSync("ps", ["-p", String(pid), "-o", "ppid="], {
    encoding: "utf8",
  })
  const n = parseInt(r.stdout.trim(), 10)
  return isNaN(n) || n <= 0 ? null : n
}

const getAncestors = (pid: number): Set<number> => {
  const ancestors = new Set<number>()
  let current: number | null = pid
  while (current !== null && current > 1) {
    const ppid = getPpid(current)
    if (ppid === null || ppid === current || ancestors.has(ppid)) break
    ancestors.add(ppid)
    current = ppid
  }
  return ancestors
}

const sendToClaudePid = (claudePid: number, text: string): void => {
  const result = spawnSync(
    "tmux",
    ["list-panes", "-a", "-F", "#{pane_id} #{pane_pid}"],
    { encoding: "utf8" },
  )
  if (result.status !== 0) return

  const ancestors = getAncestors(claudePid)
  ancestors.add(claudePid)

  for (const line of result.stdout.trim().split("\n")) {
    const [paneId, panePidStr] = line.split(" ")
    const panePid = parseInt(panePidStr, 10)
    if (!paneId || isNaN(panePid)) continue
    if (ancestors.has(panePid)) {
      spawnSync("tmux", ["send-keys", "-t", paneId, "-l", text])
      spawnSync("tmux", ["send-keys", "-t", paneId, "Enter"])
      return
    }
  }
}

const onDecision = (msg: TuiMessage): void => {
  if (msg.type === "SendMessage") {
    const session = state.sessions.get(msg.sessionId)
    if (session?.pid) sendToClaudePid(session.pid, msg.text)
    return
  }
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
