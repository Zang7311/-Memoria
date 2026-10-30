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
import AppToast from '../components/ui/AppToast.vue'
import AppDialog from '../components/ui/AppDialog.vue'
import { confirmDialog } from '../utils/dialog'
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

/** 抽屉里的面板做成「标签页」：可以同时开多个，一次只显示一个。
    交互参考三处：Windows 11 标签条（可关、可滚）、Edge 手机版标签总览、
    iOS 清后台的卡片层（点卡片切换、卡片上 × 关闭）。 */
type PanelKey = 'memory' | 'goals' | 'toolbox' | 'plugins' | 'audit'

const PANELS: Record<PanelKey, { title: string; icon: string; width: number; desc: string }> = {
  memory: { title: '记忆', icon: 'memory', width: 360, desc: '长期记忆、分类与搜索' },
  goals: { title: '目标', icon: 'command', width: 460, desc: '铃自己推进的长期目标' },
  toolbox: { title: '工具箱', icon: 'toolbox', width: 380, desc: '常用小工具与自定义指令' },
  plugins: { title: '插件', icon: 'plugin', width: 400, desc: '插件的安装与管理' },
  audit: { title: '执行记录', icon: 'diagnostic', width: 560, desc: '铃动过手的地方与结果' },
}

/** 已打开的标签（顺序＝标签条顺序） */
const openTabs = ref<PanelKey[]>([])
/** 当前显示哪一页；null = 抽屉收起 */
const activeTab = ref<PanelKey | null>(null)
/** 卡片总览（Edge 手机版 / iOS 清后台那一层） */
const showOverview = ref(false)
/** 标签条上的「+」菜单 */
const showAddMenu = ref(false)
const showSettings = ref(false)

const activePanel = computed(() => (activeTab.value ? PANELS[activeTab.value] : null))
const availablePanels = computed(() => (Object.keys(PANELS) as PanelKey[]).filter((k) => !openTabs.value.includes(k)))

/** 左栏图标：没开就打开并激活，已激活再点一次就把抽屉收起来（标签不丢） */
function openPanel(key: PanelKey) {
  if (!openTabs.value.includes(key)) openTabs.value.push(key)
  activeTab.value = activeTab.value === key ? null : key
  showAddMenu.value = false
  showOverview.value = false
}

function activateTab(key: PanelKey) {
  activeTab.value = key
  showOverview.value = false
}

/** 关掉一个标签：关的是当前页就退到相邻的一个，没有相邻就收起抽屉 */
function closeTab(key: PanelKey) {
  const index = openTabs.value.indexOf(key)
  openTabs.value = openTabs.value.filter((k) => k !== key)
  if (activeTab.value === key) {
    activeTab.value = openTabs.value[index] ?? openTabs.value[index - 1] ?? null
  }
}

