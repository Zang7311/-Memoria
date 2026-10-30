<!-- 《铃·记忆体》主布局 v0.9：左侧图标栏 + 中栏对话 + 右侧抽屉
     设计约束见 docs/UI重做方案.md：颜色/字号/间距一律走 src/styles/tokens.css 的 --ui-*，
     这里不写字面色值和魔法数字；面板本体的重做属于「各面板」批次，这里只提供骨架和外框。 -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import ChatList from '../components/ChatList.vue'
import ChatInput from '../components/ChatInput.vue'
import PlanCard from '../components/PlanCard.vue'
import MemoryPanel from '../components/MemoryPanel.vue'
import PluginManager from '../components/PluginManager.vue'
import ToolboxPanel from '../components/ToolboxPanel.vue'
import GoalsPanel from '../components/GoalsPanel.vue'
import AuditPanel from '../components/AuditPanel.vue'
import AppStatusDot from '../components/ui/AppStatusDot.vue'
import TheIcon from '../components/TheIcon.vue'
import SettingView from './SettingView.vue'
import { useGoalStore } from '../stores/goalStore'
import { useSettingStore } from '../stores/settingStore'
import { useDesktopStore } from '../stores/desktopStore'
import { useSyncStore } from '../stores/syncStore'
import { useChatStore } from '../stores/chatStore'
import { assetUrl, isImagePath } from '../utils/tauri'
import { broadcastDrop, installWindowFileDrop } from '../utils/fileDrop'
import { listen } from '@tauri-apps/api/event'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

const setting = useSettingStore()
const desktop = useDesktopStore()
const sync = useSyncStore()
const chat = useChatStore()
const goals = useGoalStore()

/** 右侧抽屉当前页；空字符串 = 收起（一次只开一页，点同一个图标收起）
    工具箱/目标/执行记录暂时还是浮层：它们自带 position:fixed，塞进抽屉会被撑满整窗，
    等「各面板」批次统一改成本体内嵌后再收进来 */
const dock = ref<'' | 'memory' | 'plugins'>('')
const showGoals = ref(false)
const showAudit = ref(false)
const showToolbox = ref(false)
const showSettings = ref(false)

function openDock(view: 'memory' | 'plugins') {
  dock.value = dock.value === view ? '' : view
}

const theme = computed(() => setting.theme || 'dark')
// 铃的头像：图片路径则显示图片（asset 协议加载），否则显示文字
const avatarImg = computed(() => (isImagePath(setting.avatarSuzu) ? assetUrl(setting.avatarSuzu!) : null))

// —— 铃的状态：只保留一处，避免和网络状态重复显示「在线」——
const suzuState = computed<'ok' | 'warn'>(() => (chat.isLoading ? 'warn' : 'ok'))
const suzuText = computed(() => (chat.isLoading ? '思考中…' : '在线'))

// —— 网络/同步只在异常时出现在顶栏，正常时不占地方 ——
const netState = computed<'ok' | 'err' | 'idle'>(() =>
  sync.networkStatus === 'online' ? 'ok' : sync.networkStatus === 'offline' ? 'err' : 'idle',
)
const syncState = computed<'ok' | 'warn' | 'err' | 'idle'>(() => {
  if (sync.syncStatus === 'syncing') return 'warn'
  if (sync.syncStatus === 'done') return 'ok'
  if (sync.syncStatus === 'error') return 'err'
  return 'idle'
})
const syncText = computed(() => {
  if (sync.syncStatus === 'syncing') return `同步 ${Math.round(sync.syncProgress * 100)}%`
  if (sync.syncStatus === 'done') return '已同步'
  if (sync.syncStatus === 'error') return '同步失败'
  return ''
})

/** 把路由信息翻成人话，显示在输入框下方（让用户知道这次是怎么选的模型） */
function routeLabel(r: { source: string; easy: boolean; needs_vision: boolean; model: string }): string {
  const src =
    r.source === 'ai'
      ? 'AI 判断'
      : r.source === 'local'
        ? '本地判断'
        : '本地规则'
  const kind = r.needs_vision ? '需要看图' : r.easy ? '闲聊' : '要干活'
  return `${src} · ${kind} · ${r.model}`
}

