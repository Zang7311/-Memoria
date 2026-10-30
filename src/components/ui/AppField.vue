<!-- 表单一行：标签 + 控件 + 说明。设置页所有配置项都用它，保证间距和字号统一 -->
<script setup lang="ts">
withDefaults(
  defineProps<{
    label?: string
    hint?: string
    /** 标签和控件并排（用于开关、单选这类短控件） */
    inline?: boolean
  }>(),
  { label: '', hint: '', inline: false },
)
</script>

<template>
  <div class="field" :class="{ inline }">
    <label v-if="label" class="label">{{ label }}</label>
    <div class="control"><slot /></div>
    <p v-if="hint" class="hint">{{ hint }}</p>
  </div>
</template>

<style scoped>
.field {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
  padding: var(--ui-sp-2) 0;
  min-width: 0;
}
.field.inline {
  flex-direction: row;
  align-items: center;
  gap: var(--ui-sp-3);
}
.label {
  flex: none;
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
}
.field.inline .control {
  flex: 1;
  min-width: 0;
}
.hint {
  margin: 0;
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
}
</style>
