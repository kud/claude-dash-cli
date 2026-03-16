import React, { memo } from "react"
import { Box, Text } from "ink"
import type { TranscriptMessage } from "../hooks/use-transcript.js"

const MAX_WIDTH = 72

const wrapText = (text: string, width: number): string[] => {
  const result: string[] = []
  for (const line of text.split("\n")) {
    if (line.length <= width) {
      result.push(line)
      continue
    }
    let remaining = line
    while (remaining.length > width) {
      const cut = remaining.lastIndexOf(" ", width)
      const pos = cut > 0 ? cut : width
      result.push(remaining.slice(0, pos))
      remaining = remaining.slice(pos).trimStart()
    }
    if (remaining) result.push(remaining)
  }
  return result
}

const UserMessage = memo(({ text }: { text: string }) => {
  const lines = wrapText(text, MAX_WIDTH)
  return (
    <Box flexDirection="row" justifyContent="flex-end" marginBottom={1}>
      <Box borderStyle="round" borderColor="gray" paddingX={1}>
        {lines.map((line, i) => (
          <Text key={i} color="white">
            {line}
          </Text>
        ))}
      </Box>
    </Box>
  )
})

const AssistantMessage = memo(({ text }: { text: string }) => {
  const lines = text.split("\n")
  return (
    <Box flexDirection="column" marginBottom={1}>
      {lines.map((line, i) => {
        const isBullet = /^\s*[-•*]\s/.test(line)
        const isCode = line.startsWith("  ") || line.startsWith("\t")
        const color = isCode ? "cyan" : "white"
        const content = isBullet ? line.replace(/^\s*[-•*]\s/, "• ") : line
        return (
          <Box key={i} paddingLeft={1}>
            <Text color={color} dimColor={isCode}>
              {content || " "}
            </Text>
          </Box>
        )
      })}
    </Box>
  )
})

type ChatMessageProps = { message: TranscriptMessage }

export const ChatMessage = memo(({ message }: ChatMessageProps) => {
  if (message.role === "user") return <UserMessage text={message.text} />
  return <AssistantMessage text={message.text} />
})
