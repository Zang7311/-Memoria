<!-- 设置 · 模型页底部 · 主密码与加密
     主密码设置 / 解锁 / 修改；密钥加密状态一览 -->
<script setup lang="ts">
import { computed, ref } from 'vue'
import AppButton from '../ui/AppButton.vue'
import AppCard from '../ui/AppCard.vue'
import AppField from '../ui/AppField.vue'
import AppInput from '../ui/AppInput.vue'
import AppStatusDot from '../ui/AppStatusDot.vue'
import { useSettingStore } from '../../stores/settingStore'
import { toastErr, toastOk } from '../../utils/toast'

const setting = useSettingStore()

const masterPwd = ref('')
const masterPwd2 = ref('')
const unlockPwd = ref('')

const statusState = computed<'ok' | 'warn' | 'err'>(() => {
  if (!setting.hasMasterPassword) return 'err'
  return setting.unlocked ? 'ok' : 'warn'
})
const statusText = computed(() => {
  if (!setting.hasMasterPassword) return '未设置（密钥将明文存储，建议设置主密码加密）'
  return setting.unlocked ? '已设置 · 已解锁' : '已设置 · 未解锁'
})

async function doSetupMaster() {
  if (!masterPwd.value) return
  if (masterPwd.value !== masterPwd2.value) {
    toastErr('两次输入不一致')
    return
  }
  try {
    await setting.setupMasterPassword(masterPwd.value)
    masterPwd.value = ''
    masterPwd2.value = ''
    toastOk('主密码已设置并解锁')
  } catch (e) {
    toastErr(`${e}`)
  }
}
async function doUnlock() {
  try {
    await setting.unlockVault(unlockPwd.value)
    unlockPwd.value = ''
    toastOk('已解锁')
  } catch (e) {
    toastErr(`${e}`)
  }
}
</script>

<template>
  <AppCard title="主密码与加密">
    <div class="status-line">
      <span>主密码状态</span>
      <AppStatusDot :state="statusState" :text="statusText" />
    </div>

    <template v-if="!setting.hasMasterPassword">
      <AppField label="设置主密码" hint="设置后，已保存的 API 密钥会被加密存储；主密码本身不写盘内容。">
        <div class="row">
          <AppInput v-model="masterPwd" type="password" placeholder="设置主密码" />
          <AppInput v-model="masterPwd2" type="password" placeholder="确认主密码" />
          <AppButton variant="primary" @click="doSetupMaster">设置</AppButton>
        </div>
      </AppField>
    </template>

    <template v-else-if="!setting.unlocked">
      <AppField label="解锁" hint="解锁后才能保存或清除密钥。">
        <div class="row">
          <AppInput v-model="unlockPwd" type="password" placeholder="输入主密码解锁" />
          <AppButton variant="primary" @click="doUnlock">解锁</AppButton>
        </div>
      </AppField>
    </template>

    <template v-else>
      <AppField label="修改主密码" hint="改完立即生效，旧密钥会按新主密码重新加密。">
        <div class="row">
          <AppInput v-model="masterPwd" type="password" placeholder="新主密码（修改）" />
          <AppInput v-model="masterPwd2" type="password" placeholder="确认新主密码" />
          <AppButton @click="doSetupMaster">修改</AppButton>
        </div>
      </AppField>
    </template>
  </AppCard>
</template>

<style scoped>
.row {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  min-width: 0;
  flex-wrap: wrap;
}
.status-line {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  margin-bottom: var(--ui-sp-2);
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
}
</style>
