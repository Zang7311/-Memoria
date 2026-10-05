export interface ToolCallRecord {
  name: string
  intent: string | null
  ok: boolean
  brief: string
  at: number
}

export interface AuditEntry {
  id: string
  at: number
  kind: string
  title: string
  request_id: string | null
  session_id: string | null
  goal_id: string | null
  tools: ToolCallRecord[]
  files_changed: string[]
  result: string
  summary: string
  duration_ms: number
}

export interface AuditQuery {
  keyword?: string
  kind?: string
  result?: string
  since?: number
  until?: number
}
