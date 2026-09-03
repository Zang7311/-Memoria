<!-- 《铃·记忆体》主布局：顶部标题栏 + 中间对话流 + 底部输入栏
     任务 1：绑定主题 class，嵌入 StatusIndicator / ChatList / ChatInput
     AI-5：插件管理面板；AI-6：悬浮球开关 / 工具箱 / 设置页 -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import StatusIndicator from '../components/StatusIndicator.vue'
import ChatList from '../components/ChatList.vue'
import ChatInput from '../components/ChatInput.vue'
import MemoryPanel from '../components/MemoryPanel.vue'
import PluginManager from '../components/PluginManager.vue'
import ToolboxPanel from '../components/ToolboxPanel.vue'
import SettingView from './SettingView.vue'
import TheIcon from '../components/TheIcon.vue'
import { useSettingStore } from '../stores/settingStore'
import { useDesktopStore } from '../stores/desktopStore'
import { assetUrl, isImagePath } from '../utils/tauri'
import { useSyncStore } from '../stores/syncStore'
import { useChatStore } from '../stores/chatStore'
import { listen } from '@tauri-apps/api/event'

const setting = useSettingStore()
const desktop = useDesktopStore()
const sync = useSyncStore()
const chat = useChatStore()
const theme = computed(() => setting.theme || 'dark')
// 铃的头像：若是图片路径则显示图片（asset 协议加载），否则显示 emoji/文字
const avatarImg = computed(() => (isImagePath(setting.avatarSuzu) ? assetUrl(setting.avatarSuzu!) : null))

// —— AI-5：插件管理面板开关 ——
const showPlugins = ref(false)

// —— AI-6：设置页 / 工具箱 / 悬浮球 ——
const showSettings = ref(false)
const showToolbox = ref(false)

