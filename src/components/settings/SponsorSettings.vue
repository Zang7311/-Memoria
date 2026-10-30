<!-- 设置 · 赞助
     赞助入口（渠道与名单为预留位） + 项目仓库 / 作者主页跳转 -->
<script setup lang="ts">
import AppButton from '../ui/AppButton.vue'
import AppCard from '../ui/AppCard.vue'
import { toastErr } from '../../utils/toast'
import { openUrl } from '../../utils/tauri'

function openExternal(url: string) {
  openUrl(url).catch((e) => toastErr(`打开链接失败：${e}`))
}
</script>

<template>
  <div class="page">
    <AppCard title="赞助">
      <p class="muted">支持铃·记忆体开源项目。赞助渠道、目标进度、赞助者名单在此展示（预留）。</p>
      <div class="row">
        <AppButton variant="primary" @click="openExternal('https://github.com/Zang7311')">访问 GitHub 主页</AppButton>
        <AppButton @click="openExternal('https://github.com/Zang7311/-Memoria')">项目仓库</AppButton>
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
  flex-wrap: wrap;
}
.muted {
  margin: 0 0 var(--ui-sp-3);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text-3);
}
</style>
