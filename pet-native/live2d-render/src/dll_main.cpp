// Live2D 渲染 DLL（给桌宠进程调用）
//
// 设计：
//   - 内部创建一个隐藏窗口 + D3D11 swap chain，复用 Cubism 官方 D3D11 渲染管线
//   - 对外只暴露三个 C 函数，所有 C++ 对象都藏在 DLL 内部
//   - 每帧渲染后从后台缓冲抓 RGBA（预乘 alpha、透明背景），调用方直接贴分层窗口
//
// 用法（Rust FFI）：
//   int pet_l2d_load(const char* modelName, const char* resourcesDir, int w, int h);
//   int pet_l2d_render(float timeSeconds, unsigned char* outRGBA, int w, int h);
//   void pet_l2d_destroy(void);

#include <windows.h>

#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#include <CubismFramework.hpp>
#include <Rendering/D3D11/CubismRenderer_D3D11.hpp>

#include "LAppDefine.hpp"
#include "LAppPal.hpp"
#include "CubismUserModelExtend.hpp"
#include "CubismDirectXView.hpp"
#include "CubismDirectXRenderer.hpp"
#include "MouseActionManager_Common.hpp"
#include "LAppAllocator_Common.hpp"
#include "CubismTextureManager.hpp"

static Csm::CubismFramework::Option g_option;
static LAppAllocator_Common g_allocator;
static CubismTextureManager* g_textureManager = nullptr;
static Csm::CubismUserModel* g_userModel = nullptr;
static bool g_ready = false;

static LRESULT CALLBACK PetL2DWndProc(HWND wnd, UINT msg, WPARAM wParam, LPARAM lParam)
{
    if (msg == WM_DESTROY)
    {
        PostQuitMessage(0);
        return 0;
    }
    return DefWindowProc(wnd, msg, wParam, lParam);
}

