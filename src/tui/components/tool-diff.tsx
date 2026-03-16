import React, { memo } from "react"
import { Box, Text } from "ink"
import type { ToolHistoryEntry } from "../../types.js"

const MAX_LINES = 12

const DiffLine = ({
  prefix,
  line,
  color,
}: {
  prefix: string
  line: string
  color: string
}) => (
  <Box paddingLeft={2}>
    <Text color={color as "red" | "green" | "cyan" | "yellow"}>{prefix} </Text>
    <Text
      color={color as "red" | "green" | "cyan" | "yellow"}
      dimColor={color === "white"}
    >
      {line}
    </Text>
  </Box>
)

const Truncated = ({ n }: { n: number }) => (
  <Box paddingLeft={2}>
    <Text dimColor>
      … {n} more line{n > 1 ? "s" : ""}
    </Text>
  </Box>
)

const EditDiff = ({ toolInput }: { toolInput: Record<string, unknown> }) => {
  const oldLines = String(toolInput["old_string"] ?? "").split("\n")
  const newLines = String(toolInput["new_string"] ?? "").split("\n")

  const removedShown = oldLines.slice(0, MAX_LINES)
  const addedShown = newLines.slice(0, MAX_LINES)
  const removedExtra = oldLines.length - removedShown.length
  const addedExtra = newLines.length - addedShown.length

  return (
    <Box flexDirection="column">
      {removedShown.map((line, i) => (
        <DiffLine key={`r${i}`} prefix="-" line={line} color="red" />
      ))}
      {removedExtra > 0 && <Truncated n={removedExtra} />}
      {addedShown.map((line, i) => (
        <DiffLine key={`a${i}`} prefix="+" line={line} color="green" />
      ))}
      {addedExtra > 0 && <Truncated n={addedExtra} />}
    </Box>
  )
}

const WriteDiff = ({ toolInput }: { toolInput: Record<string, unknown> }) => {
  const lines = String(toolInput["content"] ?? "").split("\n")
  const shown = lines.slice(0, MAX_LINES)
  const extra = lines.length - shown.length

  return (
    <Box flexDirection="column">
      {shown.map((line, i) => (
        <DiffLine key={i} prefix="+" line={line} color="green" />
      ))}
      {extra > 0 && <Truncated n={extra} />}
    </Box>
  )
}

const BashOutput = ({ output }: { output: string }) => {
  const lines = output.split("\n").filter(Boolean).slice(0, MAX_LINES)
  const extra = output.split("\n").filter(Boolean).length - lines.length

  return (
    <Box flexDirection="column">
      {lines.map((line, i) => (
        <DiffLine key={i} prefix=">" line={line} color="white" />
      ))}
      {extra > 0 && <Truncated n={extra} />}
    </Box>
  )
}

type ToolDiffProps = { entry: ToolHistoryEntry }

export const ToolDiff = memo(({ entry }: ToolDiffProps) => {
  const { toolName, toolInput, toolOutput } = entry

  if (toolName === "Edit" || toolName === "MultiEdit")
    return <EditDiff toolInput={toolInput} />

  if (toolName === "Write") return <WriteDiff toolInput={toolInput} />

  if (toolOutput?.trim()) return <BashOutput output={toolOutput} />

  return null
})
