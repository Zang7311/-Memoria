// 《铃·记忆体》应用入口
// 注册 Pinia 状态管理；全局设计令牌（src/styles/tokens.css）必须先于组件加载
import { createApp } from "vue";
import { createPinia } from "pinia";
import "./styles/tokens.css";
import App from "./App.vue";

const app = createApp(App);
app.use(createPinia());
app.mount("#app");
