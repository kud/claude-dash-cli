import React from "react"
import { Box, Text } from "ink"
import type { PendingPermission } from "../../types.js"
import { abbreviateHome } from "../utils.js"

type PermissionOverlayProps = {
  permission: PendingPermission
  onAllow: () => void
  onDeny: () => void
}

const truncateLines = (
  json: string,
  maxLines: number,
  maxCharsPerLine: number,
): string =>
  json
    .split("\n")
    .slice(0, maxLines)
    .map((line) =>
      line.length > maxCharsPerLine
        ? line.slice(0, maxCharsPerLine - 3) + "..."
        : line,
    )
    .join("\n")

export const PermissionOverlay = ({ permission }: PermissionOverlayProps) => {
  const inputJson = truncateLines(
    JSON.stringify(permission.toolInput, null, 2),
    8,
    80,
  )
  const cwd = abbreviateHome(permission.cwd)

  return (
    <Box
      borderStyle="round"
      borderColor="yellow"
      flexDirection="column"
      paddingX={2}
      paddingY={1}
    >
      <Box marginBottom={1}>
        <Text color="yellow" bold>
          ⚠ Permission Request
        </Text>
      </Box>
      <Box>
        <Text dimColor>session </Text>
        <Text>{permission.sessionId.slice(0, 8)}</Text>
        <Text> </Text>
        <Text dimColor>{cwd}</Text>
      </Box>
      <Box marginTop={1}>
        <Text dimColor>tool </Text>
        <Text bold color="cyan">
          {permission.toolName}
        </Text>
      </Box>
      <Box marginTop={1} flexDirection="column">
        <Text dimColor>{inputJson}</Text>
      </Box>
      <Box marginTop={1}>
        <Text color="green">[a] Allow</Text>
        <Text> </Text>
        <Text color="red">[d] Deny</Text>
      </Box>
    </Box>
  )
}
