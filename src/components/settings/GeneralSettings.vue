<!-- 设置 · 通用
     与铃的日记 / 铃的能力 / 管理员权限 / Agent 权限 / 长期记忆 /
     开机自启动 / 全局快捷键 / 数据路径 / 配置备份 / 重置 / 工具箱入口 -->
<script setup lang="ts">
import { computed, inject, onMounted, ref } from 'vue'
import AppButton from '../ui/AppButton.vue'
import AppCard from '../ui/AppCard.vue'
import AppField from '../ui/AppField.vue'
import AppInput from '../ui/AppInput.vue'
import AppStatusDot from '../ui/AppStatusDot.vue'
import AppSwitch from '../ui/AppSwitch.vue'
import { useMilestoneStore } from '../../stores/milestoneStore'
import { useSettingStore } from '../../stores/settingStore'
import { MODEL_MODE_LABEL } from '../../types'
import { confirmDialog } from '../../utils/dialog'
import { toastErr, toastOk } from '../../utils/toast'
import {
  detectOllama,
  getAutostart,
  isAdmin,
  registerHotkey,
  restartAsAdmin,
  setAutostart,
} from '../../utils/tauri'
import {
  SETTINGS_TOOLBOX,
  ensureSettingsLoaded,
  exportConfigFile,
  importConfigFromFile,
} from './shared'

const setting = useSettingStore()
const milestone = useMilestoneStore()
const toolbox = inject(SETTINGS_TOOLBOX, null)

// ==================== 与铃的日记（P3：1-3 天一组，折叠摘要行 + 15-40 字正文） ====================
const diaryOpen = ref<number | null>(null)
const showBadges = ref(false)
function toggleDiary(i: number) {
  diaryOpen.value = diaryOpen.value === i ? null : i
}
interface DiaryGroup {
  end: string
  days: number
  chat: number
  tools: number
  topics: string[]
  summary: string
  body: string
}
const diaryGroups = computed<DiaryGroup[]>(() => {
  // daily 已倒序（最新在前），翻转成时间正序后每 3 天一组
  const days = [...milestone.daily].reverse()
  const groups: DiaryGroup[] = []
  for (let i = 0; i < days.length; i += 3) {
    const chunk = days.slice(i, i + 3)
    const chat = chunk.reduce((s, d) => s + d.chat_count, 0)
    const tools = chunk.reduce((s, d) => s + d.tool_count, 0)
    const topics = [...new Set(chunk.flatMap((d) => d.topics))].slice(0, 3)
    const end = chunk[chunk.length - 1].date.slice(5) // mm-dd
    const start = chunk[0].date.slice(5)
    const dayLabel = chunk.length > 1 ? `${start}~${end}` : end
    // 摘要行：mm-dd · 聊了x句 · 用了x次工具 · 话题：xx
    const summary = `${dayLabel} · 聊了${chat}句 · 用了${tools}次工具${topics.length ? ' · 话题：' + topics.join('、') : ''}`
    const body = buildDiaryBody(chunk.length, chat, tools, topics)
    groups.push({ end: dayLabel, days: chunk.length, chat, tools, topics, summary, body })
  }
  return groups.reverse() // 最新在前
})
function buildDiaryBody(dayCount: number, chat: number, tools: number, topics: string[]): string {
  const t = topics.length ? topics.join('、') : ''
  if (chat === 0 && tools === 0) return `这天没什么记录，但铃记得，主人出现过～`
  if (chat === 0) return `主人这一天让铃帮忙用了${tools}次工具，忙忙碌碌的，但能帮上忙铃就很开心～`
  const topicPart = t ? `聊了「${t}」` : '聊了些家常'
  if (dayCount >= 2) return `这几天${topicPart}，共${chat}句、${tools}次工具。和主人待在一起，日子都暖暖的～`
  return `今天${topicPart}，聊了${chat}句${tools ? `、用了${tools}次工具` : ''}。铃把这些都记在日记里啦～`
}