onMounted(() => {
  // 悬浮球状态与后端同步（初始隐藏）
  desktop.loadMonitorRules().catch(() => {})
  // AI-8：初始化同步 Store（网络状态监听 + 设备缓存）
  sync.init().catch(() => {})
  // 收尾批次3：加载多会话（无会话则自动新建）
  chat.init().catch(() => {})
  // v0.6：悬浮球面板「设置…」→ 打开设置页
  listen('floating-open-settings', () => {
    showSettings.value = true
    showToolbox.value = false
  }).catch(() => {})
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

// 一键切换人格（持久化到配置，API 模式通过 system prompt 生效）
async function onPersonaChange(e: Event) {
  const v = (e.target as HTMLSelectElement).value
  await setting.update({ persona: v }).catch(() => {})
}
</script>

<template>
  <div class="main-layout" :class="theme">
    <div class="main-body">
      <div class="chat-column">
        <!-- 顶部标题栏 -->
        <header class="top-bar">
          <div class="top-left">
            <div class="title-avatar">
              <img v-if="avatarImg" :src="avatarImg" class="avatar-img" />
              <template v-else>{{ setting.avatarSuzu || '铃' }}</template>
            </div>
            <div class="title-info">
              <span class="title-name">铃</span>
              <StatusIndicator />
            </div>
          </div>
          <div class="top-right">
            <!-- AI-8：网络状态指示器（点击进设置-同步） -->
            <span
              class="net-indicator"
              :title="'网络：' + (sync.networkStatus === 'online' ? '在线' : sync.networkStatus === 'offline' ? '离线（已切离线模式）' : '未知')"
              @click="openSettings"
            >
              <span v-if="sync.networkStatus === 'online'" class="dot online">🟢</span>
              <span v-else-if="sync.networkStatus === 'offline'" class="dot offline">🔴</span>
              <span v-else class="dot unknown">⚪</span>
            </span>
            <!-- AI-8：同步状态指示器（点击进设置-同步） -->
            <span
              class="sync-indicator"
              :class="sync.syncStatus"
              :title="'同步：' + sync.syncStatus"
              @click="openSettings"
            >
              {{
                sync.syncStatus === 'syncing' ? `🔄 ${Math.round(sync.syncProgress * 100)}%` :
                sync.syncStatus === 'done' ? '✅' :
                sync.syncStatus === 'error' ? '❌' : '🔁'
              }}
            </span>
            <!-- 一键换人格（API 模式生效） -->
            <select
              class="persona-select"
              :value="setting.persona"
              @change="onPersonaChange"
              title="一键切换人格（API 模式生效）"
            >
              <option value="daily">日常</option>
              <option value="chuunibyou">中二</option>
              <option value="healing">治愈</option>
              <option value="lewd">涩涩</option>
            </select>
            <span
              class="gear"
              :class="{ active: desktop.floatingBallVisible }"
              title="悬浮球（AI-6）"
              @click="toggleFloatingBall"
            >🪶</span>
            <span
              class="gear"
              :class="{ active: showToolbox }"
              title="铃的工具箱（AI-6）"
              @click="showToolbox = !showToolbox"
            ><TheIcon name="toolbox" :size="18" /></span>
            <span
              class="gear"
              :class="{ active: showPlugins }"
              title="插件管理（AI-5）"
              @click="showPlugins = !showPlugins"
            ><TheIcon name="plugin" :size="18" /></span>
            <span
              class="gear"
              :class="{ active: showSettings }"
              title="设置（AI-6）"
              @click="showSettings = !showSettings"
            ><TheIcon name="settings" :size="18" /></span>
          </div>
        </header>

        <!-- 多会话标签栏（收尾批次3） -->
        <div class="session-tabs">
          <div
            v-for="s in chat.sessions"
            :key="s.id"
            class="session-tab"
            :class="{ active: s.id === chat.activeSessionId }"
            :title="s.title"
            @click="chat.switchSession(s.id)"
          >
            <span class="s-title">{{ s.title }}</span>
            <span class="s-del" title="删除会话" @click.stop="onDeleteSession(s.id)"><TheIcon name="close" :size="10" /></span>
          </div>
          <button class="new-session" title="新建会话" @click="chat.createSession()"><TheIcon name="add" :size="16" /></button>
        </div>

        <!-- 中间对话流 -->
        <ChatList />

        <!-- 底部输入栏 -->
        <ChatInput />

        <!-- 会话底部 token 统计（收尾批次2；始终显示，API 回复后填充真实用量） -->
        <div class="usage-bar" title="本次回复的 token 用量">
          <template v-if="chat.lastUsage">
            ⚡ 本次回复：输入 {{ chat.lastUsage.prompt_tokens }} · 输出 {{ chat.lastUsage.completion_tokens }} · 合计 {{ chat.lastUsage.total_tokens }} tokens
          </template>
          <template v-else>
            ⚡ token 统计：等待 API 回复后显示（脚本/本地模式无 token 消耗）
          </template>
        </div>
      </div>

      <!-- 右侧记忆面板（AI-4） -->
      <MemoryPanel />

      <!-- 插件管理面板（AI-5） -->
      <PluginManager v-if="showPlugins" />

      <!-- 工具箱悬浮面板（AI-6） -->
      <ToolboxPanel v-if="showToolbox" @close="showToolbox = false" />

      <!-- 设置页遮罩（AI-6） -->
      <div v-if="showSettings" class="settings-overlay" @click.self="showSettings = false">
        <div class="settings-panel">
          <button class="close-btn" title="关闭" @click="showSettings = false">✕</button>
          <SettingView />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.main-layout {
  height: 100vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--bg-main, #f6f6f6);
  color: var(--text-main, #222);
  transition: background 0.25s ease, color 0.25s ease;
}
.main-body {
  display: flex;
  flex: 1;
  min-height: 0;
  position: relative;
}
.chat-column {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
}
.top-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  border-bottom: 1px solid var(--border, rgba(128, 128, 128, 0.2));
  background: var(--bg-bar, rgba(255, 255, 255, 0.7));
  backdrop-filter: blur(8px);
}
.top-left {
  display: flex;
  align-items: center;
  gap: 10px;
}
.title-avatar {
  width: 38px;
  height: 38px;
  border-radius: 50%;
  background: var(--bubble-suzu-bg, linear-gradient(135deg, #ffe4e1, #fff0f5));
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--fs-20);
  overflow: hidden;
}
.avatar-img { width: 100%; height: 100%; object-fit: cover; }
.title-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.title-name {
  font-size: var(--fs-16);
  font-weight: 600;
  color: var(--text-main, #222);
}
.top-right {
  display: flex;
  align-items: center;
  gap: 10px;
}
.top-right .net-indicator,
.top-right .sync-indicator {
  font-size: var(--fs-15);
  cursor: pointer;
  opacity: 0.85;
  transition: opacity 0.15s ease;
  display: inline-flex;
  align-items: center;
}
.top-right .net-indicator:hover,
.top-right .sync-indicator:hover {
  opacity: 1;
}
.top-right .sync-indicator.syncing {
  animation: sync-pulse 1.2s ease-in-out infinite;
}
@keyframes sync-pulse {
  0%, 100% { opacity: 0.6; }
  50% { opacity: 1; }
}
.top-right .gear {
  font-size: var(--fs-18);
  cursor: pointer;
  opacity: 0.7;
  color: var(--text-secondary, #999);
  transition: opacity 0.15s ease, transform 0.15s ease;
}
.top-right .gear:hover {
  opacity: 1;
  transform: rotate(30deg);
}
.top-right .gear.active {
  opacity: 1;
  transform: rotate(30deg);
  color: var(--accent);
}
.persona-select {
  background: transparent;
  border: 1px solid var(--border, rgba(128, 128, 128, 0.3));
  color: var(--text-main, #222);
  border-radius: 8px;
  padding: 3px 6px;
  font-size: var(--fs-12);
  cursor: pointer;
  max-width: 96px;
}
/* 多会话标签栏 */
.session-tabs {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 12px;
  overflow-x: auto;
  border-bottom: 1px solid var(--border, rgba(128, 128, 128, 0.2));
  background: var(--bg-bar, rgba(255, 255, 255, 0.4));
  flex-shrink: 0;
}
.session-tab {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 3px 8px;
  border-radius: 8px;
  font-size: var(--fs-12);
  cursor: pointer;
  background: rgba(128, 128, 128, 0.12);
  color: var(--text-secondary, #999);
  white-space: nowrap;
  max-width: 160px;
}
.session-tab.active {
  background: var(--accent, #ff7a94);
  color: var(--text-user);
}
.s-title { overflow: hidden; text-overflow: ellipsis; }
.s-del { opacity: 0.6; font-size: var(--fs-10); padding-left: 2px; color: var(--text-secondary, #999); }
.s-del:hover { opacity: 1; }
.new-session {
  border: 1px dashed var(--border, rgba(128, 128, 128, 0.4));
  background: transparent;
  color: var(--text-secondary, #999);
  border-radius: 8px;
  width: 24px;
  height: 22px;
  cursor: pointer;
  font-size: var(--fs-13);
  flex-shrink: 0;
}
.new-session:hover { color: var(--accent, #ff7a94); border-color: var(--accent, #ff7a94); }
/* 设置页遮罩 */
.settings-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 550;
}
.settings-panel {
  position: relative;
  width: 820px;
  height: 86vh;
  background: var(--bg-main, #1d1b1f);
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  border-radius: 16px;
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5);
  overflow: hidden;
}
.close-btn {
  position: absolute;
  top: 10px;
  right: 12px;
  z-index: 10;
  width: 28px;
  height: 28px;
  border-radius: 8px;
  border: none;
  background: rgba(128, 128, 128, 0.2);
  color: var(--text-main);
  cursor: pointer;
  font-size: var(--fs-13);
}
.close-btn:hover {
  background: var(--danger-bg);
}
.usage-bar {
  padding: 4px 16px;
  font-size: var(--fs-11);
  color: var(--text-secondary, #999);
  border-top: 1px dashed var(--border, rgba(128, 128, 128, 0.2));
  text-align: right;
  background: var(--bg-bar, rgba(255, 255, 255, 0.5));
  user-select: none;
}
</style>