/** 桌面文件拖到窗口上时的提示层 */
const dropActive = ref(false)
let uninstallDrop: (() => void) | null = null

watch(
  () => chat.activeSessionId,
  (id) => {
    if (id) goals.remind()
  },
)
onUnmounted(() => {
  goals.dispose()
  uninstallDrop?.()
})

onMounted(() => {
  goals.init().catch((reason) => {
    goals.error = String(reason)
  })
  desktop.loadMonitorRules().catch(() => {})
  // 悬浮球状态与后端同步，避免按钮状态和窗口真实可见性对不上
  WebviewWindow.getByLabel('floating-ball')
    .then(async (ball) => {
      if (ball) desktop.floatingBallVisible = await ball.isVisible().catch(() => false)
    })
    .catch(() => {})
  sync.init().catch(() => {})
  chat.init().catch(() => {})
  // 悬浮球面板「设置…」→ 打开设置页
  listen('floating-open-settings', () => {
    showSettings.value = true
  }).catch(() => {})
  // 桌面拖文件进窗口 → 高亮提示 + 交给输入框当附件
  installWindowFileDrop(
    (active) => {
      dropActive.value = active
    },
    (result) => broadcastDrop(result),
  )
    .then((un) => {
      uninstallDrop = un
    })
    .catch(() => {})
})

async function onDeleteSession(id: string) {
  if (confirm('删除该会话？此操作不可恢复。')) {
    await chat.deleteSession(id).catch(() => {})
  }
}

async function toggleFloatingBall() {
  await desktop.setFloatingBallVisibility(!desktop.floatingBallVisible)
}

function openSettings() {
  showSettings.value = true
}

async function onPersonaChange(e: Event) {
  const v = (e.target as HTMLSelectElement).value
  await setting.update({ persona: v }).catch(() => {})
}
</script>

