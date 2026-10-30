<!-- 分页标签：胶囊组，选中只有底色，不用下划线/彩色条 -->
<script setup lang="ts">
export interface TabItem {
  id: string
  label: string
  count?: number
}
defineProps<{
  items: TabItem[]
  modelValue: string
}>()
const emit = defineEmits<{ 'update:modelValue': [id: string] }>()
</script>

<template>
  <div class="tabs" role="tablist">
    <button
      v-for="t in items"
      :key="t.id"
      type="button"
      role="tab"
      class="tab"
      :class="{ on: t.id === modelValue }"
      :aria-selected="t.id === modelValue"
      @click="emit('update:modelValue', t.id)"
    >
      {{ t.label }}
      <span v-if="t.count" class="count">{{ t.count }}</span>
    </button>
  </div>
</template>

<style scoped>
.tabs {
  display: flex;
  align-items: center;
  gap: 2px;
  min-width: 0;
  overflow-x: auto;
  scrollbar-width: none;
}
.tabs::-webkit-scrollbar {
  display: none;
}
.tab {
  display: inline-flex;
  align-items: center;
  gap: var(--ui-sp-1);
  border: 0;
  background: transparent;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-13);
  padding: var(--ui-sp-1) var(--ui-sp-3);
  border-radius: var(--ui-r-pill);
  white-space: nowrap;
  transition: background var(--ui-dur-1) var(--ui-ease), color var(--ui-dur-1) var(--ui-ease);
}
.tab:hover {
  color: var(--ui-text);
  background: var(--ui-hover);
}
.tab.on {
  color: var(--ui-text);
  background: var(--ui-active);
}
.count {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
}
</style>
