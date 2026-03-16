#!/usr/bin/env node
import fs from "node:fs"
import path from "node:path"
import { execSync } from "node:child_process"

const SETTINGS_PATH = path.join(
  process.env["HOME"] ?? "~",
  ".claude",
  "settings.json",
)

const HOOK_EVENTS = [
  "PreToolUse",
  "PostToolUse",
  "Stop",
  "Notification",
  "PermissionRequest",
  "PreCompact",
] as const

const resolveHookBin = (): string => {
  // Resolve absolute path to the built hook binary relative to this file.
  // __dirname equiv in ESM via import.meta.url isn't available in plain TS,
  // so we use process.argv[1] which points to the running script.
  const thisFile = process.argv[1]!
  const projectRoot = path.resolve(path.dirname(thisFile), "../..")
  const hookBin = path.join(projectRoot, "dist", "hook", "index.js")

  if (!fs.existsSync(hookBin)) {
    process.stderr.write(
      "claude-dash: dist/hook/index.js not found — building first…\n",
    )
    execSync("npm run build", { cwd: projectRoot, stdio: "inherit" })
  }

  return hookBin
}

const readSettings = (): Record<string, unknown> => {
  if (!fs.existsSync(SETTINGS_PATH)) return {}
  try {
    return JSON.parse(fs.readFileSync(SETTINGS_PATH, "utf8")) as Record<
      string,
      unknown
    >
  } catch {
    process.stderr.write(`claude-dash: could not parse ${SETTINGS_PATH}\n`)
    process.exit(1)
  }
}

const alreadyRegistered = (hooks: unknown, command: string): boolean => {
  if (!Array.isArray(hooks)) return false
  return hooks.some((group) => {
    if (!group || typeof group !== "object") return false
    const inner = (group as Record<string, unknown>)["hooks"]
    if (!Array.isArray(inner)) return false
    return inner.some(
      (h) =>
        h &&
        typeof h === "object" &&
        (h as Record<string, unknown>)["command"] === command,
    )
  })
}

const hookEntry = (command: string) => ({
  hooks: [{ type: "command", command: `node ${command}` }],
})

const install = (): void => {
  const hookBin = resolveHookBin()
  const settings = readSettings()
  const hooks = (settings["hooks"] ?? {}) as Record<string, unknown>

  let registered = 0
  let skipped = 0

  for (const event of HOOK_EVENTS) {
    const existing = (hooks[event] ?? []) as unknown[]
    if (alreadyRegistered(existing, `node ${hookBin}`)) {
      process.stdout.write(`  ✓ ${event} — already registered\n`)
      skipped++
      continue
    }
    hooks[event] = [...existing, hookEntry(hookBin)]
    process.stdout.write(`  + ${event} — registered\n`)
    registered++
  }

  settings["hooks"] = hooks
  fs.mkdirSync(path.dirname(SETTINGS_PATH), { recursive: true })
  fs.writeFileSync(SETTINGS_PATH, JSON.stringify(settings, null, 2) + "\n")

  process.stdout.write(
    `\nclaude-dash: ${registered} hook(s) registered, ${skipped} already present\n`,
  )
  process.stdout.write(`hook binary: ${hookBin}\n`)
  process.stdout.write(`settings:    ${SETTINGS_PATH}\n`)

  if (registered > 0) {
    process.stdout.write(
      "\nRestart any running Claude Code sessions to pick up the new hooks.\n",
    )
  }
}

install()
