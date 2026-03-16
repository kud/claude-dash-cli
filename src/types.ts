export type SessionStatus =
  | "waiting_for_input"
  | "processing"
  | "running_tool"
  | "waiting_for_approval"
  | "compacting"
  | "ended"

export type ToolHistoryEntry = {
  toolName: string
  toolInput: Record<string, unknown>
  toolOutput?: string
  success?: boolean
  toolUseId?: string
  startedAt: number
  endedAt?: number
}

export type AgentNode = {
  agentId: string
  sessionId: string
  status: "running" | "stopped"
  startedAt: number
  stoppedAt?: number
}

export type SessionState = {
  sessionId: string
  status: SessionStatus
  cwd: string
  permissionMode: string
  transcriptPath: string
  pid: number
  tty: string | null
  startedAt: number
  lastEventAt: number
  currentTool: string | null
  currentToolInput: Record<string, unknown> | null
  currentToolUseId: string | null
  toolHistory: ToolHistoryEntry[]
  agents: AgentNode[]
  lastNotification?: string
}

export type PendingPermission = {
  connectionId: string
  sessionId: string
  toolName: string
  toolInput: Record<string, unknown>
  toolUseId?: string
  cwd: string
  requestedAt: number
}

export type HookEvent = {
  sessionId: string
  transcriptPath: string
  cwd: string
  permissionMode: string
  hookEventName: string
  pid: number
  tty: string | null
  ts: number
  toolName?: string
  toolInput?: Record<string, unknown>
  toolOutput?: string
  toolUseId?: string
  success?: boolean
  notificationType?: string
  message?: string
  connectionId?: string
}

export type PermissionDecision = {
  decision: "allow" | "deny" | "ask"
  reason?: string
}

export type DaemonMessage =
  | {
      type: "StateSnapshot"
      sessions: SessionState[]
      pendingPermissions: PendingPermission[]
    }
  | {
      type: "StateDelta"
      sessions: SessionState[]
      pendingPermissions: PendingPermission[]
    }

export type TuiMessage = {
  type: "PermissionDecision"
  connectionId: string
  decision: "allow" | "deny"
  reason?: string
}