extern "C" {

// 加载模型。成功后返回 1，失败 0。耗时约 0.7 秒（着色器缓存已就绪时）。
__declspec(dllexport) int pet_l2d_load(const char* modelName, const char* resourcesDir, int width, int height)
{
    if (g_ready) return 1;
    if (!modelName || !resourcesDir || width <= 0 || height <= 0) return 0;

    FILE* lg = fopen("dll_log.txt", "w");
    #define DLLLOG(m) do { if (lg) { fprintf(lg, "%s\n", m); fflush(lg); } } while (0)
    DLLLOG("start");
    CoInitializeEx(NULL, COINIT_APARTMENTTHREADED);

    CubismDirectXRenderer* renderer = CubismDirectXRenderer::GetInstance();

    // 隐藏窗口：只作为 swap chain 的宿主，不显示
    WNDCLASSEX wc = {};
    wc.cbSize = sizeof(wc);
    wc.style = CS_HREDRAW | CS_VREDRAW;
    wc.lpfnWndProc = PetL2DWndProc;
    wc.hInstance = GetModuleHandle(NULL);
    wc.hCursor = LoadCursor(NULL, IDC_ARROW);
    wc.lpszClassName = "PetL2DHiddenWindow";
    RegisterClassEx(&wc);

    HWND hwnd = CreateWindowEx(0, "PetL2DHiddenWindow", "PetL2D", WS_POPUP, 0, 0, width, height,
                               NULL, NULL, wc.hInstance, NULL);
    if (hwnd == NULL) { DLLLOG("window fail"); return 0; }
    DLLLOG("window ok");

    renderer->_windowHandle = hwnd;
    renderer->_windowClass = wc;

    DXGI_SWAP_CHAIN_DESC& pd = renderer->_presentParameters;
    memset(&pd, 0, sizeof(pd));
    pd.BufferCount = renderer->BackBufferNum;
    pd.BufferDesc.Width = width;
    pd.BufferDesc.Height = height;
    pd.BufferDesc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
    pd.BufferDesc.RefreshRate.Numerator = 60;
    pd.BufferDesc.RefreshRate.Denominator = 1;
    pd.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    pd.SampleDesc.Count = 1;
    pd.SampleDesc.Quality = 0;
    pd.OutputWindow = hwnd;
    pd.SwapEffect = DXGI_SWAP_EFFECT_DISCARD;
    pd.Windowed = TRUE;

    D3D_FEATURE_LEVEL level;
    HRESULT hr = D3D11CreateDeviceAndSwapChain(
        NULL, D3D_DRIVER_TYPE_HARDWARE, NULL, 0, NULL, 0, D3D11_SDK_VERSION,
        &pd, &renderer->_swapChain, &renderer->_device, &level, &renderer->_deviceContext);
    if (FAILED(hr))
    {
        LAppPal::PrintLogLn("pet_l2d_load: D3D11CreateDeviceAndSwapChain failed 0x%x", hr);
        return 0;
    }

    DLLLOG("device ok");
    renderer->CreateRenderTarget(static_cast<UINT>(width), static_cast<UINT>(height));
    DLLLOG("render target ok");

    D3D11_DEPTH_STENCIL_DESC depthDesc;
    memset(&depthDesc, 0, sizeof(depthDesc));
    depthDesc.DepthEnable = FALSE;
    depthDesc.DepthWriteMask = D3D11_DEPTH_WRITE_MASK_ALL;
    depthDesc.DepthFunc = D3D11_COMPARISON_LESS;
    depthDesc.StencilEnable = FALSE;
    hr = renderer->_device->CreateDepthStencilState(&depthDesc, &renderer->_depthState);
    if (FAILED(hr))
    {
        LAppPal::PrintLogLn("pet_l2d_load: CreateDepthStencilState failed 0x%x", hr);
        return 0;
    }

    DLLLOG("depth ok");
    g_textureManager = new CubismTextureManager();

    // Cubism Framework 初始化
    g_option.LogFunction = LAppPal::PrintMessage;
    g_option.LoggingLevel = LAppDefine::CubismLoggingLevel;
    g_option.LoadFileFunction = LAppPal::LoadFileAsBytes;
    g_option.ReleaseBytesFunction = LAppPal::ReleaseBytes;
    Csm::CubismFramework::StartUp(&g_allocator, &g_option);
    Csm::CubismFramework::Initialize();

    Csm::Rendering::CubismRenderer_D3D11::SetConstantSettings(renderer->BackBufferNum, renderer->GetD3dDevice());

    // 加载模型（目录 = resourcesDir + modelName + "/"）
    std::string modelDir = std::string(resourcesDir) + modelName + "/";
    g_userModel = new CubismUserModelExtend(std::string(modelName), modelDir);
    static_cast<CubismUserModelExtend*>(g_userModel)->SetupModel(
        static_cast<csmUint32>(width), static_cast<csmUint32>(height));
    MouseActionManager_Common::GetInstance()->SetUserModel(g_userModel);

    CubismDirectXView::GetInstance()->SwitchRenderingTarget(CubismDirectXView::SelectTarget_None);
    float clearColor[3] = { 1.0f, 1.0f, 1.0f };
    CubismDirectXView::GetInstance()->SetRenderTargetClearColor(clearColor[0], clearColor[1], clearColor[2]);

    LAppPal::UpdateTime();
    g_ready = true;
    return 1;
}

// 渲染一帧并写出 RGBA（预乘 alpha、透明背景）。返回 1 成功。
__declspec(dllexport) int pet_l2d_render(float /*timeSeconds*/, unsigned char* outRGBA, int outWidth, int outHeight)
{
    if (!g_ready || !outRGBA) return 0;

    LAppPal::UpdateTime();

    CubismDirectXRenderer* renderer = CubismDirectXRenderer::GetInstance();
    renderer->StartFrame();
    CubismDirectXView::GetInstance()->Render(g_userModel);

    std::vector<unsigned char> rgba;
    unsigned int w = 0;
    unsigned int h = 0;
    if (!renderer->CaptureFrame(rgba, w, h)) return 0;
    if (static_cast<int>(w) != outWidth || static_cast<int>(h) != outHeight) return 0;

    memcpy(outRGBA, rgba.data(), rgba.size());
    return 1;
}

__declspec(dllexport) void pet_l2d_destroy(void)
{
    // 简化实现：进程退出时由系统回收；这里只标记不可用，避免重复渲染
    g_ready = false;
}

// 调试用：把当前帧写到文件（不上屏，验证 DLL 是否工作）
__declspec(dllexport) int pet_l2d_debug_dump(const char* path, int width, int height)
{
    if (!g_ready || !path) return 0;
    std::vector<unsigned char> rgba;
    std::vector<unsigned char> copy(static_cast<size_t>(width) * height * 4, 0);
    if (!pet_l2d_render(0.0f, copy.data(), width, height)) return 0;
    FILE* f = fopen(path, "wb");
    if (!f) return 0;
    fwrite(copy.data(), 1, copy.size(), f);
    fclose(f);
    return 1;
}

}  // extern "C"
