// 《铃·记忆体》内置插件：文本工具箱
// 演示点：纯 JS 技能（无权限、无沙箱调用），Base64 实现为手写算法（boa_engine 无 btoa/atob）
var B64_CHARS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

// UTF-8 字符串 → 字节数组（利用 encodeURIComponent 的 %XX 转义）
// 注意：boa_engine 无 String.prototype.substr，用 substring
function utf8ToBytes(str) {
  var encoded = encodeURIComponent(str);
  var bytes = [];
  for (var i = 0; i < encoded.length; i++) {
    if (encoded.charAt(i) === '%') {
      bytes.push(parseInt(encoded.substring(i + 1, i + 3), 16));
      i += 2;
    } else {
      bytes.push(encoded.charCodeAt(i));
    }
  }
  return bytes;
}

// 字节数组 → UTF-8 字符串
function bytesToUtf8(bytes) {
  var parts = [];
  for (var i = 0; i < bytes.length; i++) {
    var b = bytes[i];
    if (b < 128) {
      parts.push(String.fromCharCode(b));
    } else if (b >= 192 && b < 224 && i + 1 < bytes.length) {
      parts.push(String.fromCharCode(((b & 31) << 6) | (bytes[++i] & 63)));
    } else if (i + 2 < bytes.length) {
      parts.push(String.fromCharCode(((b & 15) << 12) | ((bytes[i + 1] & 63) << 6) | (bytes[i + 2] & 63)));
      i += 2;
    }
  }
  return parts.join('');
}

// Base64 编码
function encodeBase64(str) {
  var bytes = utf8ToBytes(str);
  var out = '';
  for (var i = 0; i < bytes.length; i += 3) {
    var b0 = bytes[i];
    var b1 = i + 1 < bytes.length ? bytes[i + 1] : 0;
    var b2 = i + 2 < bytes.length ? bytes[i + 2] : 0;
    out += B64_CHARS.charAt(b0 >> 2);
    out += B64_CHARS.charAt(((b0 & 3) << 4) | (b1 >> 4));
    out += i + 1 < bytes.length ? B64_CHARS.charAt(((b1 & 15) << 2) | (b2 >> 6)) : '=';
    out += i + 2 < bytes.length ? B64_CHARS.charAt(b2 & 63) : '=';
  }
  return out;
}

// Base64 解码
function decodeBase64(str) {
  str = String(str).replace(/[^A-Za-z0-9+/=]/g, '');
  var bytes = [];
  var buffer = 0;
  var bits = 0;
  for (var i = 0; i < str.length; i++) {
    if (str.charAt(i) === '=') break;
    var val = B64_CHARS.indexOf(str.charAt(i));
    if (val < 0) continue;
    buffer = (buffer << 6) | val;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      bytes.push((buffer >> bits) & 255);
      buffer = buffer & ((1 << bits) - 1);
    }
  }
  return bytesToUtf8(bytes);
}

globalThis.skills = {
  base64_encode: function (params) {
    var text = params.text || '';
    if (!text) return { success: false, error: '缺少 text 参数' };
    console.log('Base64 编码：', text.length, '字符');
    return { success: true, result: encodeBase64(text) };
  },

  base64_decode: function (params) {
    var text = params.text || '';
    if (!text) return { success: false, error: '缺少 text 参数' };
    try {
      return { success: true, result: decodeBase64(text) };
    } catch (e) {
      return { success: false, error: '解码失败：' + e };
    }
  },

  count_words: function (params) {
    var text = params.text || '';
    if (!text) return { success: false, error: '缺少 text 参数' };
    var cn = (text.match(/[\u4e00-\u9fa5]/g) || []).length;
    var en = (text.replace(/[\u4e00-\u9fa5]/g, ' ').match(/[A-Za-z0-9]+/g) || []).length;
    return {
      success: true,
      chinese_chars: cn,
      english_words: en,
      total_chars: text.length
    };
  },

  to_upper: function (params) {
    var text = params.text || '';
    return { success: true, result: text.toUpperCase() };
  }
};
