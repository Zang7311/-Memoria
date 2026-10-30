<!-- 卡片：面板内所有成块内容都用它，不再各写一套边框和内边距 -->
<script setup lang="ts">
withDefaults(
  defineProps<{
    title?: string
    hint?: string
    /** 内容区是否加内边距（表格类内容可以关掉自己控制） */
    padded?: boolean
  }>(),
  { title: '', hint: '', padded: true },
)
</script>

<template>
  <section class="card">
    <header v-if="title || $slots.actions" class="head">
      <div class="title-wrap">
        <span v-if="title" class="title">{{ title }}</span>
        <span v-if="hint" class="hint">{{ hint }}</span>
      </div>
      <div v-if="$slots.actions" class="actions"><slot name="actions" /></div>
    </header>
    <div class="body" :class="{ padded }"><slot /></div>
  </section>
</template>

<style scoped>
.card {
  background: var(--ui-raised);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-md);
  overflow: hidden;
}
.head {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-3) var(--ui-sp-3) 0;
}
.title-wrap {
  display: flex;
  align-items: baseline;
  gap: var(--ui-sp-2);
  min-width: 0;
  flex: 1;
}
.title {
  font-size: var(--ui-fs-13);
  color: var(--ui-text);
  font-weight: 600;
}
.hint {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.actions {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-1);
  flex: none;
}
.body.padded {
  padding: var(--ui-sp-3);
}
.body + .body {
  border-top: 1px solid var(--ui-border);
}
</style>
