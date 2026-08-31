cmake_minimum_required(VERSION 3.5.1)
project(xpatchlib)

set(NATIVERENDER_ROOT_PATH ${CMAKE_CURRENT_SOURCE_DIR})

if(DEFINED PACKAGE_FIND_FILE) # hvigor >= 5 injects this
    include(${PACKAGE_FIND_FILE})
endif()

# xpatchlib.h sits next to this file (copied by ../../build.sh so the HAR
# is self-contained) — the compiler finds it via the quoted include's own
# directory, but keep the cpp dir on the path for any internal includes.
include_directories(${NATIVERENDER_ROOT_PATH})

# The ohos arm64 static lib built by ../../build.sh (replay only: the
# "produce" feature is off, no patch generation code inside).
add_library(xpatchlib STATIC IMPORTED)
set_target_properties(xpatchlib PROPERTIES IMPORTED_LOCATION
    ${NATIVERENDER_ROOT_PATH}/../../../libs/arm64-v8a/libxpatchlib_ffi.a)

add_library(xpatchlib_napi SHARED xpatchlib_napi.cpp)
target_link_libraries(xpatchlib_napi PUBLIC xpatchlib libace_napi.z.so)