// ==================== 铃的能力（当前模式/模型的能力矩阵） ====================
// 已知模型视觉能力表（未知模型显示「未知」）
const VISION_MODELS = ['vision', 'vl', '4v', '4o', '4.1', 'llava', 'gemini', 'gpt-4', 'claude', 'image', 'omni', '看图', '视觉']
// 本地模式下能力矩阵要显示「当前本地模型」，所以在通用页也探一次 Ollama（模型页会再探）
const ollamaModels = ref<string[]>([])
const abilityMatrix = computed(() => {
  const mode = setting.modelMode
  const model = (mode === 'api' ? setting.apiModel : mode === 'local' ? (ollamaModels.value[0] || '') : '').toLowerCase()
  const isVision = VISION_MODELS.some((k) => model.includes(k))
  return {
    modeLabel: MODEL_MODE_LABEL[mode] || mode,
    modelName: mode === 'api' ? (setting.apiModel || '未设置') : mode === 'local' ? (ollamaModels.value[0] || '未拉取模型') : '内置回复库',
    rows: [
      { name: '文字对话', ok: true, unknown: false, detail: '基础能力，始终可用' },
      {
        name: '图片理解',
        ok: mode === 'script' ? false : isVision,
        unknown: mode === 'api' && !isVision,
        detail: mode === 'script' ? '离线模式不支持' : isVision ? `「${model}」支持视觉` : '当前模型不支持',
      },
      {
        name: '联网能力',
        ok: mode !== 'script',
        unknown: false,
        detail: mode === 'script' ? '完全离线' : mode === 'local' ? '模型本地，联网可选' : '云端调用，需联网',
      },
      { name: '记忆存储', ok: true, unknown: false, detail: '铃的本体能力，所有模式可用' },
      { name: '工具箱', ok: true, unknown: false, detail: '44 个工具，所有模式可用' },
      { name: '离线可用', ok: mode !== 'api', unknown: false, detail: mode === 'api' ? '云端模式需联网' : '完全离线可用' },
    ],
  }
})

// ==================== 管理员权限 / Agent 权限 ====================
const adminState = ref<boolean | null>(null)
const adminMsg = ref('')

async function requestAdmin() {
  const ok = await confirmDialog(
    '将以管理员权限重启应用（会弹出 UAC 提示）。重启后可正常使用：电源模式切换、深度清理内存等特殊工具。',
    { confirmText: '重启并提权' },
  )
  if (!ok) return
  try {
    await restartAsAdmin()
    adminMsg.value = '已请求管理员权限启动。请在弹出的 UAC 窗口点击「是」，然后关闭当前窗口，使用新的管理员窗口。'
  } catch (e) {
    adminMsg.value = `启动失败：${e}`
  }
}

async function toggleRunAsAdmin(value: boolean) {
  setting.runAsAdmin = value
  await setting.update({ run_as_admin: value })
  toastOk(value ? '已开启：下次启动将以管理员身份运行（会弹一次 UAC 确认）' : '已关闭：将以普通权限启动')
}

async function toggleAiToolbox(value: boolean) {
  setting.aiToolbox = value
  await setting.update({ ai_toolbox: value })
  toastOk(value ? '已开启：铃可直接执行工具箱工具' : '已关闭：铃不调用工具箱')
}

type AgentFlag =
  | 'agentAllowDownload'
  | 'agentAllowSoftware'
  | 'agentAllowFileWrite'
  | 'agentAllowShell'
  | 'agentAllowToolForge'
  | 'selfCheckEnabled'
  | 'agentPlanConfirm'

const AGENT_KEYS: Record<AgentFlag, string> = {
  agentAllowDownload: 'agent_allow_download',
  agentAllowSoftware: 'agent_allow_software',
  agentAllowFileWrite: 'agent_allow_file_write',
  agentAllowShell: 'agent_allow_shell',
  agentAllowToolForge: 'agent_allow_tool_forge',
  selfCheckEnabled: 'self_check_enabled',
  agentPlanConfirm: 'agent_plan_confirm',
}

