cmake_minimum_required(VERSION 3.24)
project(dreamusd_bridge CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

if(DEFINED ENV{USD_ROOT} OR DEFINED USD_ROOT)
    # --- Real OpenUSD build ---
    if(NOT DEFINED USD_ROOT)
        set(USD_ROOT $ENV{USD_ROOT})
    endif()

    message(STATUS "Building with OpenUSD from: ${USD_ROOT}")
    set(PXR_INCLUDE_DIRS ${USD_ROOT}/include)
    set(PXR_LIB_DIR ${USD_ROOT}/lib)

    find_package(Vulkan REQUIRED)

    add_library(dreamusd_bridge STATIC
        src/error.cpp
        src/stage.cpp
        src/prim.cpp
        src/transform.cpp
        src/hydra.cpp
        src/material.cpp
    )

    target_compile_definitions(dreamusd_bridge PRIVATE HAS_USD=1)

    target_include_directories(dreamusd_bridge PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
        PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/src
        ${PXR_INCLUDE_DIRS}
        ${Vulkan_INCLUDE_DIRS}
    )

    # Link USD libs — names vary by platform; glob for common patterns
    file(GLOB USD_LIBS
        "${PXR_LIB_DIR}/libusd_*.dylib"
        "${PXR_LIB_DIR}/libusd_*.so"
        "${PXR_LIB_DIR}/usd_*.lib"
    )
    target_link_libraries(dreamusd_bridge PUBLIC ${USD_LIBS} Vulkan::Vulkan)

else()
    # --- Stub build (no USD_ROOT set) ---
    message(STATUS "Building with stubs (USD_ROOT not set)")

    add_library(dreamusd_bridge STATIC
        src/error.cpp
        src/stage.cpp
        src/prim.cpp
        src/transform.cpp
        src/hydra.cpp
        src/material.cpp
    )

    # No HAS_USD — each .cpp will compile its #else stub path
    target_include_directories(dreamusd_bridge PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
        PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/src
    )
endif()

install(TARGETS dreamusd_bridge ARCHIVE DESTINATION lib)
install(DIRECTORY include/ DESTINATION include)
