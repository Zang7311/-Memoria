import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { AgentGoal, GoalBudget, GoalReport, GoalSnapshot } from '../types/goals'

export const useGoalStore = defineStore('goals', () => {
  const goals = ref<AgentGoal[]>([])
  const runningId = ref<string | null>(null)
  const automatic = ref(false)
  const error = ref('')
  const feedback = ref('')
  const reminder = ref('')
  const attentionCount = computed(() => goals.value.filter(goal => goal.status === 'active' || goal.status === 'blocked').length)
  const runningTitle = computed(() => goals.value.find(goal => goal.id === runningId.value)?.title || '')
  let unlisten: (() => void) | null = null

  function apply(snapshot: GoalSnapshot) {
    goals.value = snapshot.goals
    runningId.value = snapshot.running_id
    automatic.value = snapshot.automatic
  }
  async function refresh() { apply(await invoke<GoalSnapshot>('list_goals')) }
  async function init() {
    if (!unlisten) unlisten = await listen<GoalSnapshot>('goals_changed', event => apply(event.payload))
    await refresh()
  }
  function dispose() { unlisten?.(); unlisten = null }
  async function perform(action: () => Promise<unknown>) {
    error.value = ''
    try { await action(); await refresh() } catch (reason) { error.value = String(reason) }
  }
  async function create(title: string, description: string, budget: GoalBudget) {
    await perform(async () => {
      const goal = await invoke<AgentGoal>('create_goal', { title, description, budget })
      feedback.value = '我建了一个目标：' + goal.title + '，你可以在「目标」面板里看到并让我继续'
    })
  }
  async function advance(id: string) {
    await perform(async () => { feedback.value = (await invoke<GoalReport>('goal_advance', { id })).message })
  }
  async function setStatus(id: string, status: string) { await perform(() => invoke('set_goal_status', { id, status })) }
  async function settings(goal: AgentGoal, autoAdvance: boolean, interval: number, budget: GoalBudget = goal.budget) {
    await perform(() => invoke('update_goal_settings', { id: goal.id, autoAdvance, autoIntervalSecs: interval, budget }))
  }
  async function remove(id: string) { await perform(() => invoke('delete_goal', { id })) }
  async function remind() {
    reminder.value = ''
    try {
      await refresh()
      const pending = goals.value.filter(goal => goal.status === 'active' || goal.status === 'blocked')
      if (pending.length) reminder.value = '系统消息：你有 ' + pending.length + ' 个进行中或需要决定的目标，可在「目标」面板查看，由你决定是否继续。'
    } catch (reason) { error.value = String(reason) }
  }
  return { goals, runningId, automatic, runningTitle, error, feedback, reminder, attentionCount, init, dispose, refresh, create, advance, setStatus, settings, remove, remind }
})

