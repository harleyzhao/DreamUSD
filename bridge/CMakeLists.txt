cmake_minimum_required(VERSION 3.24)
project(dreamusd_bridge CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# For stub build, we don't need USD or Vulkan yet
# find_package(pxr QUIET)
# find_package(Vulkan REQUIRED)

add_library(dreamusd_bridge STATIC
    src/stub.cpp
)

target_include_directories(dreamusd_bridge PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/include
)

install(TARGETS dreamusd_bridge ARCHIVE DESTINATION lib)
install(DIRECTORY include/ DESTINATION include)
