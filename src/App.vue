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
const theme = computed(() => setting.theme || 'dark')

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
/* ================= 五套 UI 风格主题（大版本方向） ================= */
/* ① Win10：亚克力深灰 + 微软蓝 */
.app-root.win10 {
  --bg-main: #202225;
  --bg-bar: rgba(32, 34, 37, 0.92);
  --text-main: #f0f0f0;
  --text-secondary: #a0a0a0;
  --text-user: #ffffff;
  --text-suzu: #d8d8d8;
  --bubble-user-bg: #2f3136;
  --bubble-suzu-bg: #3a3d42;
  --border: rgba(255, 255, 255, 0.12);
  --input-bg: #2b2d30;
  --accent: #00a4ef;
  --danger: #e81123;
}
/* ② 微软浏览器(Edge)：深蓝灰 + 简洁浏览器感 */
.app-root.edge {
  --bg-main: #232627;
  --bg-bar: rgba(35, 38, 39, 0.9);
  --text-main: #e8eaed;
  --text-secondary: #9aa0a6;
  --text-user: #ffffff;
  --text-suzu: #d5dbe0;
  --bubble-user-bg: #2c2f31;
  --bubble-suzu-bg: #3a3f42;
  --border: rgba(255, 255, 255, 0.13);
  --input-bg: #2a2d2f;
  --accent: #4d8bf5;
  --danger: #f28b82;
}
/* ③ 极简文字风：纯黑白 + 高对比 */
.app-root.minimal {
  --bg-main: #0d0d0d;
  --bg-bar: rgba(13, 13, 13, 0.96);
  --text-main: #d4d4d4;
  --text-secondary: #666666;
  --text-user: #000000;
  --text-suzu: #cccccc;
  --bubble-user-bg: #ffffff;
  --bubble-suzu-bg: #1a1a1a;
  --border: rgba(255, 255, 255, 0.22);
  --input-bg: #161616;
  --accent: #888888;
  --danger: #ff4444;
}
/* ④ iOS 毛玻璃之前(扁平化)：纯色块 + 系统蓝 */
.app-root.ios-flat {
  --bg-main: #1c1c1e;
  --bg-bar: rgba(44, 44, 46, 0.95);
  --text-main: #ffffff;
  --text-secondary: #98989d;
  --text-user: #ffffff;
  --text-suzu: #c8c8cc;
  --bubble-user-bg: #0a84ff;
  --bubble-suzu-bg: #2c2c2e;
  --border: rgba(255, 255, 255, 0.15);
  --input-bg: #2c2c2e;
  --accent: #0a84ff;
  --danger: #ff453a;
}
/* ⑤ iOS 毛玻璃之后(现代)：半透明 + 粉红 + 柔和渐变 */
.app-root.ios-glass {
  --bg-main: #1b1b1f;
  --bg-bar: rgba(44, 44, 46, 0.55);
  --text-main: #ffffff;
  --text-secondary: #a0a0a8;
  --text-user: #ffffff;
  --text-suzu: #f0d8e0;
  --bubble-user-bg: #ff2d55;
  --bubble-suzu-bg: linear-gradient(135deg, #4a3a4a, #3a2a3e);
  --border: rgba(255, 255, 255, 0.2);
  --input-bg: rgba(44, 44, 46, 0.6);
  --accent: #ff2d55;
  --danger: #ff453a;
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