<template>
  <div class="layout" :class="theme">
    <!-- ==================== 左侧图标栏 ==================== -->
    <nav class="rail" aria-label="主导航">
      <button
        class="rbtn"
        :class="{ on: dock === 'memory' }"
        title="记忆"
        @click="openDock('memory')"
      >
        <TheIcon name="memory" :size="18" />
      </button>
      <button
        class="rbtn"
        :class="{ on: showGoals }"
        title="目标"
        @click="showGoals = !showGoals"
      >
        <TheIcon name="command" :size="18" />
        <span v-if="goals.attentionCount" class="rnum">{{ goals.attentionCount }}</span>
      </button>
      <button
        class="rbtn"
        :class="{ on: showToolbox }"
        title="工具箱"
        @click="showToolbox = !showToolbox"
      >
        <TheIcon name="toolbox" :size="18" />
      </button>
      <button
        class="rbtn"
        :class="{ on: dock === 'plugins' }"
        title="插件"
        @click="openDock('plugins')"
      >
        <TheIcon name="plugin" :size="18" />
      </button>
      <button
        class="rbtn"
        :class="{ on: showAudit }"
        title="执行记录"
        @click="showAudit = !showAudit"
      >
        <TheIcon name="diagnostic" :size="18" />
      </button>

      <span class="rail-spacer" />

      <button
        class="rbtn"
        :class="{ on: desktop.floatingBallVisible }"
        title="悬浮球"
        @click="toggleFloatingBall"
      >
        <TheIcon name="floating" :size="18" />
      </button>
      <button class="rbtn" :class="{ on: showSettings }" title="设置" @click="openSettings">
        <TheIcon name="settings" :size="18" />
      </button>
    </nav>

    <!-- ==================== 中栏：对话 ==================== -->
    <main class="center">
      <header class="head">
        <div class="who">
          <span class="avatar">
            <img v-if="avatarImg" :src="avatarImg" alt="" class="avatar-img" />
            <template v-else>{{ setting.avatarSuzu || '铃' }}</template>
          </span>
          <span class="who-text">
            <span class="name">铃</span>
            <AppStatusDot :state="suzuState" :text="suzuText" />
          </span>
        </div>
        <div class="head-right">
          <button
            v-if="sync.networkStatus === 'offline'"
            class="status-chip"
            title="网络：离线（已切离线模式），点击去设置"
            @click="openSettings"
          >
            <AppStatusDot :state="netState" text="离线" />
          </button>
          <button
            v-if="syncText"
            class="status-chip"
            :title="'同步：' + syncText"
            @click="openSettings"
          >
            <AppStatusDot :state="syncState" :text="syncText" />
          </button>
          <select
            class="persona"
            :value="setting.persona"
            title="切换人格（API 模式生效）"
            @change="onPersonaChange"
          >
            <option value="daily">日常</option>
            <option value="chuunibyou">中二</option>
            <option value="healing">治愈</option>
            <option value="lewd">涩涩</option>
          </select>
        </div>
      </header>

      <!-- 会话标签栏：横向滚动，选中用主色描边而不是整块粉底 -->
      <div class="sessions">
        <button
          v-for="s in chat.sessions"
          :key="s.id"
          class="stab"
          :class="{ on: s.id === chat.activeSessionId }"
          :title="s.title"
          @click="chat.switchSession(s.id)"
        >
          <span class="stab-title">{{ s.title }}</span>
          <span class="stab-del" title="删除会话" @click.stop="onDeleteSession(s.id)">
            <TheIcon name="close" :size="10" />
          </span>
        </button>
        <button class="stab-new" title="新建会话" @click="chat.createSession()">
          <TheIcon name="add" :size="14" />
        </button>
      </div>

      <!-- 对话流 -->
      <div class="thread">
        <p v-if="goals.reminder" class="sys-note" role="status">{{ goals.reminder }}</p>
        <p v-if="goals.runningId && goals.automatic" class="sys-note" role="status">
          正在自动推进：{{ goals.runningTitle }}
        </p>
        <ChatList />
        <PlanCard />
      </div>

      <!-- 输入区 + 用量提示 -->
      <div class="composer">
        <ChatInput />
        <div class="usage">
          <span v-if="chat.lastRoute" class="usage-route">路由：{{ routeLabel(chat.lastRoute) }}</span>
          <span v-if="chat.lastUsage">
            输入 {{ chat.lastUsage.prompt_tokens }} · 输出 {{ chat.lastUsage.completion_tokens }} · 合计
            {{ chat.lastUsage.total_tokens }} tokens
          </span>
          <span v-else>等待回复后显示 token 用量</span>
        </div>
      </div>
    </main>

    <!-- ==================== 右侧抽屉 ==================== -->
    <aside v-if="dock" class="dock">
      <MemoryPanel v-if="dock === 'memory'" @close="dock = ''" />
      <PluginManager v-else-if="dock === 'plugins'" @close="dock = ''" />
    </aside>

    <!-- 工具箱 / 目标 / 执行记录：自带 position:fixed，暂时仍走浮层 -->
    <ToolboxPanel v-if="showToolbox" @close="showToolbox = false" />
    <GoalsPanel v-if="showGoals" @close="showGoals = false" />
    <AuditPanel v-if="showAudit" @close="showAudit = false" />

    <!-- 桌面文件拖进窗口时的提示层 -->
    <div v-if="dropActive" class="drop-layer">
      <div class="drop-card">
        <span class="drop-title">松开就加进附件</span>
        <span class="drop-hint">图片 5MB 内 / 文本 1MB 内，一次最多 5 个</span>
      </div>
    </div>

    <!-- 设置页 -->
    <div v-if="showSettings" class="settings-overlay" @click.self="showSettings = false">
      <div class="settings-panel">
        <button class="settings-close" title="关闭" @click="showSettings = false">
          <TheIcon name="close" :size="14" />
        </button>
        <SettingView />
      </div>
    </div>
  </div>
</template>

<style scoped>
.layout {
  height: 100vh;
  display: flex;
  overflow: hidden;
  background: var(--ui-bg);
  color: var(--ui-text);
}

