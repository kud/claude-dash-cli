#!/usr/bin/env node
import net from "node:net"
import { execSync } from "node:child_process"
import type { HookEvent, PermissionDecision } from "../types.js"

const SOCKET_PATH = "/tmp/claude-dash.sock"

const resolveTty = (ppid: number): string | null => {
  try {
    const raw = execSync(`ps -p ${ppid} -o tty=`, { timeout: 500 })
      .toString()
      .trim()
    if (!raw || raw === "??") return null
    return raw.startsWith("/dev/") ? raw : `/dev/${raw}`
  } catch {
    return null
  }
}

const readStdin = (): Promise<string> =>
  new Promise((resolve) => {
    const chunks: Buffer[] = []
    process.stdin.on("data", (chunk) => chunks.push(chunk))
    process.stdin.on("end", () =>
      resolve(Buffer.concat(chunks).toString("utf8")),
    )
  })

const normalizeEvent = (raw: Record<string, unknown>): HookEvent => {
  const ppid = process.ppid
  return {
    sessionId: raw["session_id"] as string,
    transcriptPath: (raw["transcript_path"] as string) ?? "",
    cwd: (raw["cwd"] as string) ?? process.cwd(),
    permissionMode: (raw["permission_mode"] as string) ?? "",
    hookEventName: raw["hook_event_name"] as string,
    pid: ppid,
    tty: resolveTty(ppid),
    ts: Date.now(),
    toolName: raw["tool_name"] as string | undefined,
    toolInput: raw["tool_input"] as Record<string, unknown> | undefined,
    toolOutput: raw["tool_output"] as string | undefined,
    toolUseId: raw["tool_use_id"] as string | undefined,
    success: raw["success"] as boolean | undefined,
    notificationType: raw["notification_type"] as string | undefined,
    message: raw["message"] as string | undefined,
  }
}

const sendFireAndForget = (event: HookEvent): void => {
  const socket = net.createConnection(SOCKET_PATH)
  socket.on("error", () => {})
  socket.on("connect", () => {
    socket.write(JSON.stringify(event) + "\n")
    socket.end()
  })
}

const sendPermissionRequest = (event: HookEvent): Promise<PermissionDecision> =>
  new Promise((resolve) => {
    const socket = net.createConnection(SOCKET_PATH)
    let buffer = ""

    socket.on("error", () => resolve({ decision: "ask" }))

    socket.on("connect", () => {
      socket.write(JSON.stringify(event) + "\n")
    })

    socket.on("data", (chunk) => {
      buffer += chunk.toString()
      const lines = buffer.split("\n")
      buffer = lines.pop() ?? ""
      for (const line of lines) {
        if (!line.trim()) continue
        try {
          const parsed = JSON.parse(line) as PermissionDecision
          resolve(parsed)
          socket.destroy()
        } catch {}
      }
    })

    socket.on("close", () => resolve({ decision: "ask" }))
  })

const outputPermissionResult = (decision: PermissionDecision): void => {
  const behavior = decision.decision === "deny" ? "deny" : "allow"
  process.stdout.write(
    JSON.stringify({
      hookSpecificOutput: {
        hookEventName: "PermissionRequest",
        decision: { behavior },
      },
    }) + "\n",
  )
}

const main = async (): Promise<void> => {
  const raw = await readStdin()
  if (!raw.trim()) return

  let parsed: Record<string, unknown>
  try {
    parsed = JSON.parse(raw) as Record<string, unknown>
  } catch {
    return
  }

  const socketExists = await new Promise<boolean>((resolve) => {
    const socket = net.createConnection(SOCKET_PATH)
    socket.on("connect", () => {
      socket.destroy()
      resolve(true)
    })
    socket.on("error", () => resolve(false))
  })

  if (!socketExists) return

  const event = normalizeEvent(parsed)

  if (event.hookEventName === "PermissionRequest") {
    const decision = await sendPermissionRequest(event)
    outputPermissionResult(decision)
  } else {
    sendFireAndForget(event)
  }
}

main().catch(() => {})
