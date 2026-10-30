<!-- 《铃·记忆体》根组件：按窗口 label 分发渲染
     main → 主布局 / floating-ball → 悬浮球 / bubble → 气泡弹窗 -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import MainLayout from './views/MainLayout.vue'
import FloatingBall from './components/FloatingBall.vue'
import Bubble from './components/Bubble.vue'
import OnboardingView from './views/OnboardingView.vue'
import RecoveryView from './views/RecoveryView.vue'
import AppToast from './components/ui/AppToast.vue'
import AppDialog from './components/ui/AppDialog.vue'
import { useSettingStore } from './stores/settingStore'
import { useMilestoneStore } from './stores/milestoneStore'
import { assetUrl } from './utils/tauri'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { listen } from '@tauri-apps/api/event'

const setting = useSettingStore()
const milestone = useMilestoneStore()
const theme = computed(() => setting.theme || 'dark')
// 外观自定义：把用户自定值映射为 CSS 变量（覆盖主题色/背景/圆角，仅添加已设置的）
const customStyle = computed<Record<string, string>>(() => {
  const s: Record<string, string> = {}
  if (setting.accentColor) {
    s['--accent'] = setting.accentColor
    // 主色联动：未单独设置危险色时，危险色用主色的深色调（保持风格统一）
    if (!setting.dangerColor) s['--danger'] = setting.accentColor
    // 主色联动：未单独设置铃的气泡色时，气泡用主色浅渐变（跟随风格主色）
    if (!setting.bubbleSuzuColor) {
      s['--bubble-suzu-bg'] = `linear-gradient(135deg, color-mix(in srgb, ${setting.accentColor} 38%, transparent), color-mix(in srgb, ${setting.accentColor} 18%, transparent))`
    }
  }
  if (setting.dangerColor) s['--danger'] = setting.dangerColor
  if (setting.bgColor) s['--bg-main'] = setting.bgColor
  if (setting.bgImage) s['--bg-image'] = `url("${assetUrl(setting.bgImage)}")`
  if (setting.uiRadius != null) s['--radius-ui'] = `${setting.uiRadius}px`
  if (setting.bubbleUserColor) s['--bubble-user-bg'] = setting.bubbleUserColor
  if (setting.bubbleSuzuColor) s['--bubble-suzu-bg'] = setting.bubbleSuzuColor
  // 新聊天界面的气泡色：默认铃无底色、主人用 raised；用户设置过才覆盖（旧 --bubble-*-bg 留给悬浮球/气泡窗口）
  if (setting.bubbleUserColor) s['--ui-bubble-user'] = setting.bubbleUserColor
  if (setting.bubbleSuzuColor) s['--ui-bubble-suzu'] = setting.bubbleSuzuColor
  return s
})

// 当前窗口 label（Tauri 环境；非 Tauri 环境默认主窗口）
const winLabel = ref('main')
// 首次启动引导状态（仅主窗口）
const firstLaunch = ref(false)
const onboarded = ref(false)
// moon12：旧版数据迁移完成提示（一次性横幅，5 秒自动消失）
const legacyMigrated = ref('')

onMounted(async () => {
  try {
    winLabel.value = getCurrentWindow().label
  } catch {
    winLabel.value = 'main'
  }
  // 主窗口：加载配置，若是首次启动则显示引导页
  if (winLabel.value === 'main') {
    try {
      await setting.loadConfig()
      firstLaunch.value = setting.firstLaunch && !onboarded.value
    } catch {
      firstLaunch.value = false
    }
    // P3：陪伴记录——首次见面自动记录（后端幂等），并加载已有里程碑
    milestone.load().catch(() => { /* 静默 */ })
    if (!setting.firstLaunch) {
      milestone.record('first_launch', '与铃初次见面').catch(() => { /* 静默 */ })
    }
    // moon12：监听旧版数据迁移完成事件，提示用户
    listen<string>('legacy-migrated', (event) => {
      legacyMigrated.value = `已从旧版本迁移数据：${event.payload}`
      setTimeout(() => (legacyMigrated.value = ''), 5000)
    }).catch(() => { /* 非 Tauri 环境忽略 */ })
  }
  // 引导完成后：进入主界面
  window.addEventListener('onboarding-done', () => {
    onboarded.value = true
    firstLaunch.value = false
    setting.firstLaunch = false
  })
})
</script>

<template>
  <div class="app-root" :class="theme" :style="customStyle">
    <!-- moon12：旧版数据迁移完成提示条 -->
    <Transition name="fade">
      <div v-if="legacyMigrated" class="legacy-migrated-bar">{{ legacyMigrated }}</div>
    </Transition>
    <!-- 首次启动引导（仅主窗口） -->
    <OnboardingView v-if="winLabel === 'main' && firstLaunch && !onboarded" />
    <MainLayout v-else-if="winLabel === 'main'" />
    <FloatingBall v-else-if="winLabel === 'floating-ball'" />
    <Bubble v-else-if="winLabel === 'bubble'" />
    <!-- P3：救援模式窗口（--recovery 启动） -->
    <RecoveryView v-else-if="winLabel === 'recovery'" />
    <!-- 全局提示与确认框（替代 alert/confirm），所有窗口共用 -->
    <AppToast />
    <AppDialog />
  </div>
</template>

<style scoped>
.app-root {
  height: 100vh;
  width: 100vw;
  background: var(--ui-bg);
}
/* moon12：旧版数据迁移完成提示条 */
.legacy-migrated-bar {
  position: fixed;
  top: var(--ui-sp-3);
  left: 50%;
  transform: translateX(-50%);
  z-index: 9999;
  max-width: 80%;
  padding: var(--ui-sp-2) var(--ui-sp-4);
  border-radius: var(--ui-r-md);
  background: var(--ui-ok);
  color: var(--ui-on-accent);
  font-size: var(--ui-fs-13);
  box-shadow: var(--ui-shadow);
}
.fade-enter-active,
.fade-leave-active {
  transition: opacity var(--ui-dur-2) var(--ui-ease);
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