/* ---------- 左侧图标栏 ---------- */
.rail {
  width: var(--ui-rail-w);
  flex: none;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--ui-sp-1);
  padding: var(--ui-sp-3) 0;
  background: var(--ui-panel);
  border-right: 1px solid var(--ui-border);
}
.rbtn {
  position: relative;
  width: var(--ui-rail-btn);
  height: var(--ui-rail-btn);
  display: grid;
  place-items: center;
  border: 0;
  border-radius: var(--ui-r-md);
  background: transparent;
  color: var(--ui-text-2);
  transition: background var(--ui-dur-1) var(--ui-ease), color var(--ui-dur-1) var(--ui-ease);
}
.rbtn:hover {
  background: var(--ui-hover);
  color: var(--ui-text);
}
.rbtn.on {
  background: var(--ui-active);
  color: var(--ui-text);
  box-shadow: inset 0 0 0 1px var(--ui-border-strong);
}
.rnum {
  position: absolute;
  top: 2px;
  right: 2px;
  min-width: 14px;
  height: 14px;
  padding: 0 3px;
  border-radius: var(--ui-r-pill);
  background: var(--ui-accent);
  color: #fff;
  font-size: var(--ui-fs-11);
  line-height: 14px;
  text-align: center;
}
.rail-spacer {
  flex: 1;
}

/* ---------- 中栏 ---------- */
.center {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  background: var(--ui-bg);
}
.head {
  height: var(--ui-head-h);
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--ui-sp-3);
  padding: 0 var(--ui-sp-4);
  border-bottom: 1px solid var(--ui-border);
}
.who {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  min-width: 0;
}
.avatar {
  width: 28px;
  height: 28px;
  border-radius: var(--ui-r-pill);
  display: grid;
  place-items: center;
  overflow: hidden;
  flex: none;
  background: var(--ui-accent-soft);
  color: var(--ui-accent);
  font-size: var(--ui-fs-12);
  font-weight: 600;
}
.avatar-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.who-text {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  min-width: 0;
}
.name {
  font-size: var(--ui-fs-14);
  font-weight: 600;
  color: var(--ui-text);
}
.head-right {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: var(--ui-sp-1);
  flex: none;
}
.status-chip {
  display: inline-flex;
  align-items: center;
  height: 24px;
  padding: 0 var(--ui-sp-2);
  border: 1px solid transparent;
  border-radius: var(--ui-r-pill);
  background: transparent;
}
.status-chip:hover {
  background: var(--ui-hover);
  border-color: var(--ui-border);
}
.persona {
  height: 24px;
  padding: 0 var(--ui-sp-2);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-pill);
  background: var(--ui-raised);
  color: var(--ui-text-2);
  font-size: var(--ui-fs-12);
  cursor: pointer;
}
.persona:hover {
  color: var(--ui-text);
  border-color: var(--ui-border-strong);
}

/* 会话标签：选中只加描边和文字色，不整块粉底 */
.sessions {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-1);
  padding: var(--ui-sp-2) var(--ui-sp-4);
  border-bottom: 1px solid var(--ui-border);
  overflow-x: auto;
  flex: none;
  scrollbar-width: none;
}
.sessions::-webkit-scrollbar {
  display: none;
}
.stab {
  display: inline-flex;
  align-items: center;
  gap: var(--ui-sp-1);
  max-width: 180px;
  height: 26px;
  padding: 0 var(--ui-sp-2);
  border: 1px solid transparent;
  border-radius: var(--ui-r-pill);
  background: transparent;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-12);
  white-space: nowrap;
}
.stab:hover {
  background: var(--ui-hover);
  color: var(--ui-text);
}
.stab.on {
  background: var(--ui-raised);
  border-color: var(--ui-border-strong);
  color: var(--ui-text);
}
.stab-title {
  overflow: hidden;
  text-overflow: ellipsis;
}
.stab-del {
  display: inline-grid;
  place-items: center;
  width: 16px;
  height: 16px;
  border-radius: var(--ui-r-sm);
  color: var(--ui-text-3);
  opacity: 0;
  transition: opacity var(--ui-dur-1) var(--ui-ease);
}
.stab:hover .stab-del,
.stab.on .stab-del {
  opacity: 1;
}
.stab-del:hover {
  background: var(--ui-active);
  color: var(--ui-text);
}
.stab-new {
  display: inline-grid;
  place-items: center;
  width: 26px;
  height: 26px;
  border: 1px dashed var(--ui-border-strong);
  border-radius: var(--ui-r-pill);
  background: transparent;
  color: var(--ui-text-3);
  flex: none;
}
.stab-new:hover {
  color: var(--ui-accent);
  border-color: var(--ui-accent-line);
}

