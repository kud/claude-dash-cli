import React, { useState } from "react"
import { Box, Text, useInput } from "ink"
import { exec } from "node:child_process"
import type { SessionState } from "../../types.js"
import { abbreviateHome } from "../utils.js"

type NewSessionOverlayProps = {
  recentCwds: string[]
  onClose: () => void
}

const launchSession = (dir: string): string | null => {
  const inTmux = Boolean(process.env["TMUX"])
  if (inTmux) {
    const safeName = dir.split("/").pop()?.slice(0, 20) ?? "claude"
    exec(`tmux new-window -n "${safeName}" -c "${dir}" "claude"`, () => {})
    return null
  }
  // macOS fallback
  exec(
    `osascript -e 'tell application "Terminal" to do script "cd ${dir} && claude"'`,
    () => {},
  )
  return null
}

export const NewSessionOverlay = ({
  recentCwds,
  onClose,
}: NewSessionOverlayProps) => {
  const [input, setInput] = useState("")
  const [launched, setLaunched] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useInput((char, key) => {
    if (key.escape) {
      onClose()
      return
    }
    if (key.return) {
      const dir = input.trim().replace(/^~/, process.env["HOME"] ?? "~")
      if (!dir) return
      try {
        launchSession(dir)
        setLaunched(true)
        setTimeout(onClose, 800)
      } catch (e) {
        setError(String(e))
      }
      return
    }
    if (key.backspace || key.delete) {
      setInput((v) => v.slice(0, -1))
      return
    }
    if (!key.ctrl && !key.meta && char) {
      setInput((v) => v + char)
    }
  })

  const inTmux = Boolean(process.env["TMUX"])

  return (
    <Box
      borderStyle="round"
      borderColor="cyan"
      flexDirection="column"
      paddingX={2}
      paddingY={1}
      width={60}
    >
      <Box marginBottom={1}>
        <Text color="cyan" bold>
          ◆ New Session
        </Text>
        {!inTmux && <Text dimColor> (opens Terminal.app)</Text>}
      </Box>

      <Box marginBottom={1}>
        <Text dimColor>Working directory: </Text>
      </Box>

      <Box
        borderStyle="single"
        borderColor={launched ? "green" : "white"}
        paddingX={1}
        marginBottom={1}
      >
        <Text color="white">{input || " "}</Text>
        <Text color="cyan">▌</Text>
      </Box>

      {recentCwds.length > 0 && !launched && (
        <Box flexDirection="column" marginBottom={1}>
          <Text dimColor>Recent:</Text>
          {recentCwds.slice(0, 4).map((cwd) => (
            <Text key={cwd} dimColor>
              {"  "}
              {abbreviateHome(cwd)}
            </Text>
          ))}
        </Box>
      )}

      {launched && (
        <Box>
          <Text color="green">
            ✓ launching claude in {abbreviateHome(input)}…
          </Text>
        </Box>
      )}

      {error && (
        <Box>
          <Text color="red">✗ {error}</Text>
        </Box>
      )}

      <Box marginTop={1}>
        <Text dimColor>[enter] launch </Text>
        <Text dimColor>[esc] cancel</Text>
      </Box>
    </Box>
  )
}

export const extractRecentCwds = (sessions: SessionState[]): string[] => {
  const seen = new Set<string>()
  return sessions
    .sort((a, b) => b.lastEventAt - a.lastEventAt)
    .map((s) => s.cwd)
    .filter((cwd) => {
      if (seen.has(cwd)) return false
      seen.add(cwd)
      return true
    })
}