function toggleOverview() {
  if (!openTabs.value.length) return
  showOverview.value = !showOverview.value
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
  const ok = await confirmDialog('删除该会话？此操作不可恢复。', { danger: true, confirmText: '删除' })
  if (ok) await chat.deleteSession(id).catch(() => {})
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
        :class="{ on: activeTab === 'memory' }"
        title="记忆"
        @click="openPanel('memory')"
      >
        <TheIcon name="memory" :size="18" />
      </button>
      <button
        class="rbtn"
        :class="{ on: activeTab === 'goals' }"
        title="目标"
        @click="openPanel('goals')"
      >
        <TheIcon name="command" :size="18" />
        <span v-if="goals.attentionCount" class="rnum">{{ goals.attentionCount }}</span>
      </button>
      <button
        class="rbtn"
        :class="{ on: activeTab === 'toolbox' }"
        title="工具箱"
        @click="openPanel('toolbox')"
      >
        <TheIcon name="toolbox" :size="18" />
      </button>
      <button
        class="rbtn"
        :class="{ on: activeTab === 'plugins' }"
        title="插件"
        @click="openPanel('plugins')"
      >
        <TheIcon name="plugin" :size="18" />
      </button>
      <button
        class="rbtn"
        :class="{ on: activeTab === 'audit' }"
        title="执行记录"
        @click="openPanel('audit')"
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

    <!-- ==================== 右侧抽屉（标签页） ==================== -->
    <aside v-if="activePanel" class="dock" :style="{ width: activePanel.width + 'px' }">
      <!-- Windows 11 风格标签条：可横向滚动、可关闭，右侧「+」和「总览」 -->
      <div class="tabs-bar">
        <div class="tabs-scroll">
          <button
            v-for="key in openTabs"
            :key="key"
            class="tab"
            :class="{ on: key === activeTab }"
            :title="PANELS[key].title"
            @click="activateTab(key)"
          >
            <span class="tab-title">{{ PANELS[key].title }}</span>
            <span class="tab-close" title="关闭" @click.stop="closeTab(key)">
              <TheIcon name="close" :size="10" />
            </span>
          </button>
        </div>
        <div class="tabs-actions">
          <button class="tab-act" title="打开面板" @click.stop="showAddMenu = !showAddMenu">＋</button>
          <button class="tab-act" title="所有面板" @click="toggleOverview">总览</button>
        </div>

        <!-- 「+」菜单：只列还没打开的面板 -->
        <div v-if="showAddMenu" class="add-menu">
          <button
            v-for="key in availablePanels"
            :key="key"
            class="add-item"
            @click="openPanel(key)"
          >
            <TheIcon :name="PANELS[key].icon" :size="14" />
            <span>{{ PANELS[key].title }}</span>
          </button>
          <p v-if="!availablePanels.length" class="add-empty">都开着了</p>
        </div>
      </div>

      <!-- 面板本体：v-show 切页，切换不丢各自的滚动位置和状态 -->
      <div class="dock-body">
        <div v-if="openTabs.includes('memory')" v-show="activeTab === 'memory'" class="pane">
          <MemoryPanel @close="closeTab('memory')" />
        </div>
        <div v-if="openTabs.includes('goals')" v-show="activeTab === 'goals'" class="pane">
          <GoalsPanel @close="closeTab('goals')" />
        </div>
        <div v-if="openTabs.includes('toolbox')" v-show="activeTab === 'toolbox'" class="pane">
          <ToolboxPanel @close="closeTab('toolbox')" />
        </div>
        <div v-if="openTabs.includes('plugins')" v-show="activeTab === 'plugins'" class="pane">
          <PluginManager @close="closeTab('plugins')" />
        </div>
        <div v-if="openTabs.includes('audit')" v-show="activeTab === 'audit'" class="pane">
          <AuditPanel @close="closeTab('audit')" />
        </div>
      </div>
    </aside>

    <!-- ==================== 卡片总览（Edge 手机版标签页 / iOS 清后台） ==================== -->
    <Transition name="fade">
      <div v-if="showOverview" class="overview" @click.self="showOverview = false">
        <div class="ov-head">
          <span class="ov-title">已打开 {{ openTabs.length }} 个面板</span>
          <button class="ov-done" @click="showOverview = false">完成</button>
        </div>
        <TransitionGroup name="card" tag="div" class="ov-grid">
          <div
            v-for="key in openTabs"
            :key="key"
            class="ov-card"
            :class="{ on: key === activeTab }"
            @click="activateTab(key)"
          >
            <div class="ov-card-head">
              <TheIcon :name="PANELS[key].icon" :size="16" />
              <span class="ov-card-title">{{ PANELS[key].title }}</span>
              <button class="ov-card-x" title="关闭" @click.stop="closeTab(key)">
                <TheIcon name="close" :size="10" />
              </button>
            </div>
            <div class="ov-card-body">
              <span class="ov-card-desc">{{ PANELS[key].desc }}</span>
            </div>
          </div>
        </TransitionGroup>
      </div>
    </Transition>

    <!-- 桌面文件拖进窗口时：只画一圈虚线框，不弹说明 -->
    <div v-if="dropActive" class="drop-layer" />

    <!-- 设置页 -->
    <div v-if="showSettings" class="settings-overlay" @click.self="showSettings = false">
      <div class="settings-panel">
        <button class="settings-close" title="关闭" @click="showSettings = false">
          <TheIcon name="close" :size="14" />
        </button>
        <SettingView />
      </div>
    </div>

    <!-- 全局提示与确认框（替代 alert/confirm） -->
    <AppToast />
    <AppDialog />
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

/* ---------- 右侧抽屉（标签页容器） ---------- */
.dock {
  flex: none;
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  background: var(--ui-panel);
  border-left: 1px solid var(--ui-border);
  box-shadow: var(--ui-shadow-drawer);
  animation: dock-in var(--ui-dur-2) var(--ui-ease);
  transition: width var(--ui-dur-2) var(--ui-ease);
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

/* 标签条：Windows 11 那种紧凑页签，激活项有底色和底部主色线 */
.tabs-bar {
  position: relative;
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--ui-sp-1);
  height: 40px;
  padding: 0 var(--ui-sp-2);
  border-bottom: 1px solid var(--ui-border);
}
.tabs-scroll {
  display: flex;
  align-items: center;
  gap: 2px;
  min-width: 0;
  overflow-x: auto;
  scrollbar-width: none;
}
.tabs-scroll::-webkit-scrollbar {
  display: none;
}
.tab {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: var(--ui-sp-1);
  height: 28px;
  padding: 0 var(--ui-sp-2);
  border: 0;
  border-radius: var(--ui-r-sm);
  background: transparent;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-12);
  white-space: nowrap;
  transition: background var(--ui-dur-1) var(--ui-ease), color var(--ui-dur-1) var(--ui-ease);
}
.tab:hover {
  background: var(--ui-hover);
  color: var(--ui-text);
}
.tab.on {
  background: var(--ui-raised);
  color: var(--ui-text);
  box-shadow: inset 0 -2px 0 var(--ui-accent);
}
.tab-title {
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.tab-close {
  display: inline-grid;
  place-items: center;
  width: 16px;
  height: 16px;
  border-radius: var(--ui-r-sm);
  color: var(--ui-text-3);
  opacity: 0;
  transition: opacity var(--ui-dur-1) var(--ui-ease);
}
.tab:hover .tab-close,
.tab.on .tab-close {
  opacity: 1;
}
.tab-close:hover {
  background: var(--ui-active);
  color: var(--ui-text);
}
.tabs-actions {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 2px;
  flex: none;
}
.tab-act {
  height: 26px;
  padding: 0 var(--ui-sp-2);
  border: 0;
  border-radius: var(--ui-r-sm);
  background: transparent;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-12);
}
.tab-act:hover {
  background: var(--ui-hover);
  color: var(--ui-text);
}

