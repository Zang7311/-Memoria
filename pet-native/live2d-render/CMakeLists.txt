cmake_minimum_required(VERSION 3.16)
project(L2DRender CXX)

# Cubism SDK 路径（vendor 目录，已在 .gitignore 里，不提交）
set(SDK_ROOT ${CMAKE_CURRENT_SOURCE_DIR}/../vendor/Cubism)
set(CORE_PATH ${SDK_ROOT}/Core)
set(FRAMEWORK_PATH ${SDK_ROOT}/Framework)
set(COMMON_PATH ${CMAKE_CURRENT_SOURCE_DIR}/common)
set(APP_NAME L2DRender)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# 源文件里有日文注释，统一按 UTF-8 编译（防止 BOM 丢失后按 GBK 解码出错）
add_compile_options(/utf-8)

# ---- Cubism Core（静态库）----
add_library(Live2DCubismCore STATIC IMPORTED)
set_target_properties(Live2DCubismCore PROPERTIES
  IMPORTED_LOCATION_DEBUG   ${CORE_PATH}/lib/windows/x86_64/143/Live2DCubismCore_MDd.lib
  IMPORTED_LOCATION_RELEASE ${CORE_PATH}/lib/windows/x86_64/143/Live2DCubismCore_MD.lib
  INTERFACE_INCLUDE_DIRECTORIES ${CORE_PATH}/include)

# ---- Cubism Framework（D3D11 渲染后端）----
set(FRAMEWORK_SOURCE D3D11)
add_subdirectory(${FRAMEWORK_PATH} ${CMAKE_CURRENT_BINARY_DIR}/Framework)
target_link_libraries(Framework Live2DCubismCore d3d11.lib d3dcompiler.lib)

# ---- 应用（先创建 target，Samples/Common 的 CMakeLists 会往它上面挂源文件）----
add_executable(${APP_NAME} WIN32)

# Cubism 示例用「最小 Demo」分支：common/CMakeLists.txt 靠这个 CMake 变量挑源文件
set(CSM_MINIMUM_DEMO ON)

# ---- Samples/Common ----
add_subdirectory(${COMMON_PATH} ${CMAKE_CURRENT_BINARY_DIR}/src/Common)

target_sources(${APP_NAME} PRIVATE
  ${CMAKE_CURRENT_SOURCE_DIR}/src/LAppDefine.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/LAppPal.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/mainMinimum.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/CubismSprite.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/CubismSpriteShader.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/CubismTextureManager.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/CubismUserModelExtend.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/CubismDirectXView.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/CubismDirectXRenderer.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/thirdparty/WICTextureLoader.cpp)

target_include_directories(${APP_NAME} PRIVATE
  ${CMAKE_CURRENT_SOURCE_DIR}/thirdparty
  ${CMAKE_CURRENT_SOURCE_DIR}/src
  ${CMAKE_CURRENT_SOURCE_DIR}/common)

target_compile_definitions(${APP_NAME} PRIVATE CSM_MINIMUM_DEMO)

target_link_libraries(${APP_NAME} Framework windowscodecs.lib)

# ===== 桌宠用的渲染 DLL（隐藏窗口 + 离屏抓帧）=====
add_library(PetL2D SHARED)
target_sources(PetL2D PRIVATE
  ${COMMON_PATH}/CubismSampleViewMatrix_Common.cpp
  ${COMMON_PATH}/LAppAllocator_Common.cpp
  ${COMMON_PATH}/LAppModel_Common.cpp
  ${COMMON_PATH}/LAppSprite_Common.cpp
  ${COMMON_PATH}/LAppTextureManager_Common.cpp
  ${COMMON_PATH}/LAppView_Common.cpp
  ${COMMON_PATH}/MouseActionManager_Common.cpp
  ${COMMON_PATH}/TouchManager_Common.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/LAppDefine.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/LAppPal.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/CubismSprite.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/CubismSpriteShader.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/CubismTextureManager.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/CubismUserModelExtend.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/CubismDirectXView.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/CubismDirectXRenderer.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/src/dll_main.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/thirdparty/WICTextureLoader.cpp)
target_include_directories(PetL2D PRIVATE
  ${CMAKE_CURRENT_SOURCE_DIR}/thirdparty
  ${CMAKE_CURRENT_SOURCE_DIR}/src
  ${CMAKE_CURRENT_SOURCE_DIR}/common)
target_link_libraries(PetL2D Framework windowscodecs.lib)
set_target_properties(PetL2D PROPERTIES OUTPUT_NAME "PetL2D")

# DLL 旁边的资源与着色器（和 exe 一样）
add_custom_command(TARGET PetL2D POST_BUILD
  COMMAND ${CMAKE_COMMAND} -E copy_directory
    ${SDK_ROOT}/Samples/Resources $<TARGET_FILE_DIR:PetL2D>/Resources
  COMMAND ${CMAKE_COMMAND} -E copy_directory
    ${CMAKE_CURRENT_SOURCE_DIR}/shaders $<TARGET_FILE_DIR:PetL2D>/SampleShaders
  COMMAND ${CMAKE_COMMAND} -E copy_directory
    ${FRAMEWORK_PATH}/src/Rendering/D3D11/Shaders $<TARGET_FILE_DIR:PetL2D>/FrameworkShaders
  COMMAND ${CMAKE_COMMAND} -E copy_if_different
    ${CORE_PATH}/dll/windows/x86_64/Live2DCubismCore.dll $<TARGET_FILE_DIR:PetL2D>)

# 运行时需要的资源与着色器拷到 exe 旁边
add_custom_command(TARGET ${APP_NAME} POST_BUILD
  COMMAND ${CMAKE_COMMAND} -E copy_directory
    ${SDK_ROOT}/Samples/Resources $<TARGET_FILE_DIR:${APP_NAME}>/Resources
  COMMAND ${CMAKE_COMMAND} -E copy_directory
    ${CMAKE_CURRENT_SOURCE_DIR}/shaders $<TARGET_FILE_DIR:${APP_NAME}>/SampleShaders
  COMMAND ${CMAKE_COMMAND} -E copy_directory
    ${FRAMEWORK_PATH}/src/Rendering/D3D11/Shaders $<TARGET_FILE_DIR:${APP_NAME}>/FrameworkShaders)

# Cubism Core 运行时 DLL
add_custom_command(TARGET ${APP_NAME} POST_BUILD
  COMMAND ${CMAKE_COMMAND} -E copy_if_different
    ${CORE_PATH}/dll/windows/x86_64/Live2DCubismCore.dll $<TARGET_FILE_DIR:${APP_NAME}>)