async function toggleAgentFlag(flag: AgentFlag, value: boolean) {
  setting[flag] = value
  await setting.update({ [AGENT_KEYS[flag]]: value })
}

// ==================== 长期记忆 / 开机自启动 / 快捷键 / 数据路径 ====================
function saveLongTermLimit() {
  void setting.update({ long_term_memory_limit: setting.longTermMemoryLimit })
}

const autostart = ref(false)
function onAutostart(value: boolean) {
  autostart.value = value
  void toggleAutostart()
}
async function toggleAutostart() {
  try {
    await setAutostart(autostart.value)
    await setting.update({ autostart: autostart.value })
    toastOk(autostart.value ? '已开启开机自启动' : '已关闭开机自启动')
  } catch (e) {
    toastErr(`${e}`)
  }
}

const hotkey = ref('Ctrl+Alt+L')
async function saveHotkey() {
  try {
    const res = await registerHotkey(hotkey.value)
    await setting.update({ hotkey: hotkey.value })
    if (res.registered) toastOk(`快捷键已生效：${res.accelerator}`)
    else toastErr('注册失败')
  } catch (e) {
    toastErr(`${e}`)
  }
}

async function saveDataPath() {
  await setting.update({ data_path: setting.dataPath })
  toastOk('数据路径已更新（记忆将保存到新路径）')
}

// ==================== 配置备份 / 重置 ====================
const importPath = ref('')
async function onImport() {
  if (await importConfigFromFile(importPath.value)) importPath.value = ''
}

async function doReset() {
  const ok = await confirmDialog('确定恢复所有设置为默认值吗？（保留主密码与已加密密钥）', {
    danger: true,
    confirmText: '恢复默认',
  })
  if (!ok) return
  await setting.resetAll()
  hotkey.value = setting.hotkey
  autostart.value = setting.autostart
  toastOk('已重置为默认设置')
}

onMounted(async () => {
  await ensureSettingsLoaded()
  hotkey.value = setting.hotkey
  autostart.value = setting.autostart
  // P3：确保陪伴记录已加载（防 App.vue 未加载完成）
  if (!milestone.loaded) await milestone.load().catch(() => {})
  try {
    const res = await getAutostart()
    autostart.value = res.enabled
  } catch {
    /* 忽略 */
  }
  try {
    adminState.value = await isAdmin()
  } catch {
    adminState.value = false
  }
  try {
    ollamaModels.value = (await detectOllama()).models
  } catch {
    ollamaModels.value = []
  }
})
</script>