/* 对话流 */
.thread {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--ui-sp-4) var(--ui-sp-4) 0;
}
.sys-note {
  margin: 0 auto var(--ui-sp-3);
  max-width: var(--ui-thread-max);
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-left: 2px solid var(--ui-accent);
  background: var(--ui-panel);
  color: var(--ui-text-2);
  font-size: var(--ui-fs-12);
  border-radius: var(--ui-r-sm);
}

/* 输入区 */
.composer {
  flex: none;
  padding: 0 var(--ui-sp-4) var(--ui-sp-2);
  background: var(--ui-bg);
}
.usage {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-3);
  justify-content: flex-end;
  padding: var(--ui-sp-1) 0 0;
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  user-select: none;
}
.usage-route {
  margin-right: auto;
}

/* ---------- 右侧抽屉 ---------- */
.dock {
  width: var(--ui-dock-w);
  flex: none;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--ui-panel);
  border-left: 1px solid var(--ui-border);
  box-shadow: var(--ui-shadow-drawer);
  animation: dock-in var(--ui-dur-2) var(--ui-ease);
}
@keyframes dock-in {
  from {
    transform: translateX(12px);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}
/* 过渡期：现有面板自带固定宽度和标题栏，先让它们撑满抽屉，等「各面板」批次统一重做
   注意：只对「非 position:fixed」的面板这样写，否则固定定位元素会被撑成整屏 */
.dock :deep(.memory-panel),
.dock :deep(.plugin-panel) {
  width: 100%;
  height: 100%;
  border: 0;
  background: transparent;
}

/* ---------- 拖放提示层 ---------- */
.drop-layer {
  position: fixed;
  inset: var(--ui-sp-3);
  z-index: 600;
  display: grid;
  place-items: center;
  pointer-events: none;
  border: 2px dashed var(--ui-accent-line);
  border-radius: var(--ui-r-lg);
  background: color-mix(in srgb, var(--ui-bg) 72%, transparent);
}
.drop-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--ui-sp-1);
  padding: var(--ui-sp-4) var(--ui-sp-5);
  border-radius: var(--ui-r-md);
  background: var(--ui-raised);
  border: 1px solid var(--ui-border-strong);
  box-shadow: var(--ui-shadow);
}
.drop-title {
  font-size: var(--ui-fs-14);
  color: var(--ui-text);
}
.drop-hint {
  font-size: var(--ui-fs-12);
  color: var(--ui-text-3);
}

/* ---------- 设置页 ---------- */
.settings-overlay {
  position: fixed;
  inset: 0;
  z-index: 550;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.45);
}
.settings-panel {
  position: relative;
  width: 860px;
  max-width: calc(100vw - 48px);
  height: 86vh;
  overflow: hidden;
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-lg);
  background: var(--ui-bg);
  box-shadow: var(--ui-shadow);
}
.settings-close {
  position: absolute;
  top: var(--ui-sp-2);
  right: var(--ui-sp-2);
  z-index: 10;
  width: 26px;
  height: 26px;
  display: grid;
  place-items: center;
  border: 0;
  border-radius: var(--ui-r-sm);
  background: var(--ui-hover);
  color: var(--ui-text-2);
}
.settings-close:hover {
  background: var(--ui-active);
  color: var(--ui-text);
}
</style>
