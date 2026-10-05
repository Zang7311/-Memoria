export interface GoalStep { text: string; status: 'pending' | 'doing' | 'done' | 'failed' }
export interface GoalBudget { max_runs: number; max_seconds_per_run: number; max_steps_per_run?: number }
export interface AgentGoal {
  id: string
  title: string
  description: string
  status: 'active' | 'paused' | 'done' | 'blocked' | 'cancelled'
  steps: GoalStep[]
  progress: string
  next_action: string | null
  blocked_reason: string | null
  budget: GoalBudget
  used: { runs: number; total_seconds: number; calls?: number }
  checkpoints: { at: number; summary: string; outcome: 'ok' | 'partial' | 'failed' | 'blocked' }[]
  created_at: number
  updated_at: number
  auto_advance: boolean
  auto_interval_secs: number
  auto_runs: number
}
export interface GoalSnapshot { goals: AgentGoal[]; running_id: string | null; automatic: boolean }
export interface GoalReport { goal: AgentGoal; message: string }
