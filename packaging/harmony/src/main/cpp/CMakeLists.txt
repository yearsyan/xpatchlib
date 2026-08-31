cmake_minimum_required(VERSION 3.5.1)
project(xpatchlib)

set(NATIVERENDER_ROOT_PATH ${CMAKE_CURRENT_SOURCE_DIR})

if(DEFINED PACKAGE_FIND_FILE) # hvigor >= 5 injects this
    include(${PACKAGE_FIND_FILE})
endif()

include_directories(${NATIVERENDER_ROOT_PATH})
# The ohos arm64 static lib built by ../../build.sh.
add_library(xpatchlib STATIC IMPORTED)
set_target_properties(xpatchlib PROPERTIES IMPORTED_LOCATION
    ${NATIVERENDER_ROOT_PATH}/../../../libs/arm64-v8a/libxpatchlib_ffi.a)

add_library(xpatchlib_napi SHARED xpatchlib_napi.cpp)
target_link_libraries(xpatchlib_napi PUBLIC xpatchlib libace_napi.z.so)
target_include_directories(xpatchlib_napi PRIVATE
    ${NATIVERENDER_ROOT_PATH}/../../../../src/main/cpp)
