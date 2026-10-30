<!-- 设置 · 记忆
     记忆存储路径展示 + 配置（记忆库配置）导入导出。
     记忆集的增删与详情仍在主界面「记忆」面板，这里只做入口与备份。 -->
<script setup lang="ts">
import { onMounted, ref } from 'vue'
import AppButton from '../ui/AppButton.vue'
import AppCard from '../ui/AppCard.vue'
import AppField from '../ui/AppField.vue'
import AppInput from '../ui/AppInput.vue'
import { useSettingStore } from '../../stores/settingStore'
import { ensureSettingsLoaded, exportConfigFile, importConfigFromFile } from './shared'

const setting = useSettingStore()
const importPath = ref('')

async function onImport() {
  if (await importConfigFromFile(importPath.value)) importPath.value = ''
}

onMounted(async () => {
  await ensureSettingsLoaded()
})
</script>

<template>
  <div class="page">
    <AppCard title="记忆存储">
      <AppField label="记忆文件保存在" :hint="setting.dataPath || '（未设置，默认文档/铃记忆体）'">
        <p class="muted">换路径请到「设置 → 通用 → 数据路径」修改，改完新的记忆会写到新位置。</p>
      </AppField>
      <p class="muted">记忆集管理与详情请使用主界面「记忆」面板。</p>
    </AppCard>

    <AppCard title="记忆配置备份">
      <p class="muted">导出的是整份配置 JSON（含记忆库相关设置），可用于换机或回滚。</p>
      <div class="row">
        <AppButton @click="exportConfigFile">导出记忆配置</AppButton>
      </div>
      <AppField label="导入" hint="填写配置 JSON 文件的完整路径，导入后立即生效。">
        <div class="row">
          <AppInput v-model="importPath" placeholder="C:\Users\...\铃记忆体配置.json" />
          <AppButton variant="primary" @click="onImport">导入</AppButton>
        </div>
      </AppField>
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
.muted {
  margin: 0 0 var(--ui-sp-2);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text-3);
  overflow-wrap: anywhere;
}
</style>
