<!-- 空态：一句话说明 + 一个可选操作，居中显示 -->
<script setup lang="ts">
withDefaults(
  defineProps<{
    text: string
    hint?: string
    actionLabel?: string
  }>(),
  { hint: '', actionLabel: '' },
)
const emit = defineEmits<{ action: [] }>()
</script>

<template>
  <div class="empty">
    <p class="text">{{ text }}</p>
    <p v-if="hint" class="hint">{{ hint }}</p>
    <button v-if="actionLabel" type="button" class="act" @click="emit('action')">
      {{ actionLabel }}
    </button>
  </div>
</template>

<style scoped>
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-6) var(--ui-sp-4);
  text-align: center;
}
.text {
  margin: 0;
  font-size: var(--ui-fs-13);
  color: var(--ui-text);
}
.hint {
  margin: 0;
  font-size: var(--ui-fs-12);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
  max-width: 32em;
}
.act {
  margin-top: var(--ui-sp-1);
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-3);
  border: 1px solid var(--ui-border);
  background: var(--ui-raised);
  color: var(--ui-text);
  border-radius: var(--ui-r-sm);
  font-size: var(--ui-fs-13);
}
.act:hover {
  background: var(--ui-hover);
  border-color: var(--ui-border-strong);
}
</style>