/* 「+」菜单 */
.add-menu {
  position: absolute;
  top: 38px;
  right: var(--ui-sp-2);
  z-index: 20;
  min-width: 148px;
  padding: var(--ui-sp-1);
  background: var(--ui-raised);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-md);
  box-shadow: var(--ui-shadow);
  animation: menu-in var(--ui-dur-1) var(--ui-ease);
}
@keyframes menu-in {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
.add-item {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  width: 100%;
  padding: var(--ui-sp-2);
  border: 0;
  border-radius: var(--ui-r-sm);
  background: transparent;
  color: var(--ui-text);
  font-size: var(--ui-fs-13);
  text-align: left;
}
.add-item:hover {
  background: var(--ui-hover);
}
.add-empty {
  margin: 0;
  padding: var(--ui-sp-2);
  font-size: var(--ui-fs-12);
  color: var(--ui-text-3);
}

/* 面板区 */
.dock-body {
  flex: 1;
  min-height: 0;
  position: relative;
  overflow: hidden;
}
.pane {
  height: 100%;
  overflow: hidden;
}
/* 过渡期：标签条已经给了标题和关闭按钮，面板自带的 .panel-header 先隐藏；
   目标/执行记录的头部里还有「新建/刷新」，保留不动 */
.dock :deep(.panel-header) {
  display: none;
}

/* 过渡期：五个面板都还自带 position:fixed / 固定宽度 / 自己的一层外框，
   先统一压成「本体内嵌」，等「各面板」批次重做时再删掉这段 */
.dock :deep(.memory-panel),
.dock :deep(.plugin-panel),
.dock :deep(.toolbox-panel),
.dock :deep(.goals-panel),
.dock :deep(.audit-panel) {
  position: static;
  inset: auto;
  width: 100%;
  height: 100%;
  max-height: none;
  z-index: auto;
  margin: 0;
  border: 0;
  border-radius: 0;
  box-shadow: none;
  background: transparent;
  backdrop-filter: none;
}

/* ---------- 卡片总览（Edge 手机版标签页 / iOS 清后台） ---------- */
.overview {
  position: fixed;
  inset: 0;
  z-index: 560;
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-3);
  padding: var(--ui-sp-5) var(--ui-sp-6);
  background: color-mix(in srgb, var(--ui-bg) 82%, transparent);
  backdrop-filter: blur(10px);
}
.ov-head {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-3);
  flex: none;
}
.ov-title {
  font-size: var(--ui-fs-14);
  color: var(--ui-text);
}
.ov-done {
  margin-left: auto;
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-4);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-sm);
  background: var(--ui-raised);
  color: var(--ui-text);
  font-size: var(--ui-fs-13);
}
.ov-done:hover {
  background: var(--ui-hover);
}
.ov-grid {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  grid-auto-rows: 148px;
  gap: var(--ui-sp-4);
  align-content: start;
}
.ov-card {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-md);
  background: var(--ui-panel);
  box-shadow: var(--ui-shadow);
  cursor: pointer;
  transition: transform var(--ui-dur-1) var(--ui-ease), border-color var(--ui-dur-1) var(--ui-ease);
}
.ov-card:hover {
  transform: translateY(-2px);
  border-color: var(--ui-border-strong);
}
.ov-card.on {
  border-color: var(--ui-accent-line);
  box-shadow: inset 0 -2px 0 var(--ui-accent), var(--ui-shadow);
}
.ov-card-head {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-bottom: 1px solid var(--ui-border);
  color: var(--ui-text);
  font-size: var(--ui-fs-13);
}
.ov-card-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ov-card-x {
  display: inline-grid;
  place-items: center;
  width: 18px;
  height: 18px;
  border: 0;
  border-radius: var(--ui-r-pill);
  background: var(--ui-hover);
  color: var(--ui-text-2);
}
.ov-card-x:hover {
  background: var(--ui-err);
  color: #fff;
}
.ov-card-body {
  flex: 1;
  display: grid;
  place-items: center;
  padding: var(--ui-sp-3);
}
.ov-card-desc {
  font-size: var(--ui-fs-12);
  color: var(--ui-text-3);
  text-align: center;
  line-height: var(--ui-line);
}

/* 总览层进场 / 卡片缩放（近似 iOS 应用切换器的手感） */
.fade-enter-active,
.fade-leave-active {
  transition: opacity var(--ui-dur-2) var(--ui-ease);
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
.card-enter-active,
.card-leave-active {
  transition: transform var(--ui-dur-2) var(--ui-ease), opacity var(--ui-dur-2) var(--ui-ease);
}
.card-enter-from {
  opacity: 0;
  transform: scale(0.92);
}
.card-leave-to {
  opacity: 0;
  transform: scale(0.9);
}

/* ---------- 拖放提示：只一圈虚线，不挡视线、不压颜色 ---------- */
.drop-layer {
  position: fixed;
  inset: var(--ui-sp-2);
  z-index: 600;
  pointer-events: none;
  border: 2px dashed var(--ui-accent-line);
  border-radius: var(--ui-r-lg);
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