<template>
  <div class="page">
    <!-- 与铃的日记 -->
    <AppCard title="与铃的日记">
      <template v-if="milestone.days > 0">
        <p class="days">
          <span class="days-num">{{ milestone.days }}</span>
          <span class="days-unit">天</span>
          <span class="days-sub">与铃相遇{{ milestone.firstDate ? '于 ' + milestone.firstDate : '' }}</span>
        </p>
        <div class="list">
          <div v-for="(g, gi) in diaryGroups" :key="g.end" class="diary">
            <button type="button" class="diary-row" :aria-expanded="diaryOpen === gi" @click="toggleDiary(gi)">
              <span class="accent-text">{{ g.end }}</span>
              <span class="grow ellipsis">{{ g.summary }}</span>
              <span class="muted">{{ diaryOpen === gi ? '收起' : '展开' }}</span>
            </button>
            <p v-if="diaryOpen === gi" class="diary-body">{{ g.body }}</p>
          </div>
          <p v-if="diaryGroups.length === 0" class="muted">第一天，故事从一句「你好」开始～</p>
        </div>
        <div v-if="milestone.items.length > 0" class="badges">
          <button type="button" class="badges-head" :aria-expanded="showBadges" @click="showBadges = !showBadges">
            <span class="grow">纪念章（{{ milestone.items.length }}）</span>
            <span class="muted">{{ showBadges ? '收起' : '展开' }}</span>
          </button>
          <div v-if="showBadges" class="list">
            <div v-for="m in milestone.items" :key="m.key" class="badge">
              <AppStatusDot state="ok" />
              <span class="grow">{{ m.label }}</span>
              <span class="muted">{{ m.date }}</span>
            </div>
          </div>
        </div>
      </template>
      <p v-else class="muted">铃正在等你开启第一段对话…</p>
    </AppCard>

    <!-- 铃的能力 -->
    <AppCard title="铃的能力">
      <p class="cap-head">
        <span class="cap-mode">{{ abilityMatrix.modeLabel }}</span>
        <span class="cap-model">{{ abilityMatrix.modelName }}</span>
      </p>
      <div class="ability">
        <div v-for="r in abilityMatrix.rows" :key="r.name" class="ability-row">
          <span class="ability-name">{{ r.name }}</span>
          <AppStatusDot
            :state="r.ok ? 'ok' : r.unknown ? 'warn' : 'err'"
            :text="r.ok ? '支持' : r.unknown ? '未知' : '不支持'"
          />
          <span class="ability-detail">{{ r.detail }}</span>
        </div>
      </div>
    </AppCard>

    <!-- 管理员权限 -->
    <AppCard title="管理员权限">
      <div class="status-line">
        <span>当前状态</span>
        <AppStatusDot
          :state="adminState === null ? 'idle' : adminState ? 'ok' : 'warn'"
          :text="adminState === null ? '检测中…' : adminState ? '已以管理员权限运行' : '普通权限'"
        />
      </div>
      <p v-if="adminState === false" class="muted">
        电源模式切换 / 深度清理内存等特殊工具需要管理员权限。
      </p>
      <AppSwitch
        :model-value="setting.runAsAdmin"
        label="启动时始终以管理员运行（自动提权）"
        hint="开启后每次启动自动提权（弹一次 UAC 确认），适合常需要管理员工具的深度用户；无需可关闭。"
        @update:model-value="toggleRunAsAdmin"
      />
      <AppSwitch
        :model-value="setting.aiToolbox"
        label="允许 AI 调用工具箱工具"
        hint="开启后，铃可直接执行工具箱工具（如清理内存、ping、截图、查 IP 等），默认关闭。"
        @update:model-value="toggleAiToolbox"
      />
      <div v-if="adminState === false" class="row">
        <AppButton variant="primary" @click="requestAdmin">以管理员权限重启</AppButton>
      </div>
      <p v-if="adminMsg" class="note">{{ adminMsg }}</p>
    </AppCard>

    <!-- Agent 权限 -->
    <AppCard title="Agent 权限" hint="会改动系统或文件，请确认信任后再开">
      <p class="muted">
        长期目标的自动推进默认关闭，在「目标」面板逐个开启。仅在应用进程运行时按间隔推进，退出应用即停止。每次约消耗 1 次
        API 调用（工具、规划与自检可能增加调用），按当前配置模型计费；推进次数和超时上限始终生效。
      </p>
      <AppSwitch
        :model-value="setting.selfCheckEnabled"
        label="任务完成后自检"
        hint="铃动手做完事后，额外让模型核对一遍「是不是真的做成了」；没做成会自动再补一轮。只对动手的任务生效，纯聊天不额外花钱。"
        @update:model-value="(v) => toggleAgentFlag('selfCheckEnabled', v)"
      />
      <AppSwitch
        :model-value="setting.agentPlanConfirm"
        label="执行前先给我看计划"
        hint="多步任务拆完后先弹一张计划卡：步骤可以改、可以删、可以加，确认后铃再动手；不操作 120 秒后自动按原计划开始。只对 2 步以上的任务生效。"
        @update:model-value="(v) => toggleAgentFlag('agentPlanConfirm', v)"
      />
      <AppSwitch
        :model-value="setting.agentAllowDownload"
        label="允许下载文件"
        hint="开启后铃可以下载文件到本地"
        @update:model-value="(v) => toggleAgentFlag('agentAllowDownload', v)"
      />
      <AppSwitch
        :model-value="setting.agentAllowSoftware"
        label="允许安装/卸载软件"
        hint="开启后铃可以用 winget 安装或卸载软件"
        @update:model-value="(v) => toggleAgentFlag('agentAllowSoftware', v)"
      />
      <AppSwitch
        :model-value="setting.agentAllowFileWrite"
        label="允许写入/删除文件"
        hint="开启后铃可以生成文件、创建文件夹、删除文件"
        @update:model-value="(v) => toggleAgentFlag('agentAllowFileWrite', v)"
      />
      <AppSwitch
        :model-value="setting.agentAllowShell"
        label="允许执行命令"
        hint="开启后铃可以执行系统命令（能力最强，也最危险，请谨慎开启）"
        @update:model-value="(v) => toggleAgentFlag('agentAllowShell', v)"
      />
      <AppSwitch
        :model-value="setting.agentAllowToolForge"
        label="允许 Agent 自己编写小工具"
        hint="默认关闭。开启后 Agent 可以自己写小工具（Python / PowerShell / Node）来扩展能力，写完先试跑，源码随时可在「工具箱 → 自制工具」里查看、禁用、删除。这是关键字过滤 + 独立临时目录 + 超时限制，不是操作系统级隔离，请只在你信任的脚本上开启。"
        @update:model-value="(v) => toggleAgentFlag('agentAllowToolForge', v)"
      />
    </AppCard>

    <!-- 长期记忆 -->
    <AppCard title="长期记忆">
      <AppField
        label="每次注入条数"
        hint="铃会按「跟你这句话的相关性」，从过往记忆里挑出最相关的几条一起送进对话。条数越多越不容易忘事，但也更费 token；设为 0 则完全不注入长期记忆（当前会话的聊天记录不受影响，照旧连续）。"
      >
        <input
          v-model.number="setting.longTermMemoryLimit"
          class="range"
          type="range"
          min="0"
          max="20"
          step="1"
          @change="saveLongTermLimit"
        />
        <p class="status-line">
          当前：{{ setting.longTermMemoryLimit }} 条{{
            setting.longTermMemoryLimit === 0 ? '（已关闭长期记忆）' : ''
          }}
        </p>
      </AppField>
    </AppCard>

    <!-- 开机自启动 -->
    <AppCard title="开机自启动">
      <AppSwitch
        :model-value="autostart"
        :label="autostart ? '已开启' : '已关闭'"
        @update:model-value="onAutostart"
      />
    </AppCard>

    <!-- 全局快捷键 -->
    <AppCard title="全局快捷键">
      <AppField label="唤起快捷键" hint="格式如 Ctrl+Alt+L，点「应用」后立即生效并保存。">
        <div class="row">
          <AppInput v-model="hotkey" placeholder="Ctrl+Alt+L" />
          <AppButton variant="primary" @click="saveHotkey">应用</AppButton>
        </div>
      </AppField>
    </AppCard>

    <!-- 数据路径 -->
    <AppCard title="数据路径">
      <AppField :hint="`当前：${setting.dataPath || '（未设置，默认文档/铃记忆体）'}`">
        <div class="row">
          <AppInput v-model="setting.dataPath" placeholder="记忆存储路径" />
          <AppButton variant="primary" @click="saveDataPath">保存</AppButton>
        </div>
      </AppField>
    </AppCard>

    <!-- 配置备份 -->
    <AppCard title="配置备份">
      <div class="row">
        <AppButton @click="exportConfigFile">导出配置 JSON</AppButton>
      </div>
      <AppField label="导入配置" hint="填写配置 JSON 文件的完整路径，导入后立即生效。">
        <div class="row">
          <AppInput v-model="importPath" placeholder="C:\Users\...\铃记忆体配置.json" />
          <AppButton variant="primary" @click="onImport">导入</AppButton>
        </div>
      </AppField>
    </AppCard>

    <!-- 重置 -->
    <AppCard title="重置">
      <div class="row">
        <AppButton @click="doReset">恢复默认设置</AppButton>
      </div>
      <p class="muted">只恢复设置项，主密码与已加密的密钥不会被清掉。</p>
    </AppCard>

    <!-- 工具箱入口 -->
    <AppCard title="工具箱">
      <div class="row">
        <AppButton variant="primary" @click="toolbox && toolbox.toggle()">
          {{ toolbox && toolbox.open.value ? '收起工具箱' : '打开工具箱' }}
        </AppButton>
      </div>
    </AppCard>
  </div>
