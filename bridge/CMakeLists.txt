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

    add_library(dreamusd_bridge STATIC
        src/error.cpp
        src/stage.cpp
        src/prim.cpp
        src/transform.cpp
        src/hydra.cpp
        src/material.cpp
    )

    target_compile_definitions(dreamusd_bridge PRIVATE HAS_USD=1)

    # Find required dependencies
    find_package(TBB QUIET)
    if(NOT TBB_FOUND)
        # Fallback: try homebrew paths
        find_path(TBB_INCLUDE_DIR tbb/concurrent_queue.h
            HINTS /opt/homebrew/include /usr/local/include)
    else()
        get_target_property(TBB_INCLUDE_DIR TBB::tbb INTERFACE_INCLUDE_DIRECTORIES)
    endif()

    find_package(Boost QUIET)
    if(NOT Boost_FOUND)
        find_path(BOOST_INCLUDE_DIR boost/python.hpp
            HINTS /opt/homebrew/include /usr/local/include)
    else()
        set(BOOST_INCLUDE_DIR ${Boost_INCLUDE_DIRS})
    endif()

    find_path(OPENSUBDIV_INCLUDE_DIR opensubdiv/version.h
        HINTS /opt/homebrew/include /usr/local/include)

    target_include_directories(dreamusd_bridge PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
        PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/src
        ${PXR_INCLUDE_DIRS}
        ${TBB_INCLUDE_DIR}
        ${BOOST_INCLUDE_DIR}
        ${OPENSUBDIV_INCLUDE_DIR}
    )

    # Link USD libs — names vary by platform; glob for common patterns
    file(GLOB USD_LIBS
        "${PXR_LIB_DIR}/libusd_*.dylib"
        "${PXR_LIB_DIR}/libusd_*.so"
        "${PXR_LIB_DIR}/usd_*.lib"
    )
    target_link_libraries(dreamusd_bridge PUBLIC ${USD_LIBS})

    # Platform-specific frameworks
    if(APPLE)
        find_library(METAL_FRAMEWORK Metal)
        find_library(QUARTZCORE_FRAMEWORK QuartzCore)
        if(METAL_FRAMEWORK)
            target_link_libraries(dreamusd_bridge PUBLIC ${METAL_FRAMEWORK} ${QUARTZCORE_FRAMEWORK})
        endif()
    else()
        find_package(Vulkan)
        if(Vulkan_FOUND)
            target_include_directories(dreamusd_bridge PRIVATE ${Vulkan_INCLUDE_DIRS})
            target_link_libraries(dreamusd_bridge PUBLIC Vulkan::Vulkan)
        endif()
    endif()

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
