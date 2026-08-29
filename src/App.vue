<!-- 《铃·记忆体》根组件：按窗口 label 分发渲染
     main → 主布局 / floating-ball → 悬浮球 / bubble → 气泡弹窗 -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import MainLayout from './views/MainLayout.vue'
import FloatingBall from './components/FloatingBall.vue'
import Bubble from './components/Bubble.vue'
import OnboardingView from './views/OnboardingView.vue'
import { useSettingStore } from './stores/settingStore'
import { getCurrentWindow } from '@tauri-apps/api/window'

const setting = useSettingStore()
const theme = computed(() => (setting.theme === 'dark' ? 'dark' : 'light'))

// 当前窗口 label（Tauri 环境；非 Tauri 环境默认主窗口）
const winLabel = ref('main')
// 首次启动引导状态（仅主窗口）
const firstLaunch = ref(false)
const onboarded = ref(false)

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
  <div class="app-root" :class="theme">
    <!-- 首次启动引导（仅主窗口） -->
    <OnboardingView v-if="winLabel === 'main' && firstLaunch && !onboarded" />
    <MainLayout v-else-if="winLabel === 'main'" />
    <FloatingBall v-else-if="winLabel === 'floating-ball'" />
    <Bubble v-else-if="winLabel === 'bubble'" />
  </div>
</template>

<style scoped>
.app-root {
  height: 100vh;
  width: 100vw;
  background: var(--bg-main, #f6f6f6);
}
</style>

<!-- 全局主题变量定义（亮/暗两套） -->
<style>
:root,
.app-root.light {
  --bg-main: #f7f2f4;
  --bg-bar: rgba(255, 255, 255, 0.75);
  --text-main: #2b2323;
  --text-secondary: #8a8082;
  --text-user: #ffffff;
  --text-suzu: #5b3a63;
  --bubble-user-bg: #2d2d2d;
  --bubble-suzu-bg: linear-gradient(135deg, #ffe4e1, #fff0f5);
  --border: rgba(128, 128, 128, 0.25);
  --input-bg: #ffffff;
  --accent: #ff8fa3;
  --danger: #d9534f;
}
.app-root.dark {
  --bg-main: #1d1b1f;
  --bg-bar: rgba(34, 32, 36, 0.85);
  --text-main: #eee6e7;
  --text-secondary: #9a9294;
  --text-user: #ffffff;
  --text-suzu: #f3d9d9;
  --bubble-user-bg: #3a3438;
  --bubble-suzu-bg: linear-gradient(135deg, #4a3641, #3a2a36);
  --border: rgba(255, 255, 255, 0.12);
  --input-bg: #2a272b;
  --accent: #ff7a94;
  --danger: #ff6b6b;
}
html,
body {
  margin: 0;
  padding: 0;
  height: 100%;
  font-family: system-ui, 'Microsoft YaHei', -apple-system, sans-serif;
}
#app {
  height: 100%;
}
</style>