</template>

<style scoped>
.page {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-3);
  padding-bottom: var(--ui-sp-4);
}
.row {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  min-width: 0;
}
.grow {
  flex: 1;
  min-width: 0;
}
.ellipsis {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.muted {
  margin: 0;
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text-3);
}
.note {
  margin: var(--ui-sp-2) 0 0;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-left: 2px solid var(--ui-accent-line);
  border-radius: var(--ui-r-sm);
  background: var(--ui-hover);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text-2);
  overflow-wrap: anywhere;
}
.status-line {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  margin: 0;
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text-2);
}
.accent-text {
  color: var(--ui-accent);
  font-size: var(--ui-fs-12);
  font-weight: 600;
}
.range {
  width: 100%;
  accent-color: var(--ui-accent);
}
/* —— 日记 —— */
.days {
  display: flex;
  align-items: baseline;
  gap: var(--ui-sp-2);
  margin: 0 0 var(--ui-sp-3);
}
.days-num {
  font-size: var(--ui-fs-20);
  font-weight: 700;
  line-height: 1;
  color: var(--ui-accent);
}
.days-unit {
  font-size: var(--ui-fs-13);
  font-weight: 600;
  color: var(--ui-text);
}
.days-sub {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
}
.list {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
}
.diary-row,
.badges-head {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  width: 100%;
  padding: var(--ui-sp-2);
  border: 1px solid transparent;
  border-radius: var(--ui-r-sm);
  background: var(--ui-hover);
  color: var(--ui-text);
  font-size: var(--ui-fs-12);
  text-align: left;
  transition: background var(--ui-dur-1) var(--ui-ease), border-color var(--ui-dur-1) var(--ui-ease);
}
.diary-row:hover,
.badges-head:hover {
  background: var(--ui-active);
  border-color: var(--ui-border);
}
.diary-body {
  margin: 0;
  padding: var(--ui-sp-2);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text-2);
}
.badges {
  margin-top: var(--ui-sp-3);
  padding-top: var(--ui-sp-2);
  border-top: 1px solid var(--ui-border);
}
.badge {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-1) var(--ui-sp-2);
  font-size: var(--ui-fs-12);
  color: var(--ui-text);
}
/* —— 能力矩阵 —— */
.cap-head {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  margin: 0 0 var(--ui-sp-2);
}
.cap-mode {
  padding: 0 var(--ui-sp-2);
  border-radius: var(--ui-r-pill);
  background: var(--ui-accent-soft);
  color: var(--ui-accent);
  font-size: var(--ui-fs-11);
  font-weight: 600;
  line-height: 20px;
}
.cap-model {
  min-width: 0;
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  overflow-wrap: anywhere;
}
.ability {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.ability-row {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-1) var(--ui-sp-2);
  border-radius: var(--ui-r-sm);
  font-size: var(--ui-fs-12);
}
.ability-row:hover {
  background: var(--ui-hover);
}
.ability-name {
  flex: none;
  width: 68px;
  color: var(--ui-text);
}
.ability-detail {
  flex: 1;
  min-width: 0;
  color: var(--ui-text-3);
  overflow-wrap: anywhere;
}
</style>
