// DirectXTK 的 WICTextureLoader 替代实现（Windows 自带 WIC）
#include <windows.h>
#include <d3d11.h>
#include <wincodec.h>
#include <wrl/client.h>

#include <vector>

#include "WICTextureLoader.h"

using Microsoft::WRL::ComPtr;

namespace DirectX {

HRESULT CreateWICTextureFromFileEx(
    ID3D11Device* device,
    ID3D11DeviceContext* /*context*/,
    const wchar_t* fileName,
    size_t /*maxsize*/,
    D3D11_USAGE usage,
    unsigned int bindFlags,
    unsigned int cpuAccessFlags,
    unsigned int miscFlags,
    unsigned int /*loadFlags*/,
    ID3D11Resource** texture,
    ID3D11ShaderResourceView** textureView) {
    if (!device || !fileName || !texture || !textureView) return E_INVALIDARG;
    *texture = nullptr;
    *textureView = nullptr;

    ComPtr<IWICImagingFactory> factory;
    HRESULT hr = CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER,
                                  IID_PPV_ARGS(&factory));
    if (FAILED(hr)) return hr;

    ComPtr<IWICBitmapDecoder> decoder;
    hr = factory->CreateDecoderFromFilename(fileName, nullptr, GENERIC_READ,
                                            WICDecodeMetadataCacheOnDemand, &decoder);
    if (FAILED(hr)) return hr;

    ComPtr<IWICBitmapFrameDecode> frame;
    hr = decoder->GetFrame(0, &frame);
    if (FAILED(hr)) return hr;

    ComPtr<IWICFormatConverter> converter;
    hr = factory->CreateFormatConverter(&converter);
    if (FAILED(hr)) return hr;
    hr = converter->Initialize(frame.Get(), GUID_WICPixelFormat32bppRGBA, WICBitmapDitherTypeNone,
                               nullptr, 0.0f, WICBitmapPaletteTypeCustom);
    if (FAILED(hr)) return hr;

    UINT width = 0;
    UINT height = 0;
    hr = converter->GetSize(&width, &height);
    if (FAILED(hr) || width == 0 || height == 0) return E_FAIL;

    std::vector<BYTE> pixels(static_cast<size_t>(width) * height * 4);
    hr = converter->CopyPixels(nullptr, width * 4, static_cast<UINT>(pixels.size()), pixels.data());
    if (FAILED(hr)) return hr;

    D3D11_TEXTURE2D_DESC desc = {};
    desc.Width = width;
    desc.Height = height;
    desc.MipLevels = 1;
    desc.ArraySize = 1;
    desc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
    desc.SampleDesc.Count = 1;
    desc.Usage = usage;
    desc.BindFlags = bindFlags;
    desc.CPUAccessFlags = cpuAccessFlags;
    desc.MiscFlags = miscFlags;

    D3D11_SUBRESOURCE_DATA initData = {};
    initData.pSysMem = pixels.data();
    initData.SysMemPitch = width * 4;
    initData.SysMemSlicePitch = 0;

    ComPtr<ID3D11Texture2D> tex;
    hr = device->CreateTexture2D(&desc, &initData, &tex);
    if (FAILED(hr)) return hr;

    ComPtr<ID3D11ShaderResourceView> view;
    D3D11_SHADER_RESOURCE_VIEW_DESC srvDesc = {};
    srvDesc.Format = desc.Format;
    srvDesc.ViewDimension = D3D11_SRV_DIMENSION_TEXTURE2D;
    srvDesc.Texture2D.MipLevels = 1;
    hr = device->CreateShaderResourceView(tex.Get(), &srvDesc, &view);
    if (FAILED(hr)) return hr;

    *texture = tex.Detach();
    *textureView = view.Detach();
    return S_OK;
}

}  // namespace DirectX
