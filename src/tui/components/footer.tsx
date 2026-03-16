import React, { memo } from "react"
import { Box, Text } from "ink"

type FooterProps = {
  hasPendingPermissions: boolean
}

export const Footer = memo(({ hasPendingPermissions }: FooterProps) => (
  <Box paddingX={1}>
    <Text dimColor>[q] quit [Q] quit+kill [↑↓/jk] select </Text>
    <Text dimColor={!hasPendingPermissions}>[a] allow [d] deny </Text>
    <Text dimColor>[n] new session [r] refresh usage</Text>
  </Box>
))
