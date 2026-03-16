import type {
  HookEvent,
  SessionState,
  SessionStatus,
  PendingPermission,
  ToolHistoryEntry,
  AgentNode,
} from "../types.js"

export type DaemonState = {
  sessions: Map<string, SessionState>
  pendingPermissions: PendingPermission[]
}

export const createInitialState = (): DaemonState => ({
  sessions: new Map(),
  pendingPermissions: [],
})

const getOrCreateSession = (
  state: DaemonState,
  event: HookEvent,
): SessionState =>
  state.sessions.get(event.sessionId) ?? {
    sessionId: event.sessionId,
    status: "waiting_for_input",
    cwd: event.cwd,
    permissionMode: event.permissionMode,
    transcriptPath: event.transcriptPath,
    pid: event.pid,
    tty: event.tty,
    startedAt: event.ts,
    lastEventAt: event.ts,
    currentTool: null,
    currentToolInput: null,
    currentToolUseId: null,
    toolHistory: [],
    agents: [],
  }

const updateSession = (
  state: DaemonState,
  sessionId: string,
  updates: Partial<SessionState>,
): DaemonState => {
  const existing = state.sessions.get(sessionId)
  if (!existing) return state
  const updated = new Map(state.sessions)
  updated.set(sessionId, { ...existing, ...updates })
  return { ...state, sessions: updated }
}

const upsertSession = (
  state: DaemonState,
  session: SessionState,
): DaemonState => {
  const updated = new Map(state.sessions)
  updated.set(session.sessionId, session)
  return { ...state, sessions: updated }
}

const withStatus = (
  state: DaemonState,
  event: HookEvent,
  status: SessionStatus,
): DaemonState => {
  const session = getOrCreateSession(state, event)
  return upsertSession(state, { ...session, status, lastEventAt: event.ts })
}

const applySessionStart = (
  state: DaemonState,
  event: HookEvent,
): DaemonState => {
  const existing = state.sessions.get(event.sessionId)
  const session: SessionState = existing
    ? {
        ...existing,
        cwd: event.cwd,
        permissionMode: event.permissionMode,
        transcriptPath: event.transcriptPath,
        pid: event.pid,
        tty: event.tty,
        status: "waiting_for_input",
        lastEventAt: event.ts,
      }
    : { ...getOrCreateSession(state, event), status: "waiting_for_input" }
  return upsertSession(state, session)
}

const applyPreToolUse = (state: DaemonState, event: HookEvent): DaemonState => {
  const session = getOrCreateSession(state, event)
  const entry: ToolHistoryEntry = {
    toolName: event.toolName ?? "",
    toolInput: event.toolInput ?? {},
    toolUseId: event.toolUseId,
    startedAt: event.ts,
  }
  return upsertSession(state, {
    ...session,
    status: "running_tool",
    currentTool: event.toolName ?? null,
    currentToolInput: event.toolInput ?? null,
    currentToolUseId: event.toolUseId ?? null,
    lastEventAt: event.ts,
    toolHistory: [...session.toolHistory, entry],
  })
}

const applyPostToolUse = (
  state: DaemonState,
  event: HookEvent,
): DaemonState => {
  const session = getOrCreateSession(state, event)
  const toolHistory = session.toolHistory.map((entry) =>
    entry.toolUseId === event.toolUseId
      ? {
          ...entry,
          toolOutput: event.toolOutput,
          success: event.success,
          endedAt: event.ts,
        }
      : entry,
  )
  return upsertSession(state, {
    ...session,
    status: "processing",
    currentTool: null,
    currentToolInput: null,
    currentToolUseId: null,
    lastEventAt: event.ts,
    toolHistory,
  })
}

const applyPermissionRequest = (
  state: DaemonState,
  event: HookEvent,
): DaemonState => {
  const session = getOrCreateSession(state, event)
  const pending: PendingPermission = {
    connectionId: event.connectionId!,
    sessionId: event.sessionId,
    toolName: event.toolName ?? "",
    toolInput: event.toolInput ?? {},
    toolUseId: event.toolUseId,
    cwd: event.cwd,
    requestedAt: event.ts,
  }
  return {
    ...upsertSession(state, {
      ...session,
      status: "waiting_for_approval",
      lastEventAt: event.ts,
    }),
    pendingPermissions: [...state.pendingPermissions, pending],
  }
}

const applySubagentStop = (
  state: DaemonState,
  event: HookEvent,
): DaemonState => {
  const session = state.sessions.get(event.sessionId)
  if (!session) return state

  const agentId = event.toolUseId ?? event.sessionId
  const existingAgent = session.agents.find((a) => a.agentId === agentId)
  const stoppedAgent: AgentNode = existingAgent
    ? { ...existingAgent, status: "stopped", stoppedAt: event.ts }
    : {
        agentId,
        sessionId: event.sessionId,
        status: "stopped",
        startedAt: event.ts,
        stoppedAt: event.ts,
      }

  const agents = existingAgent
    ? session.agents.map((a) => (a.agentId === agentId ? stoppedAgent : a))
    : [...session.agents, stoppedAgent]

  return upsertSession(state, { ...session, agents, lastEventAt: event.ts })
}

const applyNotification = (
  state: DaemonState,
  event: HookEvent,
): DaemonState => {
  const session = getOrCreateSession(state, event)
  const status: SessionStatus =
    event.notificationType === "idle_prompt"
      ? "waiting_for_input"
      : "processing"
  return upsertSession(state, {
    ...session,
    status,
    lastEventAt: event.ts,
    lastNotification: event.message,
  })
}

export const applyEvent = (
  state: DaemonState,
  event: HookEvent,
): DaemonState => {
  switch (event.hookEventName) {
    case "SessionStart":
      return applySessionStart(state, event)
    case "SessionEnd":
      return withStatus(state, event, "ended")
    case "UserPromptSubmit":
      return withStatus(state, event, "processing")
    case "PreToolUse":
      return applyPreToolUse(state, event)
    case "PostToolUse":
      return applyPostToolUse(state, event)
    case "PermissionRequest":
      return applyPermissionRequest(state, event)
    case "Stop":
      return updateSession(
        withStatus(state, event, "waiting_for_input"),
        event.sessionId,
        { currentTool: null, currentToolInput: null, currentToolUseId: null },
      )
    case "SubagentStop":
      return applySubagentStop(state, event)
    case "Notification":
      return applyNotification(state, event)
    case "PreCompact":
      return withStatus(state, event, "compacting")
    default:
      return upsertSession(state, {
        ...getOrCreateSession(state, event),
        lastEventAt: event.ts,
      })
  }
}
