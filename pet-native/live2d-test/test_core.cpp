// 最小验证：用 Cubism Core 的 C API 加载 moc3，打印画布尺寸/图元数。
// 目的：确认 SDK 的 dll/lib/include 能正确链接与运行，再谈渲染。
// 编译（在 pet-native 目录下、已加载 MSVC 环境）：
//   cl /EHsc /I vendor\Cubism\Core\include live2d-test\test_core.cpp ^
//      /link vendor\Cubism\Core\lib\windows\x86_64\143\Live2DCubismCore_MD.lib
// 运行前把 Core\dll\windows\x86_64\Live2DCubismCore.dll 复制到 exe 旁边。

#include <cstdio>
#include <cstring>
#include <vector>

#include "Live2DCubismCore.h"

int main(int argc, char** argv) {
    const char* path = argc > 1 ? argv[1] : "vendor/Cubism/Samples/Resources/Haru/Haru.moc3";

    FILE* f = fopen(path, "rb");
    if (!f) {
        printf("[test] 打不开模型: %s\n", path);
        return 1;
    }
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    std::vector<char> buf(static_cast<size_t>(size));
    if (fread(buf.data(), 1, static_cast<size_t>(size), f) != static_cast<size_t>(size)) {
        printf("[test] 读取失败\n");
        fclose(f);
        return 1;
    }
    fclose(f);
    printf("[test] moc3 = %s（%ld 字节）\n", path, size);

    const unsigned int latest = csmGetLatestMocVersion();
    const unsigned int ver = csmGetMocVersion(buf.data(), static_cast<unsigned int>(size));
    printf("[test] moc 版本 = %u（SDK 支持到 %u）\n", ver, latest);
    if (ver == 0 || ver > latest) {
        printf("[test] 版本不兼容\n");
        return 1;
    }

    csmMoc* moc = csmReviveMocInPlace(buf.data(), static_cast<unsigned int>(size));
    if (!moc) {
        printf("[test] csmReviveMocInPlace 失败\n");
        return 1;
    }
    const unsigned int modelSize = csmGetSizeofModel(moc);
    printf("[test] 模型内存 = %u 字节\n", modelSize);

    std::vector<char> modelBuf(modelSize);
    csmModel* model = csmInitializeModelInPlace(moc, modelBuf.data(), modelSize);
    if (!model) {
        printf("[test] csmInitializeModelInPlace 失败\n");
        return 1;
    }
    csmUpdateModel(model);

    csmVector2 canvasSize;
    csmVector2 canvasOrigin;
    float ppu = 0.0f;
    csmReadCanvasInfo(model, &canvasSize, &canvasOrigin, &ppu);
    printf("[test] 画布 = %.1f x %.1f，原点 = (%.1f, %.1f)，PPU = %.2f\n",
           canvasSize.X, canvasSize.Y, canvasOrigin.X, canvasOrigin.Y, ppu);
    printf("[test] 图元(drawable) = %d，参数(parameter) = %d，部件(part) = %d\n",
           csmGetDrawableCount(model), csmGetParameterCount(model), csmGetPartCount(model));
    printf("[test] 成功：Core 链接与加载都正常\n");
    return 0;
}
