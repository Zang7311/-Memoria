<!-- 状态点：替代 🟢🔴⚪ 这类 emoji 状态 -->
<script setup lang="ts">
withDefaults(
  defineProps<{
    state?: 'ok' | 'warn' | 'err' | 'idle'
    text?: string
  }>(),
  { state: 'idle', text: '' },
)
</script>

<template>
  <span class="dot-wrap" :title="text || undefined">
    <span class="dot" :class="state" aria-hidden="true" />
    <span v-if="text" class="txt">{{ text }}</span>
  </span>
</template>

<style scoped>
.dot-wrap {
  display: inline-flex;
  align-items: center;
  gap: var(--ui-sp-1);
  min-width: 0;
}
.dot {
  width: 6px;
  height: 6px;
  border-radius: var(--ui-r-pill);
  flex: none;
  background: var(--ui-text-3);
}
.dot.ok {
  background: var(--ui-ok);
}
.dot.warn {
  background: var(--ui-warn);
}
.dot.err {
  background: var(--ui-err);
}
.txt {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  white-space: nowrap;
}
</style>
