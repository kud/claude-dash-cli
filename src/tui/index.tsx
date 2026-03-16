#!/usr/bin/env node
import path from "node:path"
import net from "node:net"
import { spawn } from "node:child_process"
import { render } from "ink"
import React from "react"
import { App } from "./app.js"

const HOOK_SOCKET_PATH = "/tmp/claude-dash.sock"
const DAEMON_POLL_ATTEMPTS = 20
const DAEMON_POLL_INTERVAL_MS = 200

const thisFile = process.argv[1]!
const isDevMode = thisFile.endsWith(".ts") || thisFile.endsWith(".tsx")

const daemonSourcePath = path
  .join(path.dirname(thisFile), "index.ts")
  .replace("/tui/", "/daemon/")

const daemonDistPath = path
  .join(path.dirname(thisFile), "index.js")
  .replace("/tui/", "/daemon/")

const findTsx = (): string => {
  const projectRoot = path.resolve(path.dirname(thisFile), "../..")
  return path.join(projectRoot, "node_modules", ".bin", "tsx")
}

const socketExists = (): Promise<boolean> =>
  new Promise((resolve) => {
    const socket = net.createConnection(HOOK_SOCKET_PATH)
    socket.on("connect", () => {
      socket.destroy()
      resolve(true)
    })
    socket.on("error", () => resolve(false))
  })

const sleep = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms))

const waitForDaemon = async (): Promise<boolean> => {
  for (let i = 0; i < DAEMON_POLL_ATTEMPTS; i++) {
    await sleep(DAEMON_POLL_INTERVAL_MS)
    if (await socketExists()) return true
  }
  return false
}

const startDaemon = (): void => {
  const [cmd, args] = isDevMode
    ? [findTsx(), [daemonSourcePath]]
    : [process.execPath, [daemonDistPath]]

  const child = spawn(cmd, args, {
    detached: true,
    stdio: "ignore",
  })
  child.unref()
}

const enterFullscreen = (): void => {
  process.stdout.write("\x1B[?1049h") // enter alternate screen buffer
  process.stdout.write("\x1B[?25l") // hide cursor
}

const exitFullscreen = (): void => {
  process.stdout.write("\x1B[?25h") // show cursor
  process.stdout.write("\x1B[?1049l") // exit alternate screen buffer, restores previous content
}

const main = async (): Promise<void> => {
  const running = await socketExists()

  if (!running) {
    startDaemon()
    const ready = await waitForDaemon()
    if (!ready) {
      process.stderr.write("claude-dash: daemon did not start in time\n")
      process.exit(1)
    }
  }

  enterFullscreen()
  process.on("exit", exitFullscreen)
  process.on("SIGINT", () => process.exit(0))
  process.on("SIGTERM", () => process.exit(0))

  render(<App />, { stdout: process.stdout })
}

main().catch((err) => {
  process.stderr.write(`claude-dash: ${err}\n`)
  process.exit(1)
})
