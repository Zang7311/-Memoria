// DirectXTK 的 WICTextureLoader 替代品（只用 Windows 自带 WIC，避免引入 DirectXTK 依赖）
// 只实现官方 D3D11 示例用到的那一个函数：CreateWICTextureFromFileEx
// 行为约定：解码成 RGBA8、单层 MIP、不预乘（预乘由 CubismTextureManager 自己在 CPU 侧做）

#pragma once

#include <d3d11.h>
#include <cstddef>

namespace DirectX {

enum WIC_LOADER_FLAGS : unsigned int {
    WIC_LOADER_DEFAULT = 0,
    WIC_LOADER_FORCE_SRGB = 0x1,
    WIC_LOADER_IGNORE_SRGB = 0x2,
    WIC_LOADER_MIP_AUTOGEN = 0x8,
    WIC_LOADER_FIT_POW2 = 0x20,
    WIC_LOADER_MAKE_SQUARE = 0x40,
    WIC_LOADER_FORCE_RGBA32 = 0x80,
};

HRESULT CreateWICTextureFromFileEx(
    ID3D11Device* device,
    ID3D11DeviceContext* context,
    const wchar_t* fileName,
    size_t maxsize,
    D3D11_USAGE usage,
    unsigned int bindFlags,
    unsigned int cpuAccessFlags,
    unsigned int miscFlags,
    unsigned int loadFlags,
    ID3D11Resource** texture,
    ID3D11ShaderResourceView** textureView);

}  // namespace DirectX
