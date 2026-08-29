// Hermes 兼容层示例插件入口
// 说明：Hermes 插件原运行于 Node 环境（依赖 Hermes SDK）。
// 《铃》的兼容层提供受限沙箱：纯 JS 逻辑可直接运行，Node API 不可用。
globalThis.skills = {
  hermes_hello: function (params) {
    var who = params.who || '世界';
    console.log('Hermes 插件被调用，向「' + who + '」问好');
    return '你好，' + who + '！——来自 Hermes 兼容层插件';
  }
};
